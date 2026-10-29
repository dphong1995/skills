// The keyboard shortcuts on all three pages, with real key presses (Input.dispatchKeyEvent) in headless Chrome.
import { after, before, describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { launch, sleep } from '../harness/browser.mjs';
import { hint, PASSAGE_HELPERS, READY, STATUS } from '../harness/page-helpers.mjs';
import { FIXTURES, outDir, render, serve } from '../harness/serve.mjs';
import { writeRound } from '../fixtures/round.mjs';

const dir = outDir('shortcuts');
let b, s;

before(async () => { b = await launch(); });
after(async () => { s && s.kill(); await b.close(); });

// The element carrying the ring: a card id, or the text of a heading; null when there is none, 'MANY' when more than one has it.
const RING = `(() => { const r = [...document.querySelectorAll('[data-current]')];
  return r.length > 1 ? 'MANY' : r[0] ? (r[0].id || r[0].querySelector('h2, h3').textContent) : null; })()`;
const ring = () => b.eval(RING);
const msg = () => b.status();
const active = () => b.eval(`document.activeElement.id || document.activeElement.getAttribute('aria-label') || document.activeElement.tagName`);
const helpOpen = () => b.eval(`!!document.querySelector('dialog[open]:has(dl)')`);
// A key the page handled: its own listener prevented the default (the harness's last-in-line hook would hide a missing preventDefault, so a shortcut that moves focus could pass without it).
const handled = async (k, mods) => {
  await b.press(k, mods);
  assert.equal(await b.eval('__keys.at(-1).pagePrevented'), true, k + ' must be prevented by the page itself');
};
// A key the page has no use for: it leaves the key alone.
const ignored = async (k, mods) => {
  await b.press(k, mods);
  assert.equal(await b.eval('__keys.at(-1).pagePrevented'), false, k + ' must not be prevented');
};
const helpRows = async () => (await b.eval(`(() => { const out = [];
  for (const g of document.querySelectorAll('dialog:has(dl) section')) {
    out.push([g.querySelector('h3').textContent, [...g.querySelectorAll('dl > dt')].map((x) => x.textContent + ' ' + x.nextElementSibling.textContent)]);
  }
  return out; })()`)).map(([g, rows]) => [g, rows.map(hint)]);
// Waits until the page has not scrolled for 300 ms, and returns scrollY.
const still = () => b.eval(`new Promise((r) => { let last = -1, n = 0; (function f() { n = scrollY === last ? n + 1 : 0; last = scrollY; n >= 6 ? r(scrollY) : setTimeout(f, 50); })(); })`);
// A wheel turn is the user's own scroll: it ends the lock a shortcut's scroll holds (the page waits for 150 ms of quiet, or 1 s when nothing scrolls).
const unlocked = async () => {
  await still();
  const [x, y] = await b.eval(`[innerWidth / 2, innerHeight / 2]`);
  await b.send('Input.dispatchMouseEvent', { type: 'mouseWheel', x, y, deltaX: 0, deltaY: 1 });
};
const lockEnds = () => sleep(1200);
const noTwWrites = async (what) => { await sleep(400); assert.equal(await b.twWrites(), 0, what + ' must not rebuild the stylesheet'); };

// What every mode shows in the `?` dialog besides its own keys.
const COMMON = ['Chung', ['? Mở hoặc đóng bảng phím', 'Esc Rời ô đang gõ, hoặc bỏ chọn chữ']];
const VIEWER_LINK = `[...document.querySelectorAll('main a')].find((x) => x.textContent === 'the code file').click()`;
const NAV_DOC = ['J Tiêu đề kế', 'K Tiêu đề trước', '1–9 Mục ## thứ n'];

// The key behaviour common to every page: the dialog, the filter, the ring.
const commonKeys = (page) => {
  it('the ? key and the header button open the dialog, which lists the page\'s keys', async () => {
    await b.eval('document.activeElement.blur()');
    await b.twSettle();
    await handled('?', { shift: true });
    await b.until(`!!document.querySelector('dialog[open]:has(dl)')`);
    await noTwWrites('the first ?');
    assert.deepEqual(await helpRows(), page.help);
    assert.equal(await b.eval(`[...document.body.children].some((e) => e.tagName === 'DIALOG' && !!e.querySelector('dl'))`), true, 'built at load, among the body children');
    await b.press('?', { shift: true });
    await b.until(`!document.querySelector('dialog[open]:has(dl)')`);
    await b.click('header button[title="Phím tắt"]');
    await b.until(`!!document.querySelector('dialog[open]:has(dl)')`);
    await b.press('Escape');
    await b.until(`!document.querySelector('dialog[open]:has(dl)')`);
    assert.equal(await b.eval(`document.querySelector('header button[title="Phím tắt"] kbd').textContent`), '?');
  });

  it('an open dialog blocks the other keys, and the viewer does not let ? open the help', async () => {
    await b.eval('document.activeElement.blur()');
    const before = await ring();
    await b.press('?', { shift: true });
    await b.until(`!!document.querySelector('dialog[open]:has(dl)')`);
    for (const k of ['j', 'k', '2', 'n', 'a', 'g']) await b.press(k);
    assert.equal(await ring(), before);
    assert.equal(await b.eval(`document.querySelector('dialog[open]:has(dl)') !== null`), true);
    await b.press('?', { shift: true });
    await b.until(`!document.querySelector('dialog[open]:has(dl)')`);
    await b.eval(page.openViewer);
    await b.until(`!!document.querySelector('dialog[open]')`);
    for (const k of ['j', 'k', '1']) await b.press(k);
    await b.press('?', { shift: true });
    await sleep(200);
    assert.equal(await ring(), before, 'keys ignored over the viewer');
    assert.equal(await helpOpen(), false, '? ignored over the viewer');
    await b.press('Escape');
    await b.until(`!document.querySelector('dialog[open]')`);
  });

  it('keys are ignored while composing, and ⌘C, Option and Shift+letter pass through unprevented', async () => {
    await b.eval('document.activeElement.blur()');
    const before = await ring();
    await b.eval(`document.dispatchEvent(new KeyboardEvent('keydown', { code: 'KeyJ', key: 'j', isComposing: true, bubbles: true, cancelable: true }));
      document.dispatchEvent(new KeyboardEvent('keydown', { code: 'KeyK', key: 'Process', keyCode: 229, bubbles: true, cancelable: true }));
      __keys.length = 0;`);
    assert.equal(await ring(), before);
    await b.press('c', { meta: true });
    await b.press('j', { alt: true });
    await b.press('j', { shift: true });
    await b.press('j', { ctrl: true });
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
    const d = outDir('shortcuts/interview');
    assert.equal(render(rounds(d), d + '/round.html').code, 0);
    await b.goto('file://' + d + '/round.html', { ready: `!!document.querySelector('#card-q03')` });
    await b.eval(PASSAGE_HELPERS);
  });

  it('has no ring until the first shortcut, not even after a click', async () => {
    assert.equal(await ring(), null);
    await b.click('#card-q01 h2');
    assert.equal(await ring(), null);
    assert.equal(await b.eval(`document.querySelectorAll('header button[title="Phím tắt"]').length`), 1);
  });

  it('the first J draws the ring on the card after the one at the top, with no stylesheet rebuild', async () => {
    await toTop();
    await b.twSettle();
    await b.press('j');
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
    await handled('k');
    assert.equal(await ring(), 'card-q01');
    await b.press('k');
    assert.equal(await ring(), 'card-q01');
    await handled('3');
    assert.equal(await ring(), 'card-q03');
    await handled('j');
    assert.equal(await ring(), 'card-q03');
    await b.press('9');
    assert.equal(await ring(), 'card-q03');
    await b.press('1');
    assert.equal(await ring(), 'card-q01');
    await b.press('2');
    await b.press('j');
    assert.equal(await ring(), 'card-q03');
    assert.equal(await b.eval(`document.querySelectorAll('[data-current]').length`), 1);
  });

  it('⌘ Enter submits (here: warns about the first open card), also from inside a box', async () => {
    await b.press('Enter', { meta: true });
    await b.until(`/chưa chọn phương án/.test(${STATUS})`);
    await b.eval(`document.querySelector('#q02-note').focus()`);
    await b.eval(`${STATUS} = ''`);
    await b.press('Enter', { ctrl: true });
    await b.until(`/chưa chọn phương án/.test(${STATUS})`);
    await b.eval(`document.activeElement.blur()`);
  });

  it('A-D pick the option of the current card without scrolling; on a multi card they toggle; a missing option is ignored', async () => {
    await b.press('1');
    const y = await still();
    await handled('b');
    assert.deepEqual(await checked('q01'), ['b']);
    await b.press('a');
    assert.deepEqual(await checked('q01'), ['a']);
    await b.press('d');
    assert.deepEqual(await checked('q01'), ['a'], 'q01 has no option D');
    await sleep(300);
    assert.equal(await b.eval('scrollY'), y);
    await b.press('2');
    await b.press('a');
    assert.deepEqual(await checked('q02'), ['x']);
    await b.press('a');
    assert.deepEqual(await checked('q02'), []);
    await b.press('a');
    await b.press('b');
    assert.deepEqual((await checked('q02')).sort(), ['x', 'y']);
  });

  it('C on a card without an option C, and ⌘⇧ Enter, change nothing', async () => {
    await b.press('2');
    const was = await checked('q02');
    await b.eval(`${STATUS} = ''`);
    await handled('c');
    await ignored('Enter', { meta: true, shift: true });
    assert.deepEqual(await checked('q02'), was);
    assert.equal(await msg(), '');
    assert.equal(await ring(), 'card-q02');
  });

  it('R picks the recommended option (never unchecks on a multi card); with none it says so', async () => {
    await b.press('1');
    await b.press('r');
    assert.deepEqual(await checked('q01'), ['b']);
    await b.press('2');
    await b.press('a');
    await b.press('b');
    assert.deepEqual(await checked('q02'), []);
    await b.press('r');
    assert.deepEqual(await checked('q02'), ['y']);
    await b.press('r');
    assert.deepEqual(await checked('q02'), ['y']);
    await b.press('3');
    await b.press('r');
    assert.equal(await msg(), 'Câu này không có đề xuất');
    assert.deepEqual(await checked('q03'), []);
  });

  it('X and E check Khác / Cần giải thích and the cursor goes to the box; typed keys stay in the box; Esc leaves it', async () => {
    await b.press('1');
    await handled('x');
    assert.deepEqual(await checked('q01'), ['other']);
    assert.equal(await active(), 'q01-other');
    await b.press('j');
    await b.press('2');
    assert.equal(await b.eval(`document.querySelector('#q01-other').value`), 'j2');
    assert.equal(await ring(), 'card-q01');
    await b.press('Escape');
    assert.notEqual(await b.eval('document.activeElement.tagName'), 'TEXTAREA');
    assert.equal(await b.eval(`document.querySelector('#q01-other').value`), 'j2', 'Esc keeps the text');
    await handled('e');
    assert.deepEqual(await checked('q01'), ['explain']);
    assert.equal(await active(), 'q01-note');
    await b.press('Escape');
    await handled('g');
    assert.equal(await active(), 'q01-note');
    await b.press('Escape');
    await b.press('a');
    assert.deepEqual(await checked('q01'), ['a']);
  });

  it('X and E on a multi card check Khác / Cần giải thích and focus the box, and never toggle them off', async () => {
    await b.press('2');
    assert.deepEqual(await checked('q02'), ['y']);
    for (const [k, box, want] of [['x', 'q02-other', ['y', 'other']], ['e', 'q02-note', ['explain']]]) {
      await b.eval(`document.activeElement.blur()`);
      await handled(k);
      assert.deepEqual(await checked('q02'), want);
      assert.equal(await active(), box);
      await b.press('Escape');
      await handled(k);
      assert.deepEqual(await checked('q02'), want, k + ' again keeps it checked');
      assert.equal(await active(), box, k + ' again focuses the box');
      await b.press('Escape');
    }
    await b.press('r');
    assert.deepEqual(await checked('q02'), ['y']);
  });

  it('N goes to the next unanswered card, wraps round, and says when all are answered', async () => {
    await b.press('2');
    await b.press('n');
    assert.equal(await ring(), 'card-q03', 'q02 is done, q03 is the next open card');
    await b.press('a');
    await b.press('2');
    await b.press('b');
    await b.press('3');
    await b.press('n');
    assert.equal(await ring(), 'card-q02', 'wraps from q03 to the open q02');
    await b.press('a');
    await b.press('n');
    assert.match(await msg(), /^Đã trả lời hết, (⌘ Enter|Ctrl Enter) để gửi$/);
    assert.equal(await ring(), 'card-q02');
  });

  it('Esc outside a box clears the text selection', async () => {
    await b.eval(`getSelection().selectAllChildren(document.querySelector('#card-q01 h2'))`);
    assert.equal(await b.eval('getSelection().isCollapsed'), false);
    await b.press('Escape');
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
    await b.until(`${RING} === 'card-q01'`, { what: 'q01 is the card at the top of the last screen' });
    assert.equal(await b.eval(`scrollY + innerHeight >= document.documentElement.scrollHeight - 2`), true);
    await b.press('j');
    assert.equal(await ring(), 'card-q02');
    await b.press('j');
    assert.equal(await ring(), 'card-q03');
    await lockEnds();
    assert.equal(await ring(), 'card-q03');
  });

  it('a manual scroll that starts while a shortcut\'s scroll is still running moves the ring', async () => {
    await toTop();
    await unlocked();
    await b.press('j');
    await b.press('j');
    assert.equal(await ring(), 'card-q03');
    await b.send('Input.dispatchMouseEvent', { type: 'mouseWheel', x: 700, y: 450, deltaX: 0, deltaY: -5000 });
    await b.until(`${RING} === 'card-q01'`, { what: 'the wheel scroll up to the top moves the ring, though it began inside the lock window' });
  });

  // Two J presses start a scroll that locks the ring; `scroll` is what the user does next, then the page jumps to the top at once.
  // The ring follows that jump (to card-q01) only when the user's action ended the lock.
  const afterUserAction = async (scroll) => {
    await toTop();
    await unlocked();
    await b.press('j');
    await b.press('j');
    assert.equal(await ring(), 'card-q03');
    await scroll();
    await b.eval(`scrollTo({ top: 0, behavior: 'instant' })`);
    await sleep(300);
    return ring();
  };

  it('a sideways wheel turn is not a scroll of the page: the lock stays', async () => {
    const [x, y] = await b.eval(`[innerWidth / 2, innerHeight / 2]`);
    assert.equal(await afterUserAction(() => b.send('Input.dispatchMouseEvent', { type: 'mouseWheel', x, y, deltaX: 40, deltaY: 0 })), 'card-q03');
  });

  it('PageDown outside a text box is the user\'s own scroll: it ends the lock', async () => {
    await b.eval('document.activeElement.blur()');
    assert.equal(await afterUserAction(() => b.press('PageDown')), 'card-q01');
  });


  it('reports no console errors', async () => assert.deepEqual(b.unexpectedErrors(), []));
});

