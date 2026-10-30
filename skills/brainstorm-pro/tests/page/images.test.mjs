import { after, before, describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { existsSync, readFileSync } from 'node:fs';
import { launch, sleep } from '../harness/browser.mjs';
import { IMAGE_HELPERS, READY, STATUS } from '../harness/page-helpers.mjs';
import { FIXTURES, outDir, parseAnswers, render, serve } from '../harness/serve.mjs';
import { writeRound } from '../fixtures/round.mjs';

const dir = outDir('images');
let b;

before(async () => { b = await launch(); });
after(async () => { await b.close(); });
const status = () => b.status();
const thumbs = (sel) => b.eval(`document.querySelectorAll(${JSON.stringify(sel + ' figure.relative img')}).length`);
const pathsOf = (stdout) => [...stdout.matchAll(/^Ảnh #(\d+): (.+)$/gm)].map((m) => ({ n: +m[1], path: m[2] }));
const sendBtn = (re) => b.eval(`[...document.querySelectorAll('aside button')].find((x) => ${re}.test(x.textContent) && x.classList.contains('btn-primary')).click()`);
const sample = (path) => readFileSync(path).toString('base64');

describe('images on the interview page', () => {
  let s, url, p1;
  before(async () => {
    s = serve(writeRound(dir), dir + '/int/round.html');
    url = await s.url;
    await b.goto(url, { ready: READY.interview });
    await b.eval(IMAGE_HELPERS);
  });
  after(() => { s.kill(); });

  it('a paste puts a chip at the caret and shows a thumbnail that loads', async () => {
    await b.click('#q01-opt-a');
    await b.eval(`(() => { const t = document.querySelector('#q01-note'); t.value = 'ab'; t.dispatchEvent(new Event('input', { bubbles: true })); t.focus(); t.setSelectionRange(1, 1); })()`);
    p1 = await b.eval(`send('#q01-note', 'paste', 'red')`);
    assert.equal(p1.prevented, true);
    assert.equal(p1.value, 'a[Ảnh #1]b');
    assert.equal(await thumbs('#card-q01'), 1);
    await b.until(`document.querySelector('#card-q01 figure.relative img').naturalWidth === 40`);
    assert.equal(await b.eval(`document.querySelector('#card-q01 figure.relative img').title === 'Ảnh #1' && document.querySelector('#card-q01 figure.relative span').textContent === '#1'`), true);
  });

  it('a paste into Khác counts only while Khác is chosen', async () => {
    await b.click('#q01-opt-other');
    const p = await b.eval(`send('#q01-other', 'paste', 'green')`);
    assert.equal(p.value, '[Ảnh #2]');
    assert.equal(await thumbs('#card-q01'), 2);
    await b.click('#q01-opt-a');
    assert.equal(await thumbs('#card-q01'), 1, 'a hidden box adds no thumbnail');
  });

  it('a drop puts a chip in, and × removes the chip and the thumbnail', async () => {
    await b.click('#q02-opt-x');
    const p = await b.eval(`send('#q02-note', 'drop', 'blue')`);
    assert.equal(p.prevented, true);
    assert.equal(p.value, '[Ảnh #3]');
    assert.equal(await thumbs('#card-q02'), 1);
    await b.click('#card-q02 figure button');
    assert.equal(await b.eval(`document.querySelector('#q02-note').value`), '');
    assert.equal(await thumbs('#card-q02'), 0);
  });

  it('warns about a wrong type and an image over 20 MB, and adds nothing', async () => {
    await b.eval(`send('#q02-note', 'paste', 'red', 'image/heic')`);
    assert.equal(await status(), 'Chỉ nhận ảnh PNG, JPEG, GIF, WebP.');
    await b.eval(`send('#q02-note', 'paste', 'red', 'image/png', 21 * 1024 * 1024)`);
    assert.equal(await status(), 'Ảnh quá 20 MB.');
    assert.equal(await b.eval(`document.querySelector('#q02-note').value`), '');
  });

  it('a file dropped outside a text box is stopped with a warning', async () => {
    const r = await b.eval(`(() => { const dt = new DataTransfer(); dt.items.add(new File([new Uint8Array(3)], 'a.png', { type: 'image/png' }));
      const href = location.href; const over = new DragEvent('dragover', { dataTransfer: dt, bubbles: true, cancelable: true }); document.body.dispatchEvent(over);
      const ev = new DragEvent('drop', { dataTransfer: dt, bubbles: true, cancelable: true }); document.body.dispatchEvent(ev);
      return { over: over.defaultPrevented, prevented: ev.defaultPrevented, same: location.href === href, msg: ${STATUS} }; })()`);
    assert.deepEqual(r, { over: true, prevented: true, same: true, msg: 'Thả ảnh vào một ô nhập.' });
  });

  it('keeps chips and thumbnails over a reload, saving paths in the draft and no image data', async () => {
    await b.goto(url, { ready: READY.interview });
    await b.eval(IMAGE_HELPERS);
    const re = await b.eval(`({ note: document.querySelector('#q01-note').value, t1: document.querySelectorAll('#card-q01 figure.relative img').length,
      ls: localStorage.getItem('interview-draft:' + location.pathname) })`);
    assert.equal(re.note, 'a[Ảnh #1]b');
    assert.equal(re.t1, 1);
    await b.until(`document.querySelector('#card-q01 figure.relative img').naturalWidth === 40`);
    assert.doesNotMatch(re.ls, /data:image|iVBOR/);
    assert.match(re.ls, /"_img":\{/);
  });

  it('sends one Ảnh line, after its note, for the image still in the text', async () => {
    await sendBtn('/^Gửi/');
    const r = await s.done;
    assert.equal(r.code, 0);
    const ps = pathsOf(r.stdout);
    assert.deepEqual(ps.map((x) => x.n), [1], 'none for the removed or hidden ones');
    assert.match(r.stdout, /Ghi chú: a\[Ảnh #1\]b\nẢnh #1: /);
    assert.deepEqual(parseAnswers(r.stdout).lines.slice(0, 3), ['[q01] Which adder?', 'Chọn: A. Plain add', 'Ghi chú: a[Ảnh #1]b']);
    assert.ok(existsSync(ps[0].path));
    assert.equal(sample(ps[0].path), p1.b64, 'the saved file holds the pasted bytes');
    assert.deepEqual(b.unexpectedErrors(), []);
  });
});

describe('images on the document page', () => {
  let s, url;
  before(async () => {
    s = serve(FIXTURES + '/doc.md', dir + '/doc/review-01.html');
    url = await s.url;
    await b.goto(url, { ready: READY.review, doc: true });
    await b.eval(IMAGE_HELPERS);
  });
  after(() => { s.kill(); });

  const box = (t) => `[aria-label="Góp ý: ${t}"]`;
  const open = (t) => b.eval(`[...document.querySelectorAll('main h2')].find((h) => h.textContent === ${JSON.stringify(t)}).parentElement.querySelector('button').click()`);
  const got = {};

  it('takes pasted and dropped images into section notes and the general note', async () => {
    await open('Overview');
    await open('Design');
    got.a1 = await b.eval(`send(${JSON.stringify(box('Overview'))}, 'paste', 'red')`);
    got.a2 = await b.eval(`send(${JSON.stringify(box('Chung'))}, 'paste', 'green')`);
    got.a3 = await b.eval(`send(${JSON.stringify(box('Design'))}, 'drop', 'blue')`);
    assert.deepEqual([got.a1, got.a2, got.a3].map((x) => x.value), ['[Ảnh #1]', '[Ảnh #2]', '[Ảnh #3]']);
    assert.equal(await b.eval(`document.querySelectorAll('main figure img[title^="Ảnh"]').length`), 3);
  });

  it('restores them after a reload with the draft shape {n, h, img}', async () => {
    await b.goto(url, { ready: READY.review, doc: true });
    const re = await b.eval(`({ vals: [...document.querySelectorAll('main textarea')].map((t) => t.value).filter(Boolean), th: document.querySelectorAll('main figure img[title^="Ảnh"]').length,
      vis: [...document.querySelectorAll('main textarea')].filter((t) => t.value).every((t) => t.offsetParent), ls: localStorage.getItem('doc-draft:' + location.pathname) })`);
    assert.equal(re.vals.length, 3);
    assert.equal(re.th, 3);
    assert.equal(re.vis, true, 'boxes with text are open');
    assert.match(re.ls, /^\{"n":\{.*\},"h":\[\],"img":\{/);
  });

  it('sends the notes in document order, then the Ảnh lines in the same order', async () => {
    await sleep(200);
    await sendBtn('/^Gửi/');
    await b.choose('s');
    const r = await s.done;
    const ps = pathsOf(r.stdout);
    assert.deepEqual(ps.map((x) => x.n), [1, 3, 2]);
    assert.equal(ps.every((x) => sample(x.path) === got['a' + x.n].b64), true, 'bytes equal');
    assert.deepEqual(parseAnswers(r.stdout).lines.slice(0, 4), ['Quyết định: Sửa', '[§ Overview] [Ảnh #1]', '[§ Design] [Ảnh #3]', '[§ Chung] [Ảnh #2]']);
    assert.match(r.stdout, /\[§ Chung\] \[Ảnh #2\]\nẢnh #1: /);
  });
});

describe('images on a file page', () => {
  it('warns that nothing can be saved, adds no chip and blocks only a drop', async () => {
    const out = dir + '/file/round.html';
    assert.equal(render(writeRound(dir), out).code, 0);
    await b.goto('file://' + out, { ready: `!!document.querySelector('#card-q02')` });
    await b.eval(IMAGE_HELPERS);
    const p = await b.eval(`send('#q01-note', 'paste', 'red')`);
    assert.deepEqual({ prevented: p.prevented, value: p.value }, { prevented: false, value: '' });
    assert.equal(await status(), 'Trang mở dạng file nên không gửi được ảnh: dán ảnh thẳng vào terminal Claude.');
    assert.equal(await thumbs('main'), 0);
    const d = await b.eval(`send('#q01-note', 'drop', 'red')`);
    assert.deepEqual({ prevented: d.prevented, value: d.value }, { prevented: true, value: '' });
    assert.deepEqual(b.unexpectedErrors(), []);
  });
});
