import { after, before, describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { launch, sleep } from '../harness/browser.mjs';
import { PASSAGE_HELPERS, IMAGE_HELPERS } from '../harness/page-helpers.mjs';
import { FIXTURES, outDir, parseAnswers, serve } from '../harness/serve.mjs';

const dir = outDir('passages');
const DOC = FIXTURES + '/doc.md';
const lines = readFileSync(DOC, 'utf8').split('\n');
const lineOf = (s) => lines.findIndex((l) => l.includes(s)) + 1;
const fenceOf = (s) => { let a = lineOf(s); while (!lines[a - 1].startsWith('```')) a--; let z = lineOf(s); while (!lines[z - 1].startsWith('```')) z++; return a + '-' + z; };
const ready = `document.querySelectorAll('main > article').length >= 5 && !!document.querySelector('[aria-label="Góp ý: Chung"]')
  && (!!document.querySelector('main [data-mmd] svg') || !!document.querySelector('main [data-mmd] .alert'))`;
let b, s, url;

const load = async (name) => {
  s = serve(DOC, `${dir}/${name}/review-01.html`);
  url = await s.url;
  await b.goto(url, { ready, doc: true });
  await b.eval(PASSAGE_HELPERS + IMAGE_HELPERS);
};
const adder = () => b.eval('T.adder()');
// A selection or press that must NOT show the adder cannot be waited for, so negatives get a settle time past the page's 100 ms timer.
const settle = () => sleep(350);
const clearSelection = async () => { await b.eval('getSelection().removeAllRanges()'); await b.until(`T.adder() === 'hidden'`); };
// Selects a phrase (programmatically), clicks the adder with the mouse, returns the new comment's id.
const comment = async (a, bb, text) => {
  const before = await b.eval('T.boxes()');
  await b.eval(`T.select(${JSON.stringify(a)}, ${JSON.stringify(bb || null)})`);
  await b.until(`T.adder() === 'shown'`, { timeout: 3000, what: 'adder for ' + a });
  await b.click('body > button.absolute:not([hidden])');
  const id = await b.until(`T.boxes().find((x) => ${JSON.stringify(before)}.indexOf(x) < 0)`);
  if (text != null) await b.eval(`T.type(${JSON.stringify(id)}, ${JSON.stringify(text)})`);
  return id;
};
const marks = (id) => b.eval(`T.marks(${JSON.stringify(id)})`);
const boxes = () => b.eval('T.boxes()');
const remove = (id) => b.eval(`[...document.querySelector('[data-hl="${id}"]').querySelectorAll('button')].find((x) => x.textContent === 'Xóa').click()`);
const ids = {};

describe('highlight comments on the review page', () => {
  before(async () => { b = await launch(); await load('main'); });
  after(async () => { s.kill(); await b.close(); });

  it('every class declared for later-built elements has a rule right after load', async () => {
    const r = await b.eval(`(async () => {
      const d = [...document.querySelectorAll('body > div[hidden]')].find((x) => x.classList.contains('bg-hl'));
      const list = [...d.classList];
      const tw = [...document.querySelectorAll('style')].map((x) => x.textContent).join('\\n');
      const daisy = await (await fetch('https://cdn.jsdelivr.net/npm/daisyui@5.7.47/daisyui.css')).text();
      const has = (css, c) => css.includes('.' + CSS.escape(c));
      return { n: list.length, missing: list.filter((c) => !has(tw, c) && !has(daisy, c)) };
    })()`);
    assert.ok(r.n > 0, 'the declaring element exists');
    assert.deepEqual(r.missing, []);
  });

  it('the first comment makes Tailwind rewrite nothing, and the box sits in the bullet and takes focus', async () => {
    // Tailwind may still be settling after load; wait for a quiet stretch, then count from zero.
    let n;
    do { n = await b.twWrites(); await sleep(500); } while (n !== (await b.twWrites()));
    await b.twReset();
    ids.bullet = await comment('second goal bullet');
    assert.equal(await b.twWrites(), 0, 'the first comment must not rebuild the stylesheet');
    assert.equal(await b.eval(`!!document.querySelector('[data-hl="${ids.bullet}"]').closest('li')`), true);
    assert.equal(await b.eval(`document.activeElement === T.ta(${JSON.stringify(ids.bullet)})`), true);
    assert.equal(await marks(ids.bullet), 1);
    assert.equal(await adder(), 'hidden');
    assert.equal(await b.eval(`getSelection().isCollapsed`), true);
    await b.eval(`T.type(${JSON.stringify(ids.bullet)}, 'c-bullet')`);
  });

  it('the first image paste into a comment makes Tailwind rewrite nothing', async () => {
    await b.twReset();
    const r = await b.eval(`send('[data-hl="${ids.bullet}"] textarea', 'paste', 'red')`);
    assert.equal(r.value, 'c-bullet[Ảnh #1]'.replace('c-bullet', r.value.slice(0, 8)), r.value);
    await b.until(`document.querySelectorAll('[data-hl="${ids.bullet}"] figure img[title="Ảnh #1"]').length === 1`);
    await sleep(500);
    assert.equal(await b.twWrites(), 0, 'the first image must not rebuild the stylesheet');
    assert.deepEqual(b.unexpectedErrors(), []);
  });

  it('keeps the adder hidden while the mouse is held and shows it after release', async () => {
    await clearSelection();
    const p = await b.eval(`T.points('third goal bullet')`);
    let mid;
    await b.drag(p.from, p.to, { steps: 12, pauseMs: 250, onPause: async () => { mid = await adder(); } });
    assert.equal(mid, 'hidden', 'no adder mid-drag, even after a pause');
    await b.until(`T.adder() === 'shown'`, { timeout: 1500 });
    assert.equal(await b.eval(`String(getSelection()).length > 5`), true);
  });

  it('the adder travels with the text when the page scrolls', async () => {
    const gap = () => b.eval(`(() => { const a = [...document.querySelectorAll('body > button.absolute')].find((x) => !x.hidden); const r = getSelection().getRangeAt(0).getClientRects(); const l = r[r.length - 1]; return { gap: Math.round(a.getBoundingClientRect().top - l.bottom), y: scrollY }; })()`);
    const before = await gap();
    await b.send('Input.dispatchMouseEvent', { type: 'mouseWheel', x: 700, y: 450, deltaX: 0, deltaY: 120 });
    await b.until(`scrollY !== ${before.y}`);
    await sleep(300);
    const after = await gap();
    assert.notEqual(after.y, before.y);
    assert.ok(Math.abs(after.gap - before.gap) <= 1, `gap ${before.gap} -> ${after.gap}`);
    assert.equal(await adder(), 'shown');
  });

  it('a mouse press elsewhere hides the adder; a right-click keeps it and a later selection change shows it', async () => {
    await b.mouse('mousePressed', 5, 450, { clickCount: 1 });
    assert.equal(await adder(), 'hidden', 'hidden at once on mousedown');
    await b.mouse('mouseReleased', 5, 450, { clickCount: 1 });
    await settle();
    assert.equal(await adder(), 'hidden');
    await clearSelection();
    const p = await b.eval(`T.points('numbered one is out')`);
    await b.drag(p.from, p.to, { steps: 6 });
    await b.until(`T.adder() === 'shown'`, { timeout: 1500 });
    await b.send('Input.dispatchMouseEvent', { type: 'mousePressed', x: 5, y: 450, button: 'right', clickCount: 1 });
    assert.equal(await adder(), 'shown', 'a right-click does not hide it');
    await b.eval(`T.select('numbered two is also')`);
    await b.until(`T.adder() === 'shown'`, { timeout: 1500 });
    await b.send('Input.dispatchMouseEvent', { type: 'mouseReleased', x: 5, y: 450, button: 'right', clickCount: 1 });
    await clearSelection();
  });

  it('a keyboard selection moves the adder to the end of the new selection', async () => {
    await b.eval(`(() => { const [t, i] = T.node('numbered two is also'); t.parentElement.scrollIntoView({ block: 'center' });
      document.activeElement && document.activeElement.blur(); const r = document.createRange(); r.setStart(t, i + 3); r.setEnd(t, i + 4);
      getSelection().removeAllRanges(); getSelection().addRange(r); })()`);
    const left = () => b.eval(`parseFloat([...document.querySelectorAll('body > button.absolute')].find((x) => !x.hidden).style.left)`);
    await b.until(`T.adder() === 'shown'`, { timeout: 2000 });
    const before = await left();
    for (let i = 0; i < 5; i++) { await b.press('ArrowRight', { shift: true }); await sleep(30); }
    assert.equal(await b.eval(`String(getSelection()).length`), 6);
    await b.until(`T.adder() === 'shown'`, { timeout: 2000 });
    await b.until(`parseFloat([...document.querySelectorAll('body > button.absolute')].find((x) => !x.hidden).style.left) > ${before + 10}`, { timeout: 2000, what: 'adder moved right' });
    await clearSelection();
  });

  it('puts a comment under its bullet, after its table, code block or paragraph, with the section key and the lines', async () => {
    ids.table = await comment('second role', null, 'c-table');
    assert.equal(await b.eval(`!!document.querySelector('[data-hl="${ids.table}"]').previousElementSibling.querySelector('table')`), true);
    ids.code = await comment('second =', null, 'c-code');
    assert.equal(await b.eval(`document.querySelector('[data-hl="${ids.code}"]').previousElementSibling.tagName`), 'FIGURE');
    ids.para = await comment('paragraph under the components', null, 'c-para');
    assert.equal(await b.eval(`document.querySelector('[data-hl="${ids.para}"]').previousElementSibling.tagName`), 'DIV');
    ids.long = await comment('long risk paragraph', 'quoted back to Claude', 'c-long');
    assert.ok((await marks(ids.long)) > 2, 'a selection over code and bold gets several marks');
    ids.cross = await comment('first open question', 'second open question', 'c-cross');
    // A section note on Risks.
    await b.eval(`[...document.querySelectorAll('main h2')].find((h) => h.textContent === 'Risks').parentElement.querySelector('button').click()`);
    await b.until(`document.activeElement.getAttribute('aria-label') === 'Góp ý: Risks'`);
    await b.type('c-section');
  });

  it('a comment box stacks after the ones already there for the same block', async () => {
    const id = await comment('paragraph under the components', null, 'c-second');
    assert.equal(await b.eval(`document.querySelector('[data-hl="${id}"]').previousElementSibling.dataset.hl`), ids.para);
    ids.stacked = id;
  });

  it('takes no comment on a selection inside a textarea, the general card or a diagram', async () => {
    await b.eval(`(() => { const ta = document.querySelector('[aria-label="Góp ý: Chung"]'); ta.value = 'hello world'; ta.focus(); ta.setSelectionRange(0, 5); })()`);
    await settle();
    assert.equal(await adder(), 'hidden', 'textarea');
    await b.eval(`(() => { const t = [...document.querySelectorAll('[data-note] h2')].find((h) => h.textContent === 'Góp ý chung').firstChild; const r = document.createRange(); r.setStart(t, 0); r.setEnd(t, 5); getSelection().removeAllRanges(); getSelection().addRange(r); })()`);
    await settle();
    assert.equal(await adder(), 'hidden', 'general card');
    const picked = await b.eval(`(() => { const t = document.querySelector('[data-mmd] svg'); if (!t) return false; const r = document.createRange(); r.selectNodeContents(t); getSelection().removeAllRanges(); getSelection().addRange(r); return true; })()`);
    await settle();
    assert.equal(await adder(), 'hidden', 'mermaid drawing' + (picked ? '' : ' (not drawn)'));
    await b.eval(`(() => { const ta = document.querySelector('[aria-label="Góp ý: Chung"]'); ta.value = 'c-general'; ta.dispatchEvent(new Event('input', { bubbles: true })); getSelection().removeAllRanges(); })()`);
  });

  it('removes an empty comment on blur, on Esc and with Xóa, and keeps inner marks when an outer comment goes', async () => {
    const e1 = await comment('plain words so that');
    await b.eval(`document.querySelector('[aria-label="Góp ý: Chung"]').focus()`);
    await b.until(`!T.boxes().includes(${JSON.stringify(e1)})`);
    assert.equal(await marks(e1), 0, 'blur removes the box and its marks');
    const e2 = await comment('is cut when quoted');
    await b.press('Escape');
    await b.until(`!T.boxes().includes(${JSON.stringify(e2)})`);
    assert.equal(await marks(e2), 0, 'Esc removes it too');
    const e3 = await comment('numbered one is out of scope', null, 'to delete');
    await remove(e3);
    assert.equal((await boxes()).includes(e3), false);
    assert.equal(await marks(e3), 0, 'Xóa removes box and marks');
    const inner = await comment('bullet with other', null, 'c-inner');
    const outer = await comment('third goal', 'terms', 'c-outer');
    assert.ok(outer, 'an overlapping comment is created');
    await remove(outer);
    assert.equal(await marks(outer), 0);
    assert.ok((await marks(inner)) >= 1, 'the inner comment keeps its marks');
    await remove(inner);
  });

  it('clicking a mark focuses its comment and lights its marks', async () => {
    await b.eval('getSelection().removeAllRanges()');
    await b.click(`mark[data-c="${ids.table}"]`);
    assert.equal(await b.eval(`document.activeElement === T.ta(${JSON.stringify(ids.table)})`), true);
    assert.equal(await b.eval(`[...document.querySelectorAll('mark[data-c="${ids.table}"]')].every((m) => m.dataset.active === 'true')`), true);
    assert.equal(await b.eval(`document.querySelector('aside span').textContent`), '9 góp ý');
  });

  it('brings comments, marks, images and the section note back after a reload', async () => {
    const state = () => b.eval(`T.boxes().map((id) => T.ta(id).value + ':' + T.marks(id)).sort().join('|')`);
    const before = await state();
    await b.goto(url, { ready, doc: true });
    await b.eval(PASSAGE_HELPERS + IMAGE_HELPERS);
    assert.equal(await state(), before);
    assert.equal(await b.eval(`document.querySelector('[aria-label="Góp ý: Risks"]').value`), 'c-section');
    assert.equal(await b.eval(`document.querySelectorAll('figure img[title="Ảnh #1"]').length`), 1);
    assert.equal(await b.eval(`document.querySelector('aside p[role=status]').textContent`), 'Đã khôi phục góp ý đã lưu.');
  });

  it('sends every comment with its section, lines and quote on Cmd+Enter', async () => {
    await b.press('Enter', { meta: true });
    const r = await s.done;
    assert.equal(r.code, 0);
    const out = parseAnswers(r.stdout).lines;
    const quoteLong = /«(.*)»/.exec(out[7] || '');
    assert.deepEqual(out.slice(0, 7), [
      'Quyết định: Sửa',
      `[§ Overview › Goals · dòng ${lineOf('second goal bullet')}] «second goal bullet» → c-bullet[Ảnh #1]`,
      `[§ Design · dòng ${lineOf('| beta')}] «second role» → c-table`,
      `[§ Design · dòng ${fenceOf('const second')}] «second =» → c-code`,
      `[§ Design › Components · dòng ${lineOf('A paragraph under')}] «paragraph under the components» → c-para`,
      `[§ Design › Components · dòng ${lineOf('A paragraph under')}] «paragraph under the components» → c-second`,
      '[§ Risks] c-section',
    ]);
    assert.ok(out[7].startsWith(`[§ Risks · dòng ${lineOf('A long risk paragraph')}] «`), out[7]);
    assert.ok(quoteLong && quoteLong[1].length <= 95 && /^long risk paragraph with .+ … .+ to Claude$/.test(quoteLong[1]), 'a long quote is cut as head … tail: ' + (quoteLong && quoteLong[1]));
    assert.ok(out[7].endsWith('→ c-long'));
    assert.equal(out[8], `[§ Open · dòng ${lineOf('The first open question')}-${lineOf('The second open question')}] «first open question stays here. The second open question» → c-cross`);
    assert.equal(out[9], '[§ Chung] c-general');
    assert.match(out[10], /^Ảnh #1: .+\/passages\/main\/images\/review-01-1\.png$/);
    assert.equal(out.length, 11);
    assert.equal(out.some((l) => l.includes('c-outer')), false);
  });

  it('warns when a saved highlight no longer matches the document', async () => {
    await load('lost');
    const id = await comment('first goal bullet', null, 'will be lost');
    assert.ok(id);
    await b.eval(`(() => { const k = 'doc-draft:' + location.pathname; const d = JSON.parse(localStorage.getItem(k)); d.h[0].q = 'text the document does not have'; localStorage.setItem(k, JSON.stringify(d)); })()`);
    await b.goto(url, { ready, doc: true });
    assert.equal(await b.eval(`document.querySelector('aside p[role=status]').textContent`), 'Không khôi phục được 1 góp ý tô đen (tài liệu đã đổi).');
    assert.equal(await b.eval('T === undefined').catch(() => true), true);
    s.kill();
  });

  it('Duyệt sends the decision together with the comments', async () => {
    await load('approve');
    await comment('first goal bullet', null, 'small one');
    await b.eval(`[...document.querySelectorAll('aside button')].find((x) => x.textContent === 'Duyệt').click()`);
    const r = await s.done;
    assert.deepEqual(parseAnswers(r.stdout).lines, ['Quyết định: Duyệt', `[§ Overview › Goals · dòng ${lineOf('first goal bullet')}] «first goal bullet» → small one`]);
  });
});
