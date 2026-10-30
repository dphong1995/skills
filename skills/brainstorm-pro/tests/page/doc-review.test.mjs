import { after, before, describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { launch, sleep } from '../harness/browser.mjs';
import { ADDER, CHOOSER, COPY_OK, GUARDED, hint, PASSAGE_HELPERS, READY, STATUS } from '../harness/page-helpers.mjs';
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
    await b.click('#topic');
    assert.equal(await b.eval(GUARDED), true);
    await b.eval(`${bar('Gửi góp ý')}.click()`);
    await b.choose('s');
    const r = await s.answers();
    assert.equal(r.code, 0);
    await b.until(`${STATUS}.startsWith('Đã gửi')`);
    assert.equal(await b.eval('window.__closed'), 1);
    assert.equal(await b.eval(GUARDED), false);
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
    await b.until(`${STATUS}.startsWith('Đã gửi: Duyệt')`);
    assert.equal(await b.eval('window.__closed'), 1);
  });

  it('restores notes after a reload and sends with the keyboard', async () => {
    await open('draft');
    await note('Risks', 'kept note');
    await b.click('#topic');
    const dialogs = b.leaveDialogs();
    await b.goto(await s.url, { ready, doc: true });
    assert.equal(await b.eval(`document.querySelector('[aria-label="Góp ý: Risks"]').value`), 'kept note');
    assert.equal(await b.eval(`document.querySelector('[aria-label="Góp ý: Risks"]').closest('[data-note]').hidden`), false);
    assert.equal(b.leaveDialogs(), dialogs + 1, 'the browser asked before the reload');
    assert.equal(await b.status(), 'Đã khôi phục góp ý đã lưu.');
    assert.equal(await b.eval(`document.querySelector('aside span').textContent`), '1 góp ý');
    await b.press('Enter', { meta: true });
    await b.choose('s');
    const r = await s.answers();
    assert.deepEqual(r.lines, ['Quyết định: Sửa', '[§ Risks] kept note']);
    await b.until(`${STATUS}.startsWith('Đã gửi 1 góp ý cho Claude.')`);
  });

  it('with a note, ⌘Enter opens the chooser with the count and d sends Duyệt with the note', async () => {
    await open('choose-d');
    await note('Design', 'add a diagram');
    await b.press('Enter', { meta: true });
    await b.until(CHOOSER);
    assert.equal(await b.eval(`document.querySelector('dialog[open] .modal-box > span').textContent`), 'Gửi 1 góp ý như:');
    assert.equal(await b.eval(`document.activeElement.classList.contains('modal-box')`), true, 'focus lands on the box, not a button');
    await b.handled('d');
    const r = await s.answers();
    assert.equal(r.code, 0);
    assert.deepEqual(r.lines, ['Quyết định: Duyệt', '[§ Design] add a diagram']);
  });

  it('s in the chooser sends Sửa', async () => {
    await open('choose-s');
    await note('Design', 'add a diagram');
    await b.press('Enter', { meta: true });
    await b.choose('s');
    const r = await s.answers();
    assert.deepEqual(r.lines, ['Quyết định: Sửa', '[§ Design] add a diagram']);
  });

  it('Enter in the chooser sends nothing and Esc closes it and returns focus to the note', async () => {
    await open('choose-esc');
    await note('Design', 'add a diagram');
    await b.press('Enter', { meta: true });
    await b.until(CHOOSER);
    await b.press('Enter');
    assert.equal(await Promise.race([s.done, sleep(300)]), undefined, 'Enter sends nothing');
    assert.equal(await b.eval(CHOOSER), true);
    await b.press('Escape');
    await b.until(`!(${CHOOSER})`);
    assert.equal(await b.eval(`document.activeElement.getAttribute('aria-label')`), 'Góp ý: Design');
    s.kill();
  });

  it('with a note, both bar buttons open the chooser', async () => {
    await open('choose-bar');
    await note('Design', 'add a diagram');
    for (const label of ['Duyệt', 'Gửi góp ý']) {
      await b.eval(`${bar(label)}.click()`);
      await b.until(CHOOSER);
      await b.press('Escape');
      await b.until(`!(${CHOOSER})`);
    }
    s.kill();
  });

  it('with no note, ⌘⇧Enter approves at once and ⌘Enter warns', async () => {
    await open('choose-bare');
    await b.press('Enter', { meta: true });
    await b.until(`${STATUS}.startsWith('Chưa có góp ý')`);
    assert.equal(await b.eval(CHOOSER), false);
    assert.equal(await Promise.race([s.done, sleep(300)]), undefined);
    await b.press('Enter', { meta: true, shift: true });
    const r = await s.answers();
    assert.deepEqual(r.lines, ['Quyết định: Duyệt']);
  });

  it('a file page asks the chooser first, then copies with the chosen decision', async () => {
    const out = `${dir}/file-choose/review-01.html`;
    s = serve(DOC, out);
    await s.url;
    await b.goto('file://' + out, { ready, doc: true });
    await note('Design', 'add a diagram');
    await b.eval(COPY_OK);
    await b.press('Enter', { meta: true });
    await b.choose('d');
    await b.until(`window.__copied !== null`);
    assert.equal(await b.eval('window.__copied'), 'Quyết định: Duyệt\n[§ Design] add a diagram');
    s.kill();
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
    const len = await b.eval('history.length');
    const view = (height) => b.send('Emulation.setDeviceMetricsOverride', { width: 1400, height, deviceScaleFactor: 1, mobile: false });
    await view(400);
    let href;
    try {
      await b.click('#qnav a:nth-of-type(2)');
      href = await b.eval(`document.querySelector('#qnav a:nth-of-type(2)').getAttribute('href')`);
      await b.until(`location.hash === ${JSON.stringify(href)}`);
      assert.equal(await b.eval('history.length'), len, 'a chip jumps without a new history entry');
      const r = await b.eval(`({ top: document.querySelector(location.hash).getBoundingClientRect().top, header: document.querySelector('header').getBoundingClientRect().bottom })`);
      assert.ok(r.top >= r.header - 2 && r.top < r.header + 40, 'the heading sits just below the header: ' + JSON.stringify(r));
    } finally { await view(1000); }
    await b.click('main > .alert a');
    await b.until(`location.hash.startsWith('#sec-') && location.hash !== ${JSON.stringify(href)}`);
    assert.equal(await b.eval('history.length'), len, 'the Đã sửa link jumps without a new history entry');
    await b.eval(`${bar('Duyệt')}.click()`);
    const a = await s.answers();
    assert.deepEqual(a.lines, ['Quyết định: Duyệt']);
    const state = JSON.parse(readFileSync(dir + '/round2/.review-state.json', 'utf8'));
    assert.deepEqual(Object.values(state).map((v) => v.round), [2]);
    assert.deepEqual(b.unexpectedErrors(), []);
  });

  it('round 2 shows the previous notes under their headings and in the alert, and the word diff behind a toggle', async () => {
    outDir('doc-review/round2-notes');
    const QUOTE = 'first goal bullet with some words second goal bullet with more words third goal bullet with other terms';
    s = await serveRound2(dir + '/round2-notes', async (first) => {
      await b.goto(await first.url, { ready, doc: true });
      await b.eval(PASSAGE_HELPERS);
      await note('Open', 'why open?');
      await note('Risks', 'drop <b>me</b>');
      await b.eval(`T.select('few plain words')`);
      await b.until(`T.adder() !== 'hidden'`, { timeout: 3000 });
      await b.click(ADDER('passage'));
      await b.until(`T.boxes().length === 1`);
      await b.type('about the intro');
      await b.eval(`document.activeElement.blur()`);
      await b.eval(`T.select('first goal bullet', 'other terms')`);
      await b.until(`T.adder() !== 'hidden'`, { timeout: 3000 });
      await b.click(ADDER('passage'));
      await b.until(`T.boxes().length === 2`);
      await b.type('why these goals');
      await general('overall');
      await b.press('Enter', { meta: true });
      await b.choose('s');
      const sent = await first.answers();
      assert.equal(sent.code, 0);
      assert.equal(sent.lines.length, 6);
    });
    const state = JSON.parse(readFileSync(dir + '/round2-notes/.review-state.json', 'utf8'));
    assert.deepEqual(Object.values(state)[0].notes.map((n) => n.kind + ':' + n.key), ['passage:Mở đầu', 'passage:Overview › Goals', 'section:Risks', 'section:Open', 'general:Chung']);
    assert.equal(Object.values(state)[0].notes[1].quote, QUOTE, 'the full quote is stored');
    await b.goto(await s.url, { ready, doc: true });
    await b.eval(PASSAGE_HELPERS);
    await b.twSettle();
    const under = (t) => b.eval(`(() => { const r = ${heading(t)}.parentElement; let o = r.nextElementSibling; const out = [];
      while (o && !o.matches('[data-k]')) { if (o.dataset.prev !== undefined) out.push(o.innerText.replace(/\\n+/g, ' | ')); o = o.nextElementSibling; } return out; })()`);
    assert.deepEqual(await under('Open'), ['Góp ý lần trước | why open?']);
    assert.deepEqual(await under('Goals'), ['Góp ý lần trước | «' + QUOTE + '» → why these goals']);
    assert.deepEqual(await under('Design'), []);
    const alert = await b.eval(`document.querySelector('main > .alert [data-prev]').innerText.replace(/\\n+/g, ' | ')`);
    assert.equal(alert, 'Góp ý lần trước không còn chỗ gắn: | [§ Mở đầu · dòng 3] «few plain words» → about the intro | [§ Risks] drop <b>me</b> | [§ Chung] overall');
    assert.equal(await b.eval(`document.querySelector('main > .alert [data-prev] b')`), null, 'note text is not HTML');
    assert.equal(await b.eval(`document.querySelectorAll('[data-diff]').length`), 1, 'only the section that changed');
    const toggle = `${heading('Open')}.parentElement.querySelector('button')`;
    const box = `document.querySelector('[data-diff]')`;
    assert.equal(await b.eval(`${toggle}.textContent`), 'Xem chỗ đổi');
    assert.equal(await b.eval(`${box}.checkVisibility()`), false);
    await b.eval(`${toggle}.click()`);
    assert.equal(await b.eval(`${toggle}.textContent`), 'Ẩn chỗ đổi');
    assert.equal(await b.eval(`${box}.checkVisibility()`), true);
    const parts = await b.eval(`[...${box}.children].map((x) => x.dataset.t + ':' + x.textContent + ':' + getComputedStyle(x).textDecorationLine)`);
    assert.ok(parts.includes('del:stays:line-through') && parts.includes('add:was:none') && parts.includes('add:reworded :none'), parts.join('|'));
    assert.equal(await b.eval(`getComputedStyle(${box}).fontFamily.includes('mono')`), true);
    assert.equal(await b.eval(`getComputedStyle(${box}).whiteSpace`), 'pre-wrap');
    assert.equal(await b.twWrites(), 0, 'the toggle must not rebuild the stylesheet');
    for (const phrase of ['reworded', '«first goal']) {
      await b.eval(`T.select(${JSON.stringify(phrase)})`);
      await sleep(350);
      assert.equal(await b.eval('T.adder()'), 'hidden', 'no Góp ý inside ' + phrase);
    }
    await b.eval(`T.select('under the components')`);
    await b.until(`T.adder() !== 'hidden'`, { timeout: 3000 });
    await b.eval(`${toggle}.click()`);
    assert.equal(await b.eval(`${box}.checkVisibility()`), false);
    assert.deepEqual(b.unexpectedErrors(), []);
    s.kill();
  });

  it('a copy after the server is gone ends the leave guard and does not close the tab', async () => {
    await open('copy');
    s.kill();
    await s.done;
    await b.click('#topic');
    assert.equal(await b.eval(GUARDED), true);
    await b.eval(COPY_OK);
    const n = b.errors.length;
    await b.eval(`${bar('Duyệt')}.click()`);
    await b.until(`${STATUS}.includes('Đã copy')`);
    assert.deepEqual(b.errors.splice(n).filter((e) => !/ERR_CONNECTION_REFUSED|Failed to load resource/.test(e)), []);
    assert.equal(await b.eval(GUARDED), false);
    assert.equal(await b.eval('window.__closed'), 0);
  });

  it('a file page copies, and a copy ends the leave guard', async () => {
    const out = `${dir}/file/review-01.html`;
    s = serve(DOC, out);
    await s.url;
    await b.goto('file://' + out, { ready, doc: true });
    assert.deepEqual(await b.eval(`[...document.querySelectorAll('aside button')].map((x) => x.textContent)`).then((l) => l.map(hint)), ['Copy: DuyệtCHORD', 'Copy góp ýMOD']);
    await b.click('#topic');
    assert.equal(await b.eval(GUARDED), true);
    await b.eval(COPY_OK);
    await b.eval(`${bar('Copy: Duyệt')}.click()`);
    await b.until(`${STATUS}.includes('Đã copy')`);
    assert.equal(await b.eval(GUARDED), false);
    assert.equal(await b.eval('window.__closed'), 0);
    assert.deepEqual(b.unexpectedErrors(), []);
  });
});