describe('read-only document keys', () => {
  before(async () => {
    const d = outDir('shortcuts/doc-read');
    assert.equal(render(FIXTURES + '/doc.md', d + '/page.html', ['--open']).code, 0);
    await b.goto('file://' + d + '/page.html', { ready: READY.doc, doc: true });
  });

  it('has no ring until the first shortcut; the first J moves from the first heading to the next, without a stylesheet rebuild', async () => {
    assert.equal(await ring(), null);
    assert.equal(await b.eval(`document.querySelectorAll('header button[title="Phím tắt"]').length`), 1);
    await b.twSettle();
    await handled('j');
    await b.until(`${RING} === 'Goals'`);
    await noTwWrites('the first J and the first ring');
  });

  commonKeys({ openViewer: VIEWER_LINK, help: [['Điều hướng', NAV_DOC], COMMON] });

  it('J and K walk the ## and ### headings, digits jump to the nth ## heading', async () => {
    await b.press('k');
    assert.equal(await ring(), 'Overview');
    await b.press('k');
    assert.equal(await ring(), 'Overview');
    const seen = [];
    for (let i = 0; i < 8; i++) { await b.press('j'); seen.push(await ring()); }
    assert.deepEqual(seen, ['Goals', 'Non-goals', 'Design', 'Components', 'Risks', 'Open', 'Open', 'Open']);
    await handled('2');
    assert.equal(await ring(), 'Design');
    await b.press('4');
    assert.equal(await ring(), 'Open');
    await b.press('9');
    assert.equal(await ring(), 'Open');
    await b.press('1');
    assert.equal(await ring(), 'Overview');
  });

  it('the review-only keys do nothing, and Esc clears the selection', async () => {
    const y = await still();
    for (const k of ['n', 'c', 'g', 'a', 'r', 'x', 'e']) await ignored(k);
    await ignored('Enter', { meta: true });
    await ignored('Enter', { meta: true, shift: true });
    assert.equal(await ring(), 'Overview');
    assert.equal(await b.eval(`document.querySelectorAll('textarea').length`), 0);
    await sleep(200);
    assert.equal(await b.eval('scrollY'), y);
    await b.eval(`getSelection().selectAllChildren(document.querySelector('main h2'))`);
    await b.press('Escape');
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
  const open = async (name) => {
    s = serve(FIXTURES + '/doc.md', `${dir}/${name}/review-01.html`);
    await b.goto(await s.url, { ready: READY.review, doc: true });
    await b.eval(PASSAGE_HELPERS);
  };
  const stillRunning = async () => (await Promise.race([s.done, sleep(500)])) === undefined;
  const boxVisible = (label) => `(() => { const r = document.querySelector('[aria-label=${JSON.stringify(label)}]').getBoundingClientRect(); return r.top >= 0 && r.bottom <= innerHeight; })()`;
  const fill = async (k, text) => { await b.press(k); await b.type(text); };


  it('shows ⌘⇧ Enter on Duyệt and the ? hint in the intro line', async () => {
    await open('main');
    assert.match(await b.eval(`[...document.querySelectorAll('aside button')].find((x) => x.textContent.startsWith('Duyệt')).textContent`), /^Duyệt(⌘⇧ Enter|Ctrl Shift Enter)$/);
    assert.match(await b.eval(`document.querySelector('main > p').textContent`), / · \? phím tắt\.$/);
    assert.equal(await b.eval(`document.querySelectorAll('header button[title="Phím tắt"]').length`), 1);
    assert.equal(await ring(), null);
  });

  it('J, K and digits work as on the read-only page; the first J draws the ring without a stylesheet rebuild', async () => {
    await b.twSettle();
    await handled('j');
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
    await handled('3');
    assert.equal(await ring(), 'Risks');
    await b.press('k');
    assert.equal(await ring(), 'Components');
  });

  it('N with no note says so', async () => {
    await b.press('n');
    assert.equal(await msg(), 'Chưa có góp ý nào');
  });

  it('A, B, D, R, X and E do nothing on the review page', async () => {
    const was = { ring: await ring(), y: await still(), count: await b.eval(`document.querySelector('aside span').textContent`) };
    await b.eval(`${STATUS} = ''`);
    for (const k of ['a', 'b', 'd', 'r', 'x', 'e']) await ignored(k);
    assert.equal(await ring(), was.ring);
    assert.equal(await active(), 'BODY');
    assert.equal(await b.eval(`document.querySelector('aside span').textContent`), was.count);
    assert.equal(await msg(), '');
    await sleep(200);
    assert.equal(await b.eval('scrollY'), was.y);
  });

  it('G opens the general note; the box takes typed keys, Esc leaves it', async () => {
    await handled('g');
    assert.equal(await active(), 'Góp ý: Chung');
    await b.press('j');
    assert.equal(await b.eval(`document.activeElement.value`), 'j');
    await b.eval(`document.activeElement.value = ''; document.activeElement.dispatchEvent(new Event('input', { bubbles: true }))`);
    await b.press('Escape');
    assert.equal(await active(), 'BODY');
  });

  it('G scrolls to the general note without moving the ring', async () => {
    const view = (height) => b.send('Emulation.setDeviceMetricsOverride', { width: 1400, height, deviceScaleFactor: 1, mobile: false });
    await view(400);
    try {
      await b.eval(`document.activeElement.blur(); scrollTo({ top: 0, behavior: 'instant' })`);
      await b.press('1');
      await unlocked();
      assert.equal(await ring(), 'Overview');
      await handled('g');
      await b.until(boxVisible('Góp ý: Chung'), { what: 'the general note is on screen' });
      await still();
      assert.equal(await ring(), 'Overview', 'the scroll caused by the focus is the page\'s own');
      await b.press('Escape');
    } finally { await view(1000); }
  });

  it('C opens and focuses the note of the current heading, and the ring follows the focus', async () => {
    await b.press('1');
    await b.press('2');
    await b.eval(`document.activeElement.blur()`);
    await handled('c');
    assert.equal(await active(), 'Góp ý: Design');
    assert.equal(await b.eval(`document.activeElement.closest('[data-note]').hidden`), false);
    await b.type('design note');
    await b.press('Escape');
    assert.equal(await ring(), 'Design');
  });

  it('C with a selection comments on it like the Góp ý button, and focuses the new note', async () => {
    await b.eval(`T.select('first goal bullet')`);
    await b.until(`T.adder() === 'shown'`);
    await b.press('c');
    const id = await b.until(`T.boxes()[0]`);
    assert.equal(await b.eval(`document.activeElement === T.ta(${JSON.stringify(id)})`), true);
    assert.equal(await b.eval(`T.adder()`), 'hidden');
    assert.equal(await b.eval(`T.marks(${JSON.stringify(id)})`), 1);
    await b.type('goal note');
    await b.press('Escape');
  });

  it('N walks the written notes in document order, wraps, and scrolls the box into view without focusing it', async () => {
    await fill('g', 'general note');
    await b.press('Escape');
    await b.press('1');
    const seen = [];
    const where = [];
    for (let i = 0; i < 4; i++) {
      await b.press('n');
      seen.push(await ring());
      where.push(await b.eval(`document.activeElement.tagName`));
    }
    assert.deepEqual(seen, ['Goals', 'Design', 'Open', 'Goals'], 'the passage note sits under Goals, the general note after the last heading');
    assert.deepEqual(where, ['BODY', 'BODY', 'BODY', 'BODY']);
    await b.press('2');
    await b.press('n');
    assert.equal(await ring(), 'Design', 'from Design the next note is its own');
    await b.until(boxVisible('Góp ý: Design'), { what: 'the Design box is on screen' });
    await b.eval(`scrollTo({ top: 0, behavior: 'instant' })`);
    await b.press('n');
    assert.equal(await ring(), 'Open', 'its own note first, then the next');
  });

  it('⌘ Enter sends the notes, also while typing in a box', async () => {
    await b.eval(`document.querySelector('[aria-label="Góp ý: Chung"]').focus()`);
    await b.press('Enter', { meta: true });
    const a = await s.answers();
    assert.equal(a.code, 0);
    assert.equal(a.lines[0], 'Quyết định: Sửa');
    assert.equal(a.lines.length, 4);
    assert.ok(a.lines.some((l) => l.endsWith('→ goal note')) && a.lines.includes('[§ Design] design note') && a.lines.includes('[§ Chung] general note'));
  });

  it('N reaches a note on the intro paragraph, which has no heading above it, and wraps back to it', async () => {
    s.kill();
    await open('intro');
    await b.eval(`T.select('few plain words')`);
    await b.until(`T.adder() === 'shown'`);
    await b.press('c');
    const id = await b.until(`T.boxes()[0]`);
    await b.type('intro note');
    await b.press('Escape');
    await b.press('2');
    await b.press('c');
    await b.type('design note');
    await b.press('Escape');
    const intro = `Góp ý: hl:${id}`;
    // A short screen shows one box at a time: the boxes on screen say where N scrolled to.
    const NEAR = `['Góp ý: Design', ${JSON.stringify(intro)}].filter((l) => { const r = document.querySelector('[aria-label=' + JSON.stringify(l) + ']').getBoundingClientRect(); return r.top >= 0 && r.bottom <= innerHeight; }).join()`;
    const seen = [];
    await b.send('Emulation.setDeviceMetricsOverride', { width: 1400, height: 400, deviceScaleFactor: 1, mobile: false });
    try {
      for (let i = 0; i < 3; i++) {
        await b.eval(`scrollTo({ top: ${i === 1 ? 'document.documentElement.scrollHeight' : 0}, behavior: 'instant' })`);
        await still();
        await b.press('n');
        const want = i === 1 ? intro : 'Góp ý: Design';
        await b.until(`${NEAR} === ${JSON.stringify(want)}`, { what: 'only the ' + want + ' box is on screen' });
        await still();
        seen.push([await b.eval(NEAR), await ring()]);
      }
    } finally { await b.send('Emulation.setDeviceMetricsOverride', { width: 1400, height: 1000, deviceScaleFactor: 1, mobile: false }); }
    // The intro note has no heading above it: the ring is cleared, so the next J goes to the first heading.
    assert.deepEqual(seen, [['Góp ý: Design', 'Design'], [intro, null], ['Góp ý: Design', 'Design']]);
    assert.equal(await ring(), 'Design');
    await b.press('n');
    await still();
    assert.equal(await ring(), null);
    await b.press('j');
    await b.until(`${RING} === 'Overview'`, { what: 'J after the intro note goes to the first heading' });
  });

  it('⌘⇧ Enter with no note approves at once', async () => {
    s.kill();
    await open('chord-bare');
    await b.press('Enter', { meta: true, shift: true });
    const a = await s.answers();
    assert.equal(a.code, 0);
    assert.deepEqual(a.lines, ['Quyết định: Duyệt']);
  });

  it('⌘⇧ Enter with a note asks first, sends on the second press within 3 s, and starts over after 3 s', async () => {
    await open('chord-note');
    await fill('g', 'one note');
    await b.press('Enter', { meta: true, shift: true });
    await b.until(`/^Duyệt kèm 1 góp ý\\? Bấm .* lần nữa trong 3 giây$/.test(${STATUS})`);
    assert.equal(await stillRunning(), true, 'the first press sends nothing');
    // The page reads performance.now(), so moving it forward 5 s stands in for waiting.
    await b.eval(`(() => { const now = performance.now.bind(performance); performance.now = () => now() + 5000; })()`);
    await b.eval(`${STATUS} = ''`);
    await b.press('Enter', { ctrl: true, shift: true });
    await b.until(`/^Duyệt kèm 1 góp ý\\?/.test(${STATUS})`);
    assert.equal(await stillRunning(), true, 'after 3 s the next press asks again');
    await b.press('Enter', { meta: true, shift: true });
    const a = await s.answers();
    assert.equal(a.code, 0);
    assert.deepEqual(a.lines, ['Quyết định: Duyệt', '[§ Chung] one note']);
  });

  it('⌘⇧ Enter after the send went through does not ask again', async () => {
    await b.until(`/^Đã gửi: Duyệt\\./.test(${STATUS})`);
    await b.press('Enter', { meta: true, shift: true });
    await sleep(300);
    assert.match(await msg(), /^Đã gửi: Duyệt\./);
  });

  it('reports no console errors', async () => assert.deepEqual(b.unexpectedErrors(), []));
});
