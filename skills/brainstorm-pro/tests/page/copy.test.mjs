import { after, before, describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { launch } from '../harness/browser.mjs';
import { COPY_OK, GUARDED, READY } from '../harness/page-helpers.mjs';
import { FIXTURES, outDir, render, serve } from '../harness/serve.mjs';
import { writeRound } from '../fixtures/round.mjs';

const dir = outDir('copy');
let b, s;

describe('code block Copy button', () => {
  before(async () => { b = await launch(); });
  after(async () => { s && s.kill(); await b.close(); });

  it('interview: copies a context code block and an option code block verbatim', async () => {
    const ctx = 'const ctx = 1;\nconst more = 2;';
    const opt = 'const picked = "b";\nreturn picked;';
    s = serve(writeRound(dir, (d) => {
      d.questions[0].context.push({ t: 'code', label: 'Ctx block', text: ctx, lang: 'ts' });
      d.questions[0].options[0].code = { text: opt, lang: 'ts' };
    }), dir + '/round.html');
    await b.goto(await s.url, { ready: READY.interview });
    await b.eval(COPY_OK);
    await b.twSettle();
    const copies = await b.eval(`[...document.querySelectorAll('#card-q01 figure figcaption button')].filter((x) => x.textContent === 'Copy').length`);
    assert.equal(copies, 3, 'file block, context block and option block');
    for (const [text, label] of [[ctx, 'Ctx block'], [opt, '']]) {
      const btn = `[...document.querySelectorAll('#card-q01 figure')].filter((f) => ${label ? `f.querySelector('figcaption').textContent.startsWith(${JSON.stringify(label)})` : `f.closest('[style*="grid-row"]')`})[0].querySelector('figcaption button:last-child')`;
      await b.eval('window.__copied = null');
      await b.eval(`${btn}.click()`);
      await b.until('window.__copied !== null');
      assert.equal(await b.eval('window.__copied'), text);
      assert.equal(await b.eval(`${btn}.textContent`), 'Đã copy');
    }
    assert.equal(await b.twWrites(), 0);
    await b.until(`[...document.querySelectorAll('#card-q01 figure figcaption button')].filter((x) => /copy/i.test(x.textContent)).every((x) => x.textContent === 'Copy')`, { timeout: 4000 });
  });

  it('served review: Copy leaves the leave guard on', async () => {
    s.kill();
    const out = dir + '/review/review-01.html';
    s = serve(FIXTURES + '/doc.md', out);
    await b.goto(await s.url, { ready: READY.review, doc: true });
    await b.eval(COPY_OK);
    await b.click('#topic');
    assert.equal(await b.eval(GUARDED), true);
    await b.click('main figure:not([data-mmd]) figcaption button');
    await b.until(`window.__copied !== null`);
    assert.equal(await b.eval(GUARDED), true);
    assert.equal(await b.eval('window.__closed'), 0);
  });

  it('document: a fence copies its text with no trailing newline, and keeps the selection', async () => {
    const out = dir + '/doc/page.html';
    assert.equal(render(FIXTURES + '/doc.md', out, ['--open']).code, 0);
    await b.goto('file://' + out, { ready: READY.doc, doc: true });
    await b.eval(COPY_OK);
    await b.twSettle();
    await b.eval(`(() => { const t = document.querySelector('main p'); const r = document.createRange(); r.selectNodeContents(t); const s = getSelection(); s.removeAllRanges(); s.addRange(r); window.__sel = String(s); })()`);
    const sel = await b.eval('window.__sel');
    assert.ok(sel.length > 0);
    await b.click('main figure:not([data-mmd]) figcaption button');
    await b.until(`window.__copied !== null`);
    assert.equal(await b.eval('window.__copied'), 'const first = 1;\nconst second = 2;');
    assert.equal(await b.eval('String(getSelection())'), sel);
    assert.equal(await b.eval(`document.querySelector('main figure:not([data-mmd]) figcaption button').textContent`), 'Đã copy');
    assert.equal(await b.eval(`document.querySelectorAll('main figure[data-mmd] figcaption button').length`), 0);
    assert.equal(await b.twWrites(), 0);
  });

  it('a failed copy reads Không copy được', async () => {
    const out = dir + '/failed/page.html';
    assert.equal(render(FIXTURES + '/doc.md', out, ['--open']).code, 0);
    await b.goto('file://' + out, { ready: READY.doc, doc: true });
    await b.eval(`navigator.clipboard.writeText = async () => { throw new Error('no'); }; document.execCommand = () => false`);
    await b.click('main figure:not([data-mmd]) figcaption button');
    await b.until(`document.querySelector('main figure:not([data-mmd]) figcaption button').textContent === 'Không copy được'`);
  });

  it('the textarea fallback keeps the selection', async () => {
    await b.eval(`navigator.clipboard.writeText = async () => { throw new Error('no'); }; document.execCommand = () => true;
      (() => { const r = document.createRange(); r.selectNodeContents(document.querySelector('main p')); const s = getSelection(); s.removeAllRanges(); s.addRange(r); window.__sel = String(s); })()`);
    const sel = await b.eval('window.__sel');
    assert.ok(sel.length > 0);
    await b.click('main figure:not([data-mmd]) figcaption button');
    await b.until(`document.querySelector('main figure:not([data-mmd]) figcaption button').textContent === 'Đã copy'`);
    assert.equal(await b.eval('String(getSelection())'), sel);
  });
});
