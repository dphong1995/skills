import { after, before, describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { launch } from '../harness/browser.mjs';
import { hint, READY, STATUS } from '../harness/page-helpers.mjs';
import { FIXTURES, outDir, serveRound2, serve } from '../harness/serve.mjs';

const dir = outDir('doc-review');
const DOC = FIXTURES + '/doc.md';
const ready = READY.review;
let b, s;
const heading = (t) => `[...document.querySelectorAll('main h2, main h3')].find((h) => h.textContent === ${JSON.stringify(t)})`;
// Serves a review into its own directory (the round state lives beside the page) and opens it.
const open = async (name, src = DOC, round = 1) => {
  s = serve(src, `${dir}/${name}/review-0${round}.html`);
  await b.goto(await s.url, { ready, doc: true });
};
const note = async (title, text) => {
  await b.eval(`${heading(title)}.parentElement.querySelector('button').click()`);
  await b.until(`document.activeElement.tagName === 'TEXTAREA'`);
  await b.type(text);
};
const general = async (text) => { await b.eval(`document.querySelector('[aria-label="Góp ý: Chung"]').focus()`); await b.type(text); };
const bar = (label) => `[...document.querySelectorAll('aside button')].find((x) => x.textContent.startsWith(${JSON.stringify(label)}))`;

describe('document review page', () => {
  before(async () => { b = await launch(); });
  after(async () => { s && s.kill(); await b.close(); });

  it('shows the review header, heading buttons and the bar', async () => {
    await open('head');
    const r = await b.eval(`({
      title: document.title, badge: document.querySelector('header .badge').textContent,
      buttons: [...document.querySelectorAll('main button')].filter((x) => x.textContent === 'Góp ý').length,
      hidden: [...document.querySelectorAll('main [data-note]')].filter((x) => x.hidden).length,
      general: [...document.querySelectorAll('main > article h2')].map((h) => h.textContent).pop(), hint: document.querySelector('main > p').textContent,
      bar: [...document.querySelectorAll('aside button')].map((x) => x.textContent), count: document.querySelector('aside span').textContent,
      banner: !!document.querySelector('main > .alert'), nav: document.querySelectorAll('#qnav a').length,
    })`);
    assert.equal(r.title, 'Duyệt · Sample Spec');
    assert.equal(r.badge, 'Chờ duyệt · lần 1');
    assert.equal(r.buttons, 7, 'four ## and three ### headings');
    assert.equal(r.hidden, 7);
    assert.equal(r.general, 'Góp ý chung');
    assert.match(r.hint, /^Tô đen một đoạn rồi bấm Góp ý để góp ý ngay chỗ đó; (⌘ Enter|Ctrl Enter) gửi tất cả · \? phím tắt\.$/);
    assert.deepEqual(r.bar.map(hint), ['DuyệtCHORD', 'Gửi góp ýMOD']);
    assert.equal(r.count, '0 góp ý');
    assert.equal(r.banner, false);
    assert.equal(r.nav, 4);
    s.kill();
  });

  it('a heading button opens its box, ticks the button and counts the note', async () => {
    await open('toggle');
    await b.eval(`${heading('Design')}.parentElement.querySelector('button').click()`);
    await b.until(`document.activeElement.getAttribute('aria-label') === 'Góp ý: Design'`);
    await b.type('add a diagram');
    assert.equal(await b.eval(`${heading('Design')}.parentElement.querySelector('button').textContent`), 'Góp ý ✓');
    assert.equal(await b.eval(`document.querySelector('aside span').textContent`), '1 góp ý');
    await b.eval(`document.activeElement.blur(); ${heading('Design')}.parentElement.querySelector('button').click()`);
    assert.equal(await b.eval(`document.activeElement.getAttribute('aria-label')`), 'Góp ý: Design', 'a filled box takes focus');
    await b.eval(`(() => { const t = document.activeElement; t.value = ''; t.dispatchEvent(new Event('input', { bubbles: true })); t.blur(); })()`);
    assert.equal(await b.eval(`document.querySelector('aside span').textContent`), '0 góp ý');
    await b.eval(`${heading('Design')}.parentElement.querySelector('button').click()`);
    assert.equal(await b.eval(`${heading('Design')}.parentElement.nextElementSibling.hidden`), true, 'an empty box closes on a second click');
    s.kill();
  });

  it('Sửa sends the notes in document order, with the section keys', async () => {
    await open('edit');
    await note('Design', 'add a diagram');
    await note('Goals', 'tighten goals');
    await general('looks fine');
    await b.eval(`${bar('Gửi góp ý')}.click()`);
    const r = await s.answers();
    assert.equal(r.code, 0);
    assert.equal(r.src, DOC);
    assert.deepEqual(r.lines, ['Quyết định: Sửa', '[§ Overview › Goals] tighten goals', '[§ Design] add a diagram', '[§ Chung] looks fine']);
  });

  it('Sửa with no note warns and sends nothing; Duyệt then sends the bare decision', async () => {
    await open('approve');
    await b.eval(`${bar('Gửi góp ý')}.click()`);
    await b.until(`${STATUS}.startsWith('Chưa có góp ý')`);
    assert.match(await b.status(), /Góp ý chung cuối trang\.$/);
    assert.equal(await b.eval(`${bar('Duyệt')}.disabled`), false);
    await b.eval(`${bar('Duyệt')}.click()`);
    const r = await s.answers();
    assert.equal(r.code, 0);
    assert.deepEqual(r.lines, ['Quyết định: Duyệt']);
  });

  it('restores notes after a reload and sends with the keyboard', async () => {
    await open('draft');
    await note('Risks', 'kept note');
    await b.goto(await s.url, { ready, doc: true });
    assert.equal(await b.eval(`document.querySelector('[aria-label="Góp ý: Risks"]').value`), 'kept note');
    assert.equal(await b.eval(`document.querySelector('[aria-label="Góp ý: Risks"]').closest('[data-note]').hidden`), false);
    assert.equal(await b.status(), 'Đã khôi phục góp ý đã lưu.');
    assert.equal(await b.eval(`document.querySelector('aside span').textContent`), '1 góp ý');
    await b.press('Enter', { meta: true });
    const r = await s.answers();
    assert.deepEqual(r.lines, ['Quyết định: Sửa', '[§ Risks] kept note']);
    await b.until(`${STATUS}.startsWith('Đã gửi 1 góp ý cho Claude.')`);
  });

  it('round 2 marks the sections that changed, were added or were dropped', async () => {
    outDir('doc-review/round2');
    s = await serveRound2(dir + '/round2');
    await b.goto(await s.url, { ready, doc: true });
    const r = await b.eval(`({
      badge: document.querySelector('header .badge').textContent, banner: document.querySelector('main > .alert').innerText.replace(/\\n+/g, ' / '),
      marks: [...document.querySelectorAll('main .badge')].map((x) => x.closest('div').querySelector('h2, h3').textContent + ':' + x.textContent),
      links: [...document.querySelectorAll('main > .alert a')].map((a) => a.textContent + '>' + a.getAttribute('href').startsWith('#sec-')),
    })`);
    assert.equal(r.badge, 'Chờ duyệt · lần 2');
    assert.equal(r.banner, 'Lần 2: các mục thay đổi so với bản bạn xem lần trước. / Đã sửa (1): Open / Mới (1): Added / Đã bỏ (1): Risks');
    assert.deepEqual(r.marks, ['Added:Mới', 'Open:Đã sửa']);
    assert.deepEqual(r.links, ['Open>true', 'Added>true']);
    await b.eval(`document.querySelector('main > .alert a').click()`);
    await b.until(`location.hash.startsWith('#sec-')`);
    await b.eval(`${bar('Duyệt')}.click()`);
    const a = await s.answers();
    assert.deepEqual(a.lines, ['Quyết định: Duyệt']);
    const state = JSON.parse(readFileSync(dir + '/round2/.review-state.json', 'utf8'));
    assert.deepEqual(Object.values(state).map((v) => v.round), [2]);
    assert.deepEqual(b.unexpectedErrors(), []);
  });
});
