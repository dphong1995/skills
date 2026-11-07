import { test } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { existsSync, mkdtempSync, readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const legacy = JSON.parse(readFileSync(join(root, 'tests/fixtures/settings-legacy.json'), 'utf8'));
const ITEMS = ['skills', 'subagent', 'vietnamese', 'compact', 'org-skills', 'cmux', 'hud', 'widget'];

function run(args, { home } = {}) {
  home ??= mkdtempSync(join(tmpdir(), 'skills-install-'));
  const r = spawnSync('bash', [join(root, 'install.sh'), ...args], {
    env: { ...process.env, CLAUDE_HOME: home, SKILLS_INSTALL_DRY_EXTERNAL: '1' },
    stdio: ['ignore', 'pipe', 'pipe'],
    encoding: 'utf8',
  });
  return { code: r.status, stdout: r.stdout, stderr: r.stderr, home };
}
const settings = (home) => JSON.parse(readFileSync(join(home, 'settings.json'), 'utf8'));
const claudeMd = (home) => (existsSync(join(home, 'CLAUDE.md')) ? readFileSync(join(home, 'CLAUDE.md'), 'utf8') : '');
const ls = (home) => readdirSync(home).filter((n) => n !== 'backups').sort();
const count = (s, sub) => s.split(sub).length - 1;

test('only skills', () => {
  const { code, home } = run(['--only', 'skills']);
  assert.equal(code, 0);
  for (const s of ['brainstorm-pro', 'ck-plan', 'cook', 'routing-bug-fixes-to-cook']) assert.ok(existsSync(join(home, 'skills', s)), s);
  assert.equal(readdirSync(join(home, 'agents')).filter((n) => n.endsWith('.md')).length, 19);
  const md = claudeMd(home);
  assert.ok(md.includes('<!-- skills-repo: web-pages -->'));
  assert.ok(!md.includes('skills-repo: language'));
  const s = settings(home);
  assert.equal(s.env.CLAUDE_CODE_ENABLE_TODO_TOOLS, '1');
  assert.ok(!('language' in s) && !('skillOverrides' in s) && !('hooks' in s));
  assert.ok(!existsSync(join(home, 'hooks')) && !existsSync(join(home, 'mods')));
});

test('only vietnamese', () => {
  const { code, home } = run(['--only', 'vietnamese']);
  assert.equal(code, 0);
  assert.ok(existsSync(join(home, 'hooks/reply-language.mjs')));
  const s = settings(home);
  assert.equal(s.language, 'vietnamese');
  assert.equal(s.hooks.Stop.length, 1);
  assert.equal(s.hooks.Stop[0].hooks[0].command, 'node ~/.claude/hooks/reply-language.mjs');
  assert.ok(claudeMd(home).includes('<!-- skills-repo: language -->'));
  assert.ok(!existsSync(join(home, 'skills')));
});

test('only subagent', () => {
  const { code, home } = run(['--only', 'subagent']);
  assert.equal(code, 0);
  assert.ok(existsSync(join(home, 'hooks/subagent-context-guard.mjs')));
  const s = settings(home);
  assert.equal(s.hooks.PostToolUse.length, 1);
  assert.deepEqual(Object.keys(s), ['hooks']);
  assert.ok(claudeMd(home).includes('<!-- skills-repo: subagent-models -->'));
  assert.ok(!existsSync(join(home, 'skills')) && !existsSync(join(home, 'mods')));
  assert.ok(!existsSync(join(home, 'hooks/reply-language.mjs')));
});

test('only compact', () => {
  const { code, home } = run(['--only', 'compact']);
  assert.equal(code, 0);
  assert.ok(existsSync(join(home, 'mods/main-context-compact')));
  assert.deepEqual(settings(home), { env: { CLAUDE_CODE_PLUGIN_DIRS: '~/.claude/mods/main-context-compact' } });
  assert.equal(claudeMd(home), '');
  assert.ok(!existsSync(join(home, 'skills')) && !existsSync(join(home, 'hooks')));
});

test('only org-skills', () => {
  const { code, home } = run(['--only', 'org-skills']);
  assert.equal(code, 0);
  assert.deepEqual(settings(home), { skillOverrides: legacy.skillOverrides });
  assert.deepEqual(ls(home).sort(), ['.skills-repo-selection', 'settings.json']);
});

test('all', () => {
  const { code, stdout, home } = run(['--all']);
  assert.equal(code, 0);
  for (const n of ['cmux', 'hud', 'claude-sessions']) assert.match(stdout, new RegExp(`would run: .*${n}`), n);
  assert.deepEqual(settings(home), legacy);
});

test('rerun does not duplicate', () => {
  const home = mkdtempSync(join(tmpdir(), 'skills-install-'));
  for (let i = 0; i < 2; i++) assert.equal(run(['--only', 'skills,subagent,vietnamese'], { home }).code, 0);
  const s = settings(home);
  for (const ev of Object.keys(s.hooks)) assert.equal(s.hooks[ev].length, 1, ev);
  const md = claudeMd(home);
  for (const b of ['web-pages', 'subagent-models', 'language']) assert.equal(count(md, `<!-- skills-repo: ${b} -->`), 1, b);
});

test('old install then new --only does not duplicate', () => {
  const home = mkdtempSync(join(tmpdir(), 'skills-install-'));
  writeFileSync(join(home, 'settings.json'), JSON.stringify(legacy));
  const only = 'skills,subagent,vietnamese,compact,org-skills';
  for (let i = 0; i < 2; i++) assert.equal(run(['--only', only], { home }).code, 0);
  const s = settings(home);
  for (const ev of Object.keys(legacy.hooks)) {
    assert.equal(s.hooks[ev].length, 1, ev);
  }
  const md = claudeMd(home);
  for (const b of ['web-pages', 'subagent-models', 'language']) assert.equal(count(md, `<!-- skills-repo: ${b} -->`), 1, b);
});

test('selection accumulates', () => {
  const home = mkdtempSync(join(tmpdir(), 'skills-install-'));
  assert.equal(run(['--only', 'skills'], { home }).code, 0);
  assert.equal(run(['--only', 'hud'], { home }).code, 0);
  assert.equal(readFileSync(join(home, '.skills-repo-selection'), 'utf8'), 'skills\nhud\n');
  const r = run(['--update'], { home });
  assert.equal(r.code, 0);
  assert.match(r.stdout, /would run: .*hud/);
  assert.match(r.stdout, /skill  brainstorm-pro/);
  assert.ok(!/would run: .*cmux/.test(r.stdout));
  assert.equal(readFileSync(join(home, '.skills-repo-selection'), 'utf8'), 'skills\nhud\n');
});

test('unknown item installs nothing', () => {
  const { code, stderr, home } = run(['--only', 'skills,nope']);
  assert.equal(code, 2);
  assert.match(stderr, /nope/);
  assert.deepEqual(readdirSync(home), []);
});

test('no flags without a TTY exits 2', () => {
  const { code, home } = run([]);
  assert.equal(code, 2);
  assert.deepEqual(readdirSync(home), []);
});

test('--update without selection and no TTY exits 2', () => {
  assert.equal(run(['--update']).code, 2);
});

test('selection of only unknown names counts as none', () => {
  const home = mkdtempSync(join(tmpdir(), 'skills-install-'));
  spawnSync('bash', ['-c', `printf 'bogus\\n' > "${home}/.skills-repo-selection"`]);
  const r = run(['--update'], { home });
  assert.equal(r.code, 2);
  assert.ok(!existsSync(join(home, 'skills')));
});

test('two flags exit 2', () => {
  const { code, home } = run(['--all', '--only', 'skills']);
  assert.equal(code, 2);
  assert.deepEqual(readdirSync(home), []);
});

test('list', () => {
  const { code, stdout, home } = run(['--list']);
  assert.equal(code, 0);
  const names = stdout.trim().split('\n').map((l) => l.split(/\s+/)[0]);
  assert.deepEqual(names, ITEMS);
  assert.deepEqual(readdirSync(home), []);
});
