import { after, before, describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { FIXTURES, outDir, parseAnswers, render, serve } from '../harness/serve.mjs';
import { writeRound } from '../fixtures/round.mjs';

const dir = outDir('server');
const png = readFileSync(FIXTURES + '/img.png');
const caseDir = (name) => { const d = join(dir, 'cases', name); mkdirSync(d, { recursive: true }); return d; };

describe('the served interview page', () => {
  let s, origin, base, round;
  const H = (type, o = origin) => ({ 'Content-Type': type, ...(o ? { Origin: o } : {}) });
  const post = (p, type, body, o) => fetch(origin + base + p, { method: 'POST', headers: H(type, o), body });
  // A rejected oversized body may reach the client as a status or as a reset connection, depending on timing.
  const statusOf = (p) => p.then((r) => r.status, () => 'reset');

  before(async () => {
    const d = caseDir('serve');
    round = writeRound(d);
    s = serve(round, d + '/page.html');
    const url = new URL(await s.url);
    origin = url.origin;
    base = url.pathname;
  });
  after(() => s.kill());

  it('serves the page with its endpoints and keeps them out of the written file', async () => {
    const page = await (await fetch(origin + base)).text();
    assert.ok(page.includes(JSON.stringify(base + 'answers')) && page.includes(JSON.stringify(base + 'image')));
    assert.equal(readFileSync(join(dir, 'cases/serve/page.html'), 'utf8').includes('"upload"'), false);
    assert.equal((await fetch(origin + base + 'nope')).status, 404);
    assert.equal((await fetch(origin + '/' + 'a'.repeat(32) + '/')).status, 404, 'a wrong token');
    assert.equal((await post('nope', 'application/json', '{}')).status, 404);
  });

  it('saves an upload beside the page and serves it back', async () => {
    const r = await post('image', 'image/png', png);
    assert.equal(r.status, 200);
    const j = await r.json();
    assert.deepEqual(j, { n: 1, path: join(dir, 'cases/serve/images/page-1.png'), url: base + 'images/page-1.png' });
    assert.ok(readFileSync(j.path).equals(png));
    assert.equal((await post('image', 'image/jpeg; charset=x', png)).status, 200, 'a content type with parameters');
    const g = await fetch(origin + j.url);
    assert.equal(g.status, 200);
    assert.equal(g.headers.get('content-type'), 'image/png');
    assert.equal(g.headers.get('cache-control'), 'no-store');
    assert.ok(Buffer.from(await g.arrayBuffer()).equals(png));
    assert.equal((await fetch(origin + base + 'images/nope.png')).status, 404);
    for (const n of ['..%2Fpage.html', '../page.html', '..%2F..%2Fetc%2Fpasswd', '']) assert.equal((await fetch(origin + base + 'images/' + n)).status, 404, 'traversal ' + n);
  });

  it('refuses a wrong origin, a wrong type, an empty body and one that is too big', async () => {
    assert.equal((await post('image', 'image/png', png, 'http://evil.example')).status, 403);
    assert.equal((await post('image', 'image/png', png, null)).status, 403, 'no Origin header');
    assert.equal((await post('answers', 'application/json', '{"text":"x"}', 'http://evil.example')).status, 403);
    assert.equal((await post('image', 'text/plain', png)).status, 415);
    assert.equal((await post('image', 'image/svg+xml', png)).status, 415);
    assert.equal((await post('image', 'image/png', Buffer.alloc(0))).status, 400);
    assert.equal(await statusOf(post('image', 'image/png', Buffer.alloc(21 * 1024 * 1024))), 413);
    assert.equal((await post('answers', 'text/plain', '{"text":"x"}')).status, 415);
    assert.equal((await post('answers', 'application/json', '')).status, 400);
    assert.equal((await post('answers', 'application/json', 'not json')).status, 400);
    assert.equal((await post('answers', 'application/json', '{"text":"   "}')).status, 400);
    assert.equal((await post('answers', 'application/json', '{"text":5}')).status, 400);
    assert.equal(await statusOf(post('answers', 'application/json', JSON.stringify({ text: 'x'.repeat(1024 * 1024 + 10) }))), 413);
  });

  it('prints ANSWERS <source> and the text, then exits 0', async () => {
    const r = await post('answers', 'application/json', JSON.stringify({ text: '  [q01] Which?\nChọn: A. Plain add\n ' }));
    assert.equal(r.status, 200);
    assert.deepEqual(await r.json(), { ok: true });
    const done = await s.done;
    assert.equal(done.code, 0);
    assert.equal(done.stdout, `${join(dir, 'cases/serve/page.html')}\nServing ${origin + base}\nANSWERS ${round}\n[q01] Which?\nChọn: A. Plain add\n`);
  });
});

describe('the served document page', () => {
  it('serves a review page and prints ANSWERS <document> the same way', async () => {
    const d = caseDir('doc');
    const s = serve(FIXTURES + '/doc.md', d + '/review-01.html');
    const url = new URL(await s.url);
    const page = await (await fetch(url.href)).text();
    assert.ok(page.includes('"review":true'));
    assert.ok(readFileSync(d + '/review-01.html', 'utf8').includes('"review":true'), 'the written file is the review page too');
    const r = await fetch(url.href + 'answers', { method: 'POST', headers: { 'Content-Type': 'application/json', Origin: url.origin }, body: JSON.stringify({ text: 'Quyết định: Duyệt' }) });
    assert.equal(r.status, 200);
    const done = await s.done;
    assert.equal(done.code, 0);
    assert.deepEqual(parseAnswers(done.stdout), { src: FIXTURES + '/doc.md', lines: ['Quyết định: Duyệt'] });
  });

  it('--open writes a read-only page and exits 0; a failing open exits 4', () => {
    const d = caseDir('open');
    const r = render(FIXTURES + '/doc.md', d + '/page.html', ['--open']);
    assert.equal(r.code, 0, r.stderr);
    assert.equal(r.stdout, d + '/page.html\n');
    assert.ok(readFileSync(d + '/page.html', 'utf8').includes('"review":false'));
    const f = render(FIXTURES + '/doc.md', d + '/fail.html', ['--open'], { bin: 'failbin' });
    assert.equal(f.code, 4);
    assert.match(f.stderr, /Could not open a browser/);
    const g = render(FIXTURES + '/doc.md', d + '/fail2.html', ['--serve'], { bin: 'failbin' });
    assert.equal(g.code, 4);
    assert.match(g.stdout, /^.*fail2\.html\nServing http:\/\/127\.0\.0\.1:\d+\/[0-9a-f]{32}\/\n$/);
  });
});

describe('render-interview.mjs on bad input', () => {
  const USAGE = 'Usage: node render-interview.mjs <round.json | document.md> <out.html> [--serve | --open]';

  it('prints the usage and exits 1 without a source, an output or with both --serve and --open', () => {
    const d = caseDir('usage');
    for (const [src, out, args] of [[null, null, []], [FIXTURES + '/doc.md', null, []], [FIXTURES + '/doc.md', d + '/x.html', ['--serve', '--open']]]) {
      const r = render(src, out, args);
      assert.equal(r.code, 1);
      assert.equal(r.stderr.trim(), USAGE);
      assert.equal(r.stdout, '');
    }
  });

  it('writes the page and prints its path for a good round', () => {
    const d = caseDir('good');
    const r = render(writeRound(d), d + '/out/page.html');
    assert.equal(r.code, 0, r.stderr);
    assert.equal(r.stdout, d + '/out/page.html\n');
    const html = readFileSync(d + '/out/page.html', 'utf8');
    assert.ok(html.includes('"Fixture round"') && !html.includes('/*__INTERVIEW_DATA__*/'));
    assert.equal(html.includes('"send"'), false);
  });

  const big = join(dir, 'cases', 'big.png');
  const cases = {
    'empty questions': [(x) => { x.questions = []; }, '"questions" must be a non-empty array'],
    'missing title': [(x) => { delete x.questions[0].title; }, 'questions[0] is missing "title"'],
    'missing options': [(x) => { delete x.questions[1].options; }, 'questions[1] is missing "options"'],
    'two recommended': [(x) => { x.questions[0].options[0].recommended = true; }, 'questions[0] marks 2 options "recommended"; mark one'],
    'recommendation without a recommended option': [(x) => { delete x.questions[0].options[1].recommended; }, 'questions[0] has "recommendation" but no option with "recommended": true'],
    'relative root': [(x) => { x.root = 'fixtures'; }, '"root" must be an absolute path: fixtures'],
    'relative code file': [(x) => { x.questions[0].context[1].file = 'fixtures/code.ts'; }, 'code "file" must be an absolute path: fixtures/code.ts'],
    'lines outside the file': [(x) => { x.questions[0].context[1].lines = [1, 99]; }, 'lines 1-99 are outside'],
    'code without lines or text': [(x) => { delete x.questions[0].context[1].lines; }, 'needs "lines" or "text"'],
    'image without a file': [(x) => { delete x.questions[0].context[4].file; }, 'img block needs "file"'],
    'image of a wrong type': [(x) => { x.questions[0].context[4].file = 'fixtures/code.ts'; }, 'is not an image the page shows'],
    'image over 3 MB': [(x) => { x.questions[0].context[4].file = big; }, 'is 3.1 MB, over 3 MB; resize a copy first'],
  };
  for (const [name, [mutate, message]] of Object.entries(cases)) {
    it('rejects ' + name, () => {
      mkdirSync(join(dir, 'cases'), { recursive: true });
      if (!existsSync(big)) writeFileSync(big, Buffer.alloc(3.1 * 1048576));
      const d = caseDir(name.replace(/\W+/g, '-'));
      const r = render(writeRound(d, mutate), d + '/page.html');
      assert.equal(r.code, 1);
      assert.ok(r.stderr.includes(message), r.stderr);
      assert.equal(existsSync(d + '/page.html'), false, 'no page is written');
    });
  }
});
