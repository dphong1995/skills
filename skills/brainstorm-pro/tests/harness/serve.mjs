// Runs assets/render-interview.mjs with a fake `open` first on PATH, so no browser window ever opens.
import { spawn, spawnSync } from 'node:child_process';
import { copyFileSync, mkdirSync, rmSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = dirname(fileURLToPath(import.meta.url));
export const SKILL = join(HERE, '..', '..');
export const TESTS = join(SKILL, 'tests');
export const FIXTURES = join(TESTS, 'fixtures');
export const SCRIPT = join(SKILL, 'assets', 'render-interview.mjs');

// A fresh directory under tests/out/<name> for one test file's output.
export const outDir = (name) => {
  const dir = join(TESTS, 'out', name);
  rmSync(dir, { recursive: true, force: true });
  mkdirSync(dir, { recursive: true });
  return dir;
};

// bin: 'fakebin' (open succeeds) or 'failbin' (open fails, the script exits 4).
// The child gets a fixed CLAUDE_HOME (the Vietnamese fixture unless `claudeHome` is given), so the page language never follows the developer's own settings.
const run = (src, out, args, { cwd = TESTS, bin = 'fakebin', claudeHome = join(FIXTURES, 'claude-home-vi') } = {}) => [process.execPath, [SCRIPT, ...(src ? [src] : []), ...(out ? [out] : []), ...args],
  { cwd, env: { ...process.env, CLAUDE_HOME: claudeHome, PATH: join(HERE, bin) + ':' + process.env.PATH } }];

// Runs the script to the end without serving (a file page, --open, or an error case).
export const render = (src, out, args = [], opts) => {
  const [cmd, a, o] = run(src, out, args, opts);
  const r = spawnSync(cmd, a, { ...o, encoding: 'utf8', timeout: 60000 });
  return { code: r.status, stdout: r.stdout, stderr: r.stderr };
};

// Starts --serve; `url` resolves to the served address, `done` to { code, stdout, stderr } when the script exits.
// `opts.timeout` (default 20 s): after it the child is killed and `done` rejects with the last stderr lines.
export const serve = (src, out, opts) => {
  const [cmd, a, o] = run(src, out, ['--serve', ...((opts && opts.args) || [])], opts);
  const p = spawn(cmd, a, o);
  let stdout = '', stderr = '';
  let gotUrl;
  const url = new Promise((r, no) => { gotUrl = { r, no }; });
  url.catch(() => {});
  p.stdout.on('data', (d) => { stdout += d; const m = /Serving (\S+)/.exec(stdout); if (m) gotUrl.r(m[1]); });
  p.stderr.on('data', (d) => { stderr += d; });
  const done = new Promise((r, no) => {
    const timer = setTimeout(() => { p.kill(); no(new Error(`serve timed out; stderr: ${stderr.trim().split('\n').slice(-5).join(' | ')}`)); }, (opts && opts.timeout) || 20000);
    p.on('exit', (code) => { clearTimeout(timer); if (!/Serving/.test(stdout)) gotUrl.no(new Error('exited ' + code + ' ' + stderr)); r({ code, stdout, stderr }); });
  });
  done.catch(() => {});
  // The decision a finished run printed: { code, src, lines }.
  const answers = async () => { const r = await done; return { code: r.code, ...parseAnswers(r.stdout) }; };
  return { url, done, answers, kill: () => p.kill() };
};

// The lines of a finished run after "ANSWERS <src>": { src, lines }, or null when there is no answer.
export const parseAnswers = (stdout) => {
  const all = stdout.trimEnd().split('\n');
  const i = all.findIndex((l) => l.startsWith('ANSWERS '));
  return i < 0 ? null : { src: all[i].slice(8), lines: all.slice(i + 1) };
};

// A review served twice from `dir` (the round state lives beside the page): round 1 of doc.md, then round 2 of doc-v2.md. Returns round 2's server.
// `round1(server)` may drive round 1 (type notes, send them) before it is stopped.
export const serveRound2 = async (dir, round1) => {
  const md = dir + '/doc.md';
  copyFileSync(FIXTURES + '/doc.md', md);
  copyFileSync(FIXTURES + '/code.ts', dir + '/code.ts');
  const first = serve(md, dir + '/review-01.html');
  try {
    await first.url;
    if (round1) await round1(first);
  } finally { first.kill(); }
  await first.done;
  copyFileSync(FIXTURES + '/doc-v2.md', md);
  return serve(md, dir + '/review-02.html');
};
