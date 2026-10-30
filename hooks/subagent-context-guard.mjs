// PostToolUse hook: when a subagent's context crosses its limit (Sonnet 200k, Opus 300k), nudge it once to stop and return a PARTIAL report.
// The main thread has no agent_id and is never touched; any error exits silently.
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

const TAIL_BYTES = 256 * 1024;
const limitFor = (model) => (/opus/i.test(model || '') ? 300000 : 200000);

try {
  let input = '';
  for await (const chunk of process.stdin) input += chunk;
  const { agent_id: agentId, session_id: sessionId, transcript_path: transcriptPath } = JSON.parse(input);
  if (!agentId || !sessionId || !transcriptPath) process.exit(0);
  if (!/^[\w-]+$/.test(agentId)) process.exit(0);

  const markerDir = path.join(os.tmpdir(), 'claude-subagent-guard');
  const marker = path.join(markerDir, String(agentId));
  if (fs.existsSync(marker)) process.exit(0);

  const file = path.join(path.dirname(transcriptPath), sessionId, 'subagents', `agent-${agentId}.jsonl`);
  const fd = fs.openSync(file, 'r');
  let text;
  try {
    const { size } = fs.fstatSync(fd);
    const start = Math.max(0, size - TAIL_BYTES);
    const buf = Buffer.alloc(size - start);
    fs.readSync(fd, buf, 0, buf.length, start);
    text = buf.toString('utf8');
    if (start > 0) text = text.slice(text.indexOf('\n') + 1);
  } finally {
    fs.closeSync(fd);
  }

  let last = null;
  const lines = text.split('\n');
  for (let i = lines.length - 1; i >= 0 && !last; i--) {
    try {
      const rec = JSON.parse(lines[i]);
      if (rec.type === 'assistant' && rec.message?.usage && rec.message.model !== '<synthetic>') last = rec.message;
    } catch (e) { /* partial or garbage line */ }
  }
  if (!last) process.exit(0);

  const u = last.usage;
  const context = (u.input_tokens || 0) + (u.cache_read_input_tokens || 0) + (u.cache_creation_input_tokens || 0);
  const limit = limitFor(last.model);
  if (context <= limit) process.exit(0);

  fs.mkdirSync(markerDir, { recursive: true });
  fs.writeFileSync(marker, '', { flag: 'wx' });
  console.log(JSON.stringify({
    hookSpecificOutput: {
      hookEventName: 'PostToolUse',
      additionalContext: `Your context is at ${Math.round(context / 1000)}k tokens (limit ${limit / 1000}k). Finish the step you are on, then stop and return your report now. `
        + 'Start the report with "PARTIAL", then list what is done and exactly what remains (files, findings, commands still to run) so a fresh agent can continue. Do not start new work.',
    },
  }));
} catch (e) {
  process.exit(0);
}
