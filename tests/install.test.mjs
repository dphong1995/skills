import { test } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const legacy = JSON.parse(readFileSync(join(root, 'tests/fixtures/settings-legacy.json'), 'utf8'));
const ITEMS = ['skills', 'subagent', 'vietnamese', 'compact', 'org-skills', 'cmux', 'hud', 'widget'];

function run(args, { home } = {}) {
  home ??= mkdtempSync(join(tmpdir(), 'skills-install-'));
  const r = spawnSync('bash', [join(root, 'install.sh'), ...args], {
    // A lock that does not exist keeps a committed release.lock from reaching the real widget install.
    env: { ...process.env, CLAUDE_HOME: home, SKILLS_INSTALL_DRY_EXTERNAL: '1', SKILLS_WIDGET_LOCK: join(home, 'no-such.lock'), SKILLS_WIDGET_DMG: '' },
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
  for (const n of ['cmux', 'hud']) assert.match(stdout, new RegExp(`would run: .*${n}`), n);
  assert.match(stdout, /would install pinned widget-v|widget skipped: no release pinned/);
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

// ---- widget: pinned, verified release dmg ----
const SYSTEM_PATH = ['/usr/bin', '/bin', '/usr/sbin', '/sbin'];
const WIDGET_TOOLS = ['awk', 'basename', 'cat', 'codesign', 'cp', 'cut', 'date', 'dirname', 'ditto', 'env', 'grep', 'hdiutil', 'head', 'ln', 'ls', 'mkdir', 'mktemp', 'mv', 'plutil', 'readlink', 'rm', 'sed', 'shasum', 'sleep', 'sort', 'tail', 'touch', 'tr', 'uname'];
const mac = process.platform === 'darwin' ? test : test.skip;

function makeDmg(dir, name, { bundleId = 'local.claude-sessions', version = '2.0.0-rc.1', sign = true } = {}) {
  const src = join(dir, `src-${name}`);
  const app = join(src, 'ClaudeSessions.app');
  mkdirSync(join(app, 'Contents/MacOS'), { recursive: true });
  writeFileSync(join(app, 'Contents/Info.plist'), `<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>${bundleId}</string>
<key>CFBundleExecutable</key><string>ClaudeSessions</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleShortVersionString</key><string>${version}</string>
</dict></plist>
`);
  writeFileSync(join(app, 'Contents/MacOS/ClaudeSessions'), '#!/bin/sh\nexit 0\n');
  chmodSync(join(app, 'Contents/MacOS/ClaudeSessions'), 0o755);
  if (sign) assert.equal(spawnSync('codesign', ['--force', '-s', '-', app]).status, 0);
  const dmg = join(dir, `${name}.dmg`);
  assert.equal(spawnSync('hdiutil', ['create', '-quiet', '-volname', 'ClaudeSessions', '-srcfolder', src, '-format', 'UDZO', dmg]).status, 0);
  return { dmg, sha: createHash('sha256').update(readFileSync(dmg)).digest('hex') };
}

function widgetEnv(dmgs) {
  const dir = mkdtempSync(join(tmpdir(), 'skills-widget-'));
  const bin = join(dir, 'bin');
  mkdirSync(bin);
  for (const t of WIDGET_TOOLS) {
    const found = SYSTEM_PATH.map((d) => join(d, t)).find((p) => existsSync(p));
    assert.ok(found, t);
    symlinkSync(found, join(bin, t));
  }
  const stub = (name, body) => {
    writeFileSync(join(bin, name), `#!/bin/bash\necho "${name} $*" >> "${dir}/calls.log"\n${body}\n`);
    chmodSync(join(bin, name), 0o755);
  };
  stub('curl', 'out=""; while [ $# -gt 0 ]; do if [ "$1" = "-o" ]; then out="$2"; fi; shift; done\n[ -z "$TEST_CURL_FAIL" ] || exit 22\ncp "$TEST_DMG" "$out"');
  stub('open', 'exit 0');
  // The widget counts as running while <dir>/running exists; quitting it (osascript) removes the file.
  stub('osascript', `rm -f "${dir}/running"`);
  stub('pgrep', `[ -e "${dir}/running" ]`);
  stub('pkill', 'exit 0');
  assert.ok(!existsSync(join(bin, 'swift')) && !existsSync(join(bin, 'cargo')));
  const home = join(dir, 'home');
  mkdirSync(join(home, 'Applications'), { recursive: true });
  return { dir, bin, home, stub, claude: join(dir, 'claude'), calls: () => (existsSync(join(dir, 'calls.log')) ? readFileSync(join(dir, 'calls.log'), 'utf8') : '') };
}

function runWidget(env, { sha, tag = 'widget-v2.0.0-rc.1', dmg, curlFail = false, local = false, extra = {} }) {
  const lock = join(env.dir, 'release.lock');
  writeFileSync(lock, `tag=${tag}\nsha256=${sha}\n`);
  const e = {
    PATH: env.bin, HOME: env.home, CLAUDE_HOME: env.claude, SKILLS_WIDGET_LOCK: lock, TEST_DMG: dmg, ...extra,
  };
  if (curlFail) e.TEST_CURL_FAIL = '1';
  if (local) e.SKILLS_WIDGET_DMG = dmg;
  const r = spawnSync('/bin/bash', [join(root, 'install.sh'), '--only', 'widget'], { env: e, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] });
  return { code: r.status, out: r.stdout + r.stderr };
}

const installedApp = (env) => join(env.home, 'Applications/ClaudeSessions.app');
const markExisting = (env, version = '1.0.0') => {
  const app = installedApp(env);
  mkdirSync(join(app, 'Contents'), { recursive: true });
  writeFileSync(join(app, 'Contents/Info.plist'), `<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0"><dict><key>CFBundleIdentifier</key><string>local.claude-sessions</string><key>CFBundleShortVersionString</key><string>${version}</string></dict></plist>
`);
  writeFileSync(join(app, 'OLD'), 'old');
};

mac('widget installs from pinned release dmg', () => {
  const env = widgetEnv();
  const good = makeDmg(env.dir, 'good');
  const r = runWidget(env, { sha: good.sha, dmg: good.dmg });
  assert.equal(r.code, 0, r.out);
  assert.ok(existsSync(join(installedApp(env), 'Contents/MacOS/ClaudeSessions')), r.out);
  assert.equal(env.calls().split('\n').filter((l) => l.startsWith('open ')).length, 1);
  assert.match(env.calls(), /curl .*https:\/\/github\.com\/dphong1995\/skills\/releases\/download\/widget-v2\.0\.0-rc\.1\/ClaudeSessions\.dmg/);
  assert.ok(!existsSync(join(env.home, 'Applications/.ClaudeSessions.app.new')));
});

mac('widget: every verification failure leaves the app and still finishes the run', () => {
  const env = widgetEnv();
  const good = makeDmg(env.dir, 'good');
  const unsigned = makeDmg(env.dir, 'unsigned', { sign: false });
  const wrongId = makeDmg(env.dir, 'wrongid', { bundleId: 'evil.app' });
  const cases = [
    ['checksum mismatch', { sha: '0'.repeat(64), dmg: good.dmg }, /checksum/],
    ['curl failing', { sha: good.sha, dmg: good.dmg, curlFail: true }, /download/],
    ['unsigned app', { sha: unsigned.sha, dmg: unsigned.dmg }, /signature/],
    ['wrong bundle id', { sha: wrongId.sha, dmg: wrongId.dmg }, /bundle id/],
  ];
  for (const [name, opts, cause] of cases) {
    markExisting(env);
    const r = runWidget(env, opts);
    assert.equal(r.code, 0, `${name}: ${r.out}`);
    assert.match(r.out, new RegExp(`Skipped:\\n\\s+widget: .*${cause.source}`), name);
    assert.ok(existsSync(join(installedApp(env), 'OLD')), `${name}: existing app untouched`);
    assert.ok(!env.calls().split('\n').some((l) => l.startsWith('open ')), name);
    assert.match(r.out, /Installed into/, `${name}: run finished`);
    assert.equal(readFileSync(join(env.claude, '.skills-repo-selection'), 'utf8'), 'widget\n', name);
  }
});

mac('widget: same version installed does nothing', () => {
  const env = widgetEnv();
  const good = makeDmg(env.dir, 'good');
  markExisting(env, '2.0.0-rc.1');
  const r = runWidget(env, { sha: good.sha, dmg: good.dmg });
  assert.equal(r.code, 0, r.out);
  assert.match(r.out, /widget already 2\.0\.0-rc\.1/);
  assert.ok(existsSync(join(installedApp(env), 'OLD')));
  assert.ok(!/^(open|osascript|pkill) /m.test(env.calls()));
});

mac('widget: lock version already installed skips the download', () => {
  const env = widgetEnv();
  const good = makeDmg(env.dir, 'good');
  markExisting(env, '2.0.0-rc.1');
  const r = runWidget(env, { sha: good.sha, dmg: good.dmg });
  assert.equal(r.code, 0, r.out);
  assert.ok(!/^(curl|hdiutil) /m.test(env.calls()));
  assert.match(r.out, /already installed/);
});

mac('widget: dry run with a lock present does no download, quit or copy', () => {
  const env = widgetEnv();
  const good = makeDmg(env.dir, 'good');
  markExisting(env);
  writeFileSync(join(env.dir, 'running'), '');
  const r = runWidget(env, { sha: good.sha, dmg: good.dmg, extra: { SKILLS_INSTALL_DRY_EXTERNAL: '1' } });
  assert.equal(r.code, 0, r.out);
  assert.match(r.out, /would install pinned widget-v2\.0\.0-rc\.1/);
  assert.match(r.out, /Dry run: nothing was downloaded, built, quit or reopened/);
  assert.ok(!/verified the pinned widget release/.test(r.out), r.out);
  assert.equal(env.calls(), '');
  assert.ok(existsSync(join(env.dir, 'running')));
  assert.ok(existsSync(join(installedApp(env), 'OLD')));
});

mac('widget: a running old app is quit, then the new one is opened', () => {
  const env = widgetEnv();
  const good = makeDmg(env.dir, 'good');
  markExisting(env);
  writeFileSync(join(env.dir, 'running'), '');
  const r = runWidget(env, { sha: good.sha, dmg: good.dmg });
  assert.equal(r.code, 0, r.out);
  const calls = env.calls().split('\n');
  const quit = calls.findIndex((l) => l.startsWith('osascript '));
  const open = calls.findIndex((l) => l.startsWith('open '));
  assert.ok(quit >= 0 && open > quit, env.calls());
  assert.ok(!existsSync(join(installedApp(env), 'OLD')));
  assert.ok(existsSync(join(installedApp(env), 'Contents/MacOS/ClaudeSessions')));
});

mac('widget: a failed swap restores the old app and reopens it', () => {
  const env = widgetEnv();
  const good = makeDmg(env.dir, 'good');
  markExisting(env);
  writeFileSync(join(env.dir, 'running'), '');
  rmSync(join(env.bin, 'mv'));
  env.stub('mv', 'case "$1" in *.ClaudeSessions.app.new) exit 1 ;; esac\nexec /bin/mv "$@"');
  const r = runWidget(env, { sha: good.sha, dmg: good.dmg });
  assert.equal(r.code, 0, r.out);
  assert.match(r.out, /Skipped:\n\s+widget: cannot move the new app into place/);
  assert.ok(existsSync(join(installedApp(env), 'OLD')));
  assert.ok(!existsSync(join(env.home, 'Applications/.ClaudeSessions.app.old')));
  const calls = env.calls().split('\n');
  assert.ok(calls.findIndex((l) => l.startsWith('open ')) > calls.findIndex((l) => l.startsWith('osascript ')));
  assert.equal(calls.filter((l) => l.startsWith('open ')).length, 1);
});

mac('widget: SKILLS_WIDGET_DMG skips download and checksum', () => {
  const env = widgetEnv();
  const good = makeDmg(env.dir, 'good');
  const r = runWidget(env, { sha: '0'.repeat(64), dmg: good.dmg, local: true });
  assert.equal(r.code, 0, r.out);
  assert.match(r.out, /skipping download and checksum/);
  assert.ok(!/^curl /m.test(env.calls()));
  assert.ok(existsSync(join(installedApp(env), 'Contents/MacOS/ClaudeSessions')));
});

test('widget without a lock is skipped: no release pinned', () => {
  const home = mkdtempSync(join(tmpdir(), 'skills-install-'));
  for (const dry of [{}, { SKILLS_INSTALL_DRY_EXTERNAL: '1' }]) {
    const r = spawnSync('bash', [join(root, 'install.sh'), '--only', 'widget'], {
      env: { ...process.env, CLAUDE_HOME: home, ...dry, SKILLS_WIDGET_LOCK: join(home, 'no-such.lock') },
      encoding: 'utf8',
    });
    assert.equal(r.status, 0, r.stderr);
    assert.match(r.stdout, /widget skipped: no release pinned/);
    assert.match(r.stdout, /widget: no release pinned/);
    assert.ok(!/swift|build\.sh/.test(r.stdout), r.stdout);
  }
});
