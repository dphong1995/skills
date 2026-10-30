import { test, after } from 'node:test';
import assert from 'node:assert/strict';
import { spawn, spawnSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const hook = path.join(path.dirname(fileURLToPath(import.meta.url)), '..', 'subagent-context-guard.mjs');
const markerDir = path.join(os.tmpdir(), 'claude-subagent-guard');
const root = fs.mkdtempSync(path.join(os.tmpdir(), 'guard-test-'));
const agentIds = [];
let n = 0;

after(() => {
  fs.rmSync(root, { recursive: true, force: true });
  for (const id of agentIds) fs.rmSync(path.join(markerDir, id), { force: true });
});

const record = (model, context) => JSON.stringify({
  type: 'assistant',
  requestId: 'req',
  message: { model, usage: { input_tokens: 2, cache_read_input_tokens: context - 3002, cache_creation_input_tokens: 3000, output_tokens: 120 } },
});

function setup(lines) {
  const id = `test${process.pid}x${n++}`;
  agentIds.push(id);
  const session = 'sess';
  const transcript = path.join(root, `${id}.jsonl`);
  if (lines) {
    const dir = path.join(root, session, 'subagents');
    fs.mkdirSync(dir, { recursive: true });
    fs.writeFileSync(path.join(dir, `agent-${id}.jsonl`), lines.join('\n') + '\n');
  }
  return { agent_id: id, session_id: session, transcript_path: transcript };
}

const run = (event) => spawnSync(process.execPath, [hook], { input: typeof event === 'string' ? event : JSON.stringify(event), encoding: 'utf8' });

function assertSilent(r) {
  assert.equal(r.status, 0);
  assert.equal(r.stdout, '');
}

test('no agent_id is silent', () => {
  const e = setup([record('claude-sonnet-5-5', 250000)]);
  delete e.agent_id;
  assertSilent(run(e));
});

test('sonnet at 199k is silent', () => assertSilent(run(setup([record('claude-sonnet-5-5', 199000)]))));

test('sonnet at 201k nudges with valid JSON', () => {
  const r = run(setup([record('claude-sonnet-5-5', 201000)]));
  assert.equal(r.status, 0);
  const out = JSON.parse(r.stdout);
  assert.equal(out.hookSpecificOutput.hookEventName, 'PostToolUse');
  assert.match(out.hookSpecificOutput.additionalContext, /201k tokens \(limit 200k\)/);
  assert.match(out.hookSpecificOutput.additionalContext, /"PARTIAL"/);
});

test('opus at 299k is silent', () => assertSilent(run(setup([record('claude-opus-5-5', 299000)]))));

test('opus at 301k nudges', () => {
  const r = run(setup([record('claude-opus-5-5', 301000)]));
  assert.match(JSON.parse(r.stdout).hookSpecificOutput.additionalContext, /301k tokens \(limit 300k\)/);
});

test('uses the last assistant record', () => {
  const r = run(setup([record('claude-sonnet-5-5', 250000), '{"type":"user"}', record('claude-sonnet-5-5', 100000)]));
  assertSilent(r);
});

test('second call for the same agent is silent', () => {
  const e = setup([record('claude-sonnet-5-5', 250000)]);
  assert.notEqual(run(e).stdout, '');
  assertSilent(run(e));
});

test('missing transcript is silent', () => assertSilent(run(setup(null))));

test('garbage transcript is silent', () => assertSilent(run(setup(['not json', '{"type":"assistant"}', '\u0000\u0001']))));

test('non-JSON stdin is silent', () => assertSilent(run('not json')));

const runAsync = (event) => new Promise((resolve) => {
  const child = spawn(process.execPath, [hook]);
  let stdout = '';
  child.stdout.on('data', (d) => { stdout += d; });
  child.on('close', (status) => resolve({ status, stdout }));
  child.stdin.end(JSON.stringify(event));
});

test('concurrent calls for the same agent nudge exactly once', async () => {
  const e = setup([record('claude-sonnet-5-5', 250000)]);
  const results = await Promise.all(Array.from({ length: 6 }, () => runAsync(e)));
  assert.ok(results.every((r) => r.status === 0));
  assert.equal(results.filter((r) => r.stdout !== '').length, 1);
});

test('synthetic records are skipped', () => {
  const synthetic = JSON.stringify({ type: 'assistant', message: { model: '<synthetic>', usage: { input_tokens: 0, output_tokens: 0 } } });
  const r = run(setup([record('claude-sonnet-5-5', 250000), synthetic]));
  assert.match(JSON.parse(r.stdout).hookSpecificOutput.additionalContext, /250k tokens/);
});

test('unsafe agent_id is silent', () => {
  const e = setup([record('claude-sonnet-5-5', 250000)]);
  e.agent_id = '../x';
  assertSilent(run(e));
});

test('record after a 400 KB line is found in the tail window', () => {
  const pad = JSON.stringify({ type: 'user', text: 'x'.repeat(400 * 1024) });
  assert.notEqual(run(setup([pad, record('claude-sonnet-5-5', 250000)])).stdout, '');
});

test('record before a 300 KB line is outside the tail window', () => {
  const pad = JSON.stringify({ type: 'user', text: 'x'.repeat(300 * 1024) });
  assertSilent(run(setup([record('claude-sonnet-5-5', 250000), pad])));
});
