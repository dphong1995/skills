// The keyboard shortcuts on all three pages, with real key presses (Input.dispatchKeyEvent) in headless Chrome.
import { after, before, describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { launch, sleep } from '../harness/browser.mjs';
import { PASSAGE_HELPERS } from '../harness/page-helpers.mjs';
import { FIXTURES, outDir, parseAnswers, render, serve } from '../harness/serve.mjs';
import { writeRound } from '../fixtures/round.mjs';

const dir = outDir('shortcuts');
let b, s;

// The element carrying the ring: a card id, or the text of a heading; null when there is none, 'MANY' when more than one has it.
const RING = `(() => { const r = [...document.querySelectorAll('[data-current]')];
  return r.length > 1 ? 'MANY' : r[0] ? (r[0].id || r[0].querySelector('h2, h3').textContent) : null; })()`;
const ring = () => b.eval(RING);
const msg = () => b.eval(`document.querySelector('aside [role=status]').textContent`);
const active = () => b.eval(`document.activeElement.id || document.activeElement.getAttribute('aria-label') || document.activeElement.tagName`);
const helpOpen = () => b.eval(`!!document.querySelector('dialog[open]:has(dl)')`);
// A printable key as one keyDown with its text: the harness's press() adds a separate char event, which would type the key into a box the shortcut just focused.
const press = async (k, mods = {}) => {
  if (k.length !== 1 || mods.ctrl || mods.meta) return b.press(k, mods);
  const code = /[a-z]/i.test(k) ? 'Key' + k.toUpperCase() : /\d/.test(k) ? 'Digit' + k : '';
  const vk = k.toUpperCase().charCodeAt(0);
  const base = { key: k, code, windowsVirtualKeyCode: vk, nativeVirtualKeyCode: vk, modifiers: (mods.alt ? 1 : 0) | (mods.shift ? 8 : 0) };
  await b.send('Input.dispatchKeyEvent', { type: 'keyDown', ...base, text: k });
  await b.send('Input.dispatchKeyEvent', { type: 'keyUp', ...base });
};
const MAC_OR_NOT = (t) => t.replace(/⌘⇧ Enter|Ctrl Shift Enter/, 'CHORD').replace(/⌘ Enter|Ctrl Enter/, 'MOD');
const helpRows = async () => (await b.eval(`[...document.querySelector('dialog:has(dl)').querySelectorAll('section')].map((x) =>
  [x.querySelector('h3').textContent, [...x.querySelectorAll('dt')].map((t) => t.textContent + ' ' + t.nextElementSibling.textContent)])`))
  .map(([g, rows]) => [g, rows.map(MAC_OR_NOT)]);
// Waits until the page has not scrolled for 300 ms, and returns scrollY.
const still = () => b.eval(`new Promise((r) => { let last = -1, n = 0; (function f() { n = scrollY === last ? n + 1 : 0; last = scrollY; n >= 6 ? r(scrollY) : setTimeout(f, 50); })(); })`);
// The ring only follows the scroll again once the page's own scroll is over (scrollend, or 1 s at most).
const unlocked = () => sleep(1200);
const settleTw = async () => { let n; do { n = await b.twWrites(); await sleep(500); } while (n !== (await b.twWrites())); await b.twReset(); };
const noTwWrites = async (what) => { await sleep(400); assert.equal(await b.twWrites(), 0, what + ' must not rebuild the stylesheet'); };

// What every mode shows in the `?` dialog besides its own keys.
const COMMON = ['Chung', ['? Mở hoặc đóng bảng phím', 'Esc Rời ô đang gõ, hoặc bỏ chọn chữ']];
const VIEWER_LINK = `[...document.querySelectorAll('main a')].find((x) => x.textContent === 'the code file').click()`;
const NAV_DOC = ['J Tiêu đề kế', 'K Tiêu đề trước', '1–9 Mục ## thứ n'];

// The key behaviour common to every page: the dialog, the filter, the ring.
const commonKeys = (page) => {
  it('the ? key and the header button open the dialog, which lists the page\'s keys', async () => {
    await b.eval('document.activeElement.blur()');
    await settleTw();
    await press('?', { shift: true });
    await b.until(`!!document.querySelector('dialog[open]:has(dl)')`);
    await noTwWrites('the first ?');
    assert.deepEqual(await helpRows(), page.help);
    assert.equal(await b.eval(`[...document.body.children].some((e) => e.tagName === 'DIALOG' && !!e.querySelector('dl'))`), true, 'built at load, among the body children');
    await press('?', { shift: true });
    await b.until(`!document.querySelector('dialog[open]:has(dl)')`);
    await b.click('header button[title="Phím tắt"]');
    await b.until(`!!document.querySelector('dialog[open]:has(dl)')`);
    await press('Escape');
    await b.until(`!document.querySelector('dialog[open]:has(dl)')`);
    assert.equal(await b.eval(`document.querySelector('header button[title="Phím tắt"] kbd').textContent`), '?');
  });

  it('an open dialog blocks the other keys, and the viewer does not let ? open the help', async () => {
    await b.eval('document.activeElement.blur()');
    const before = await ring();
    await press('?', { shift: true });
    await b.until(`!!document.querySelector('dialog[open]:has(dl)')`);
    for (const k of ['j', 'k', '2', 'n', 'a', 'g']) await press(k);
    assert.equal(await ring(), before);
    assert.equal(await b.eval(`document.querySelector('dialog[open]:has(dl)') !== null`), true);
    await press('?', { shift: true });
    await b.until(`!document.querySelector('dialog[open]:has(dl)')`);
    await b.eval(page.openViewer);
    await b.until(`!!document.querySelector('dialog[open]')`);
    for (const k of ['j', 'k', '1']) await press(k);
    await press('?', { shift: true });
    await sleep(200);
    assert.equal(await ring(), before, 'keys ignored over the viewer');
    assert.equal(await helpOpen(), false, '? ignored over the viewer');
    await press('Escape');
    await b.until(`!document.querySelector('dialog[open]')`);
  });

  it('keys are ignored while composing, and ⌘C, Option and Shift+letter pass through unprevented', async () => {
    await b.eval('document.activeElement.blur()');
    const before = await ring();
    await b.eval(`document.dispatchEvent(new KeyboardEvent('keydown', { code: 'KeyJ', key: 'j', isComposing: true, bubbles: true, cancelable: true }));
      document.dispatchEvent(new KeyboardEvent('keydown', { code: 'KeyK', key: 'Process', keyCode: 229, bubbles: true, cancelable: true }));
      __keys.length = 0;`);
    assert.equal(await ring(), before);
    await press('c', { meta: true });
    await press('j', { alt: true });
    await press('j', { shift: true });
    await press('j', { ctrl: true });
    assert.deepEqual(await b.eval('__keys.map((k) => k.pagePrevented)'), [false, false, false, false], 'the page leaves these four alone');
    assert.equal(await ring(), before);
  });
};

describe('interview page keys', () => {
  const rounds = (d) => writeRound(d, (data) => {
    data.questions[1].options[1].recommended = true;
    data.questions.push({ id: 'q03', n: 3, title: 'No recommendation here?', options: [{ id: 'm', label: 'Opt M' }, { id: 'n', label: 'Opt N' }] });
  });
  const checked = (q) => b.eval(`[...document.querySelectorAll('#card-${q} input:checked')].map((i) => i.value)`);
  const toTop = () => b.eval(`scrollTo({ top: 0, behavior: 'instant' })`);

  before(async () => {
    b = await launch();
    const d = outDir('shortcuts/interview');
    assert.equal(render(rounds(d), d + '/round.html').code, 0);
    await b.goto('file://' + d + '/round.html', { ready: `!!document.querySelector('#card-q03')` });
    await b.eval(PASSAGE_HELPERS);
  });
  after(async () => { await b.close(); });

  it('has no ring until the first shortcut, not even after a click', async () => {
    assert.equal(await ring(), null);
    await b.click('#card-q01 h2');
    assert.equal(await ring(), null);
    assert.equal(await b.eval(`document.querySelectorAll('header button[title="Phím tắt"]').length`), 1);
  });

  it('the first J draws the ring on the card after the one at the top, with no stylesheet rebuild', async () => {
    await toTop();
    await settleTw();
    await press('j');
    await b.until(`${RING} === 'card-q02'`);
    await noTwWrites('the first J and the first ring');
  });

  commonKeys({
    openViewer: `[...document.querySelectorAll('main button')].find((x) => x.textContent === 'Xem toàn file').click()`,
    help: [
      ['Điều hướng', ['J Câu kế', 'K Câu trước', '1–9 Câu thứ n', 'N Câu chưa trả lời kế tiếp']],
      ['Thao tác', ['A–D Chọn phương án (câu nhiều lựa chọn: bấm lại để bỏ)', 'R Chọn phương án Claude đề xuất', 'X Khác (tự nhập)', 'E Cần giải thích thêm',
        'G Ghi chú của câu', 'MOD Copy câu trả lời']],
      COMMON,
    ],
  });

  it('J and K step between cards and stop at the ends; digits jump to the nth card', async () => {
    await press('k');
    assert.equal(await ring(), 'card-q01');
    await press('k');
    assert.equal(await ring(), 'card-q01');
    await press('3');
    assert.equal(await ring(), 'card-q03');
    await press('j');
    assert.equal(await ring(), 'card-q03');
    await press('9');
    assert.equal(await ring(), 'card-q03');
    await press('1');
    assert.equal(await ring(), 'card-q01');
    await press('2');
    await press('j');
    assert.equal(await ring(), 'card-q03');
    assert.equal(await b.eval(`document.querySelectorAll('[data-current]').length`), 1);
  });

  it('⌘ Enter submits (here: warns about the first open card), also from inside a box', async () => {
    await press('Enter', { meta: true });
    await b.until(`/chưa chọn phương án/.test(document.querySelector('aside [role=status]').textContent)`);
    await b.eval(`document.querySelector('#q02-note').focus()`);
    await b.eval(`document.querySelector('aside [role=status]').textContent = ''`);
    await press('Enter', { ctrl: true });
    await b.until(`/chưa chọn phương án/.test(document.querySelector('aside [role=status]').textContent)`);
    await b.eval(`document.activeElement.blur()`);
  });

  it('A-D pick the option of the current card without scrolling; on a multi card they toggle; a missing option is ignored', async () => {
    await press('1');
    const y = await still();
    await press('b');
    assert.deepEqual(await checked('q01'), ['b']);
    await press('a');
    assert.deepEqual(await checked('q01'), ['a']);
    await press('d');
    assert.deepEqual(await checked('q01'), ['a'], 'q01 has no option D');
    await sleep(300);
    assert.equal(await b.eval('scrollY'), y);
    await press('2');
    await press('a');
    assert.deepEqual(await checked('q02'), ['x']);
    await press('a');
    assert.deepEqual(await checked('q02'), []);
    await press('a');
    await press('b');
    assert.deepEqual((await checked('q02')).sort(), ['x', 'y']);
  });

  it('R picks the recommended option (never unchecks on a multi card); with none it says so', async () => {
    await press('1');
    await press('r');
    assert.deepEqual(await checked('q01'), ['b']);
    await press('2');
    await press('a');
    await press('b');
    assert.deepEqual(await checked('q02'), []);
    await press('r');
    assert.deepEqual(await checked('q02'), ['y']);
    await press('r');
    assert.deepEqual(await checked('q02'), ['y']);
    await press('3');
    await press('r');
    assert.equal(await msg(), 'Câu này không có đề xuất');
    assert.deepEqual(await checked('q03'), []);
  });

  it('X and E check Khác / Cần giải thích and the cursor goes to the box; typed keys stay in the box; Esc leaves it', async () => {
    await press('1');
    await press('x');
    assert.deepEqual(await checked('q01'), ['other']);
    assert.equal(await active(), 'q01-other');
    await press('j');
    await press('2');
    assert.equal(await b.eval(`document.querySelector('#q01-other').value`), 'j2');
    assert.equal(await ring(), 'card-q01');
    await press('Escape');
    assert.notEqual(await b.eval('document.activeElement.tagName'), 'TEXTAREA');
    assert.equal(await b.eval(`document.querySelector('#q01-other').value`), 'j2', 'Esc keeps the text');
    await press('e');
    assert.deepEqual(await checked('q01'), ['explain']);
    assert.equal(await active(), 'q01-note');
    await press('Escape');
    await press('g');
    assert.equal(await active(), 'q01-note');
    await press('Escape');
    await press('a');
    assert.deepEqual(await checked('q01'), ['a']);
  });

  it('N goes to the next unanswered card, wraps round, and says when all are answered', async () => {
    await press('2');
    await press('n');
    assert.equal(await ring(), 'card-q03', 'q02 is done, q03 is the next open card');
    await press('a');
    await press('2');
    await press('b');
    await press('3');
    await press('n');
    assert.equal(await ring(), 'card-q02', 'wraps from q03 to the open q02');
    await press('a');
    await press('n');
    assert.match(await msg(), /^Đã trả lời hết, (⌘ Enter|Ctrl Enter) để gửi$/);
    assert.equal(await ring(), 'card-q02');
  });

  it('Esc outside a box clears the text selection', async () => {
    await b.eval(`getSelection().selectAllChildren(document.querySelector('#card-q01 h2'))`);
    assert.equal(await b.eval('getSelection().isCollapsed'), false);
    await press('Escape');
    assert.equal(await b.eval('getSelection().isCollapsed'), true);
  });

  it('the ring follows a click or focus inside a card, and manual scrolling', async () => {
    await b.click('#card-q03 h2');
    assert.equal(await ring(), 'card-q03');
    await b.eval(`document.querySelector('#q01-note').focus()`);
    assert.equal(await ring(), 'card-q01');
    await unlocked();
    await b.click('#card-q03 h2');
    await unlocked();
    await toTop();
    await b.until(`${RING} === 'card-q01'`, { what: 'ring follows the scroll to the top' });
  });

  it('J at the end of the page still moves the ring, and the scroll does not move it back', async () => {
    await b.eval(`document.activeElement.blur(); scrollTo({ top: document.documentElement.scrollHeight, behavior: 'instant' })`);
    await unlocked();
    await b.until(`${RING} === 'card-q01'`, { what: 'q01 is the card at the top of the last screen' });
    assert.equal(await b.eval(`scrollY + innerHeight >= document.documentElement.scrollHeight - 2`), true);
    await press('j');
    assert.equal(await ring(), 'card-q02');
    await press('j');
    assert.equal(await ring(), 'card-q03');
    await unlocked();
    assert.equal(await ring(), 'card-q03');
  });


  it('reports no console errors', async () => assert.deepEqual(b.unexpectedErrors(), []));
});

describe('read-only document keys', () => {

  before(async () => {
    b = await launch();
    const d = outDir('shortcuts/doc-read');
    assert.equal(render(FIXTURES + '/doc.md', d + '/page.html', ['--open']).code, 0);
    await b.goto('file://' + d + '/page.html', { ready: `document.querySelectorAll('main > article').length >= 5`, doc: true });
  });
  after(async () => { await b.close(); });

  it('has no ring until the first shortcut; the first J moves from the first heading to the next, without a stylesheet rebuild', async () => {
    assert.equal(await ring(), null);
    assert.equal(await b.eval(`document.querySelectorAll('header button[title="Phím tắt"]').length`), 1);
    await settleTw();
    await press('j');
    await b.until(`${RING} === 'Goals'`);
    await noTwWrites('the first J and the first ring');
  });

  commonKeys({ openViewer: VIEWER_LINK, help: [['Điều hướng', NAV_DOC], COMMON] });

  it('J and K walk the ## and ### headings, digits jump to the nth ## heading', async () => {
    await press('k');
    assert.equal(await ring(), 'Overview');
    await press('k');
    assert.equal(await ring(), 'Overview');
    const seen = [];
    for (let i = 0; i < 8; i++) { await press('j'); seen.push(await ring()); }
    assert.deepEqual(seen, ['Goals', 'Non-goals', 'Design', 'Components', 'Risks', 'Open', 'Open', 'Open']);
    await press('2');
    assert.equal(await ring(), 'Design');
    await press('4');
    assert.equal(await ring(), 'Open');
    await press('9');
    assert.equal(await ring(), 'Open');
    await press('1');
    assert.equal(await ring(), 'Overview');
  });

  it('the review-only keys do nothing, and Esc clears the selection', async () => {
    const y = await still();
    for (const k of ['n', 'c', 'g', 'a', 'r', 'x']) await press(k);
    await press('Enter', { meta: true });
    await press('Enter', { meta: true, shift: true });
    assert.equal(await ring(), 'Overview');
    assert.equal(await b.eval(`document.querySelectorAll('textarea').length`), 0);
    await sleep(200);
    assert.equal(await b.eval('scrollY'), y);
    await b.eval(`getSelection().selectAllChildren(document.querySelector('main h2'))`);
    await press('Escape');
    assert.equal(await b.eval('getSelection().isCollapsed'), true);
  });

  it('the ring follows a click on a heading and manual scrolling', async () => {
    await b.click(`h3[id]:not([hidden])`);
    assert.equal(await ring(), 'Goals');
    await unlocked();
    await b.eval(`scrollTo({ top: document.documentElement.scrollHeight, behavior: 'instant' })`);
    await still();
    await b.eval(`scrollTo({ top: 0, behavior: 'instant' })`);
    await b.until(`${RING} === 'Overview'`, { what: 'ring follows the scroll to the top' });
  });

  it('reports no console errors', async () => assert.deepEqual(b.unexpectedErrors(), []));
});

describe('review document keys', () => {
  const ready = `document.querySelectorAll('main > article').length >= 5 && !!document.querySelector('[aria-label="Góp ý: Chung"]')`;
  const open = async (name) => {
    s = serve(FIXTURES + '/doc.md', `${dir}/${name}/review-01.html`);
    await b.goto(await s.url, { ready, doc: true });
    await b.eval(PASSAGE_HELPERS);
  };
  const answers = async () => { const r = await s.done; return { code: r.code, ...parseAnswers(r.stdout) }; };
  const stillRunning = async () => (await Promise.race([s.done, sleep(500)])) === undefined;
  const boxVisible = (label) => `(() => { const r = document.querySelector('[aria-label=${JSON.stringify(label)}]').getBoundingClientRect(); return r.top >= 0 && r.bottom <= innerHeight; })()`;
  const fill = async (k, text) => { await press(k); await b.type(text); };

  before(async () => { b = await launch(); });
  after(async () => { s && s.kill(); await b.close(); });

  it('shows ⌘⇧ Enter on Duyệt and the ? hint in the intro line', async () => {
    await open('main');
    assert.match(await b.eval(`[...document.querySelectorAll('aside button')].find((x) => x.textContent.startsWith('Duyệt')).textContent`), /^Duyệt(⌘⇧ Enter|Ctrl Shift Enter)$/);
    assert.match(await b.eval(`document.querySelector('main > p').textContent`), / · \? phím tắt\.$/);
    assert.equal(await b.eval(`document.querySelectorAll('header button[title="Phím tắt"]').length`), 1);
    assert.equal(await ring(), null);
  });

  it('J, K and digits work as on the read-only page; the first J draws the ring without a stylesheet rebuild', async () => {
    await settleTw();
    await press('j');
    await b.until(`${RING} === 'Goals'`);
    await noTwWrites('the first J and the first ring');
  });

  commonKeys({
    openViewer: VIEWER_LINK,
    help: [
      ['Điều hướng', [...NAV_DOC, 'N Góp ý đã viết kế tiếp']],
      ['Thao tác', ['C Góp ý đoạn đang chọn, hoặc góp ý của mục đang chọn', 'G Góp ý chung', 'MOD Gửi góp ý', 'CHORD Duyệt (có góp ý thì bấm hai lần)']],
      COMMON,
    ],
  });

  it('digits and K work as on the read-only page', async () => {
    await press('3');
    assert.equal(await ring(), 'Risks');
    await press('k');
    assert.equal(await ring(), 'Components');
  });

  it('N with no note says so', async () => {
    await press('n');
    assert.equal(await msg(), 'Chưa có góp ý nào');
  });

  it('G opens the general note; the box takes typed keys, Esc leaves it', async () => {
    await press('g');
    assert.equal(await active(), 'Góp ý: Chung');
    await press('j');
    assert.equal(await b.eval(`document.activeElement.value`), 'j');
    await b.eval(`document.activeElement.value = ''; document.activeElement.dispatchEvent(new Event('input', { bubbles: true }))`);
    await press('Escape');
    assert.equal(await active(), 'BODY');
  });

  it('C opens and focuses the note of the current heading, and the ring follows the focus', async () => {
    await press('1');
    await press('2');
    await b.eval(`document.activeElement.blur()`);
    await press('c');
    assert.equal(await active(), 'Góp ý: Design');
    assert.equal(await b.eval(`document.activeElement.closest('[data-note]').hidden`), false);
    await b.type('design note');
    await press('Escape');
    assert.equal(await ring(), 'Design');
  });

  it('C with a selection comments on it like the Góp ý button, and focuses the new note', async () => {
    await b.eval(`T.select('first goal bullet')`);
    await b.until(`T.adder() === 'shown'`);
    await press('c');
    const id = await b.until(`T.boxes()[0]`);
    assert.equal(await b.eval(`document.activeElement === T.ta(${JSON.stringify(id)})`), true);
    assert.equal(await b.eval(`T.adder()`), 'hidden');
    assert.equal(await b.eval(`T.marks(${JSON.stringify(id)})`), 1);
    await b.type('goal note');
    await press('Escape');
  });

  it('N walks the written notes in document order, wraps, and scrolls the box into view without focusing it', async () => {
    await fill('g', 'general note');
    await press('Escape');
    await press('1');
    const seen = [];
    const where = [];
    for (let i = 0; i < 4; i++) {
      await press('n');
      seen.push(await ring());
      where.push(await b.eval(`document.activeElement.tagName`));
    }
    assert.deepEqual(seen, ['Goals', 'Design', 'Open', 'Goals'], 'the passage note sits under Goals, the general note after the last heading');
    assert.deepEqual(where, ['BODY', 'BODY', 'BODY', 'BODY']);
    await press('2');
    await press('n');
    assert.equal(await ring(), 'Design', 'from Design the next note is its own');
    await b.until(boxVisible('Góp ý: Design'), { what: 'the Design box is on screen' });
    await b.eval(`scrollTo({ top: 0, behavior: 'instant' })`);
    await press('n');
    assert.equal(await ring(), 'Open', 'its own note first, then the next');
  });

  it('⌘ Enter sends the notes, also while typing in a box', async () => {
    await b.eval(`document.querySelector('[aria-label="Góp ý: Chung"]').focus()`);
    await press('Enter', { meta: true });
    const a = await answers();
    assert.equal(a.code, 0);
    assert.equal(a.lines[0], 'Quyết định: Sửa');
    assert.equal(a.lines.length, 4);
    assert.ok(a.lines.some((l) => l.endsWith('→ goal note')) && a.lines.includes('[§ Design] design note') && a.lines.includes('[§ Chung] general note'));
  });

  it('⌘⇧ Enter with no note approves at once', async () => {
    s.kill();
    await open('chord-bare');
    await press('Enter', { meta: true, shift: true });
    const a = await answers();
    assert.equal(a.code, 0);
    assert.deepEqual(a.lines, ['Quyết định: Duyệt']);
  });

  it('⌘⇧ Enter with a note asks first, sends on the second press within 3 s, and starts over after 3 s', async () => {
    await open('chord-note');
    await fill('g', 'one note');
    await press('Enter', { meta: true, shift: true });
    await b.until(`/^Duyệt kèm 1 góp ý\\? Bấm .* lần nữa trong 3 giây$/.test(document.querySelector('aside [role=status]').textContent)`);
    assert.equal(await stillRunning(), true, 'the first press sends nothing');
    // The page reads performance.now(), so moving it forward 5 s stands in for waiting.
    await b.eval(`(() => { const now = performance.now.bind(performance); performance.now = () => now() + 5000; })()`);
    await b.eval(`document.querySelector('aside [role=status]').textContent = ''`);
    await press('Enter', { ctrl: true, shift: true });
    await b.until(`/^Duyệt kèm 1 góp ý\\?/.test(document.querySelector('aside [role=status]').textContent)`);
    assert.equal(await stillRunning(), true, 'after 3 s the next press asks again');
    await press('Enter', { meta: true, shift: true });
    const a = await answers();
    assert.equal(a.code, 0);
    assert.deepEqual(a.lines, ['Quyết định: Duyệt', '[§ Chung] one note']);
  });

  it('reports no console errors', async () => assert.deepEqual(b.unexpectedErrors(), []));
});
