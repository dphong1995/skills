import { after, before, describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { launch } from '../harness/browser.mjs';
import { COPY_OK, GUARDED, READY, STATUS } from '../harness/page-helpers.mjs';
import { outDir, render, serve } from '../harness/serve.mjs';
import { writeRound } from '../fixtures/round.mjs';

const dir = outDir('interview');
let b, s, url;
const ready = READY.interview;
const card = (q, k) => `#card-${q} [style*="grid-row"]:nth-of-type(${k})`;
const checked = (q) => b.eval(`[...document.querySelectorAll('input[name=choice-${q}]')].filter((i) => i.checked).map((i) => i.value).join(',')`);
const status = () => b.status();
// A reload with the saved draft cleared, so each test starts from an empty page.
const fresh = async () => { await b.eval('localStorage.clear()'); await b.goto(url, { ready }); };

describe('interview page', () => {
  before(async () => {
    s = serve(writeRound(dir), dir + '/round.html');
    url = await s.url;
    b = await launch();
    await b.goto(url, { ready });
  });
  after(async () => { s.kill(); await b.close(); });

  it('renders the cards and their parts', async () => {
    const r = await b.eval(`(() => {
      const c1 = document.querySelector('#card-q01');
      return {
        title: document.title, topic: document.querySelector('#topic').textContent,
        cards: document.querySelectorAll('main > article').length, nav: [...document.querySelectorAll('#qnav a')].map((a) => a.textContent + '>' + a.getAttribute('href')),
        decided: document.querySelector('main > details summary').textContent,
        badges: [...c1.querySelectorAll('header .badge')].map((x) => x.textContent),
        optionCards: c1.querySelectorAll('[style*="grid-row"]').length, recommended: c1.querySelectorAll('.badge-primary:not(.badge-outline)').length,
        radios: c1.querySelectorAll('input[type=radio]').length, boxes: document.querySelectorAll('#card-q02 input[type=checkbox]').length,
        figures: c1.querySelectorAll('figure').length, mock: c1.querySelectorAll('iframe[data-mock]').length, table: c1.querySelectorAll('table').length,
        img: c1.querySelectorAll('figure img').length, note: c1.textContent.includes('Lưu ý: Check the numbers'),
        fileButtons: [...c1.querySelectorAll('figure button')].filter((x) => x.textContent === 'Xem toàn file').length,
        pathLinks: c1.querySelectorAll('a[title="Xem file"]').length, why: c1.textContent.includes('Vì sao cần chốt'),
        recommendation: c1.textContent.includes('Claude đề xuất B. Checked add'), parts: [...c1.textContent.matchAll(/Ví dụ: /g)].length,
        chosen: document.querySelector('aside span').textContent,
      };
    })()`);
    assert.equal(r.title, 'Câu 1–2 · Fixture round');
    assert.equal(r.topic, 'Fixture round');
    assert.equal(r.cards, 2);
    assert.deepEqual(r.nav, ['Câu 1>#card-q01', 'Câu 2>#card-q02']);
    assert.equal(r.decided, 'Đã chốt trước đó (2)');
    assert.deepEqual(r.badges.slice(0, 4), ['Câu 1', 'Design', 'Dễ đổi về sau', 'Chưa chọn']);
    assert.equal(r.optionCards, 3);
    assert.equal(r.recommended, 1);
    assert.equal(r.radios, 5);
    assert.equal(r.boxes, 4);
    assert.equal(r.figures, 5, 'code, diff, mermaid, img and mockup');
    assert.equal(r.mock, 1);
    assert.equal(r.table, 1);
    assert.ok(r.img >= 1);
    assert.ok(r.note && r.why && r.recommendation);
    assert.ok(r.fileButtons >= 1, 'code block offers Xem toàn file');
    assert.ok(r.pathLinks >= 2, 'path spans link to the viewer');
    assert.equal(r.parts, 3);
    assert.equal(r.chosen, '0/2 câu');
    assert.deepEqual(b.unexpectedErrors(), []);
  });

  // Clicks card q01's prototype button; returns the hint and the card counts checked on the way.
  const protoButton = (real) => b.eval(`(() => {
    const c1 = document.querySelector('#card-q01');
    const btns = [...c1.querySelectorAll('button')].filter((x) => x.textContent === 'Mở prototype ↗');
    const out = { n: btns.length, hint: c1.textContent.includes('Lật bằng 1–3, ←/→, R ở tab mới; chọn ở đây.'), none: [...document.querySelectorAll('#card-q02 button')].filter((x) => x.textContent.includes('prototype')).length };
    if (${!!real}) btns[0].setAttribute('data-proto-btn', '');
    else btns[0].click();
    return out;
  })()`);
  const protoChecks = (r) => { assert.equal(r.n, 1); assert.ok(r.hint); assert.equal(r.none, 0); };
  // Stubbed window.open: asserts the call gets a blob URL and 'noopener'.
  const protoClick = async () => {
    await b.eval('window.__opened = []; window.open = (u, t, f) => { window.__opened.push([u, t, f]); return null; }');
    protoChecks(await protoButton());
    const opened = await b.eval('window.__opened');
    assert.equal(opened.length, 1);
    assert.ok(opened[0][0].startsWith('blob:'));
    assert.equal(opened[0][2], 'noopener');
    const text = await b.eval(`fetch(window.__opened[0][0]).then((x) => x.text())`);
    assert.ok(text.includes('proto-picker') && text.includes('data-variant="Quiet"'));
    assert.equal(await checked('q01'), '');
  };
  // Real click: follows the new tab through the browser's CDP target list, headless, no window.
  const protoClickReal = async () => {
    const list = async () => (await (await fetch(`http://127.0.0.1:${b.port}/json/list`)).json()).filter((t) => t.type === 'page');
    const before = new Set((await list()).map((t) => t.id));
    protoChecks(await protoButton(true));
    await b.click('[data-proto-btn]'); // a real mouse click: a scripted click() has no user activation and the popup is blocked
    let tab;
    for (let i = 0; i < 80 && !tab; i++) {
      tab = (await list()).find((t) => !before.has(t.id) && t.url.startsWith('blob:'));
      if (!tab) await new Promise((r) => setTimeout(r, 100));
    }
    assert.ok(tab, 'the click opened a new blob: tab');
    const ws = new WebSocket(tab.webSocketDebuggerUrl);
    await new Promise((r) => { ws.onopen = r; });
    const probe = (expression) => new Promise((res) => {
      ws.onmessage = (m) => { const d = JSON.parse(m.data); if (d.id === 1) res(d.result.result.value); };
      ws.send(JSON.stringify({ id: 1, method: 'Runtime.evaluate', params: { expression, returnByValue: true } }));
    });
    try {
      for (let i = 0; i < 40 && !(await probe(`!!document.querySelector('.proto-picker')`)); i++) await new Promise((r) => setTimeout(r, 100));
      assert.equal(await probe(`!!document.querySelector('.proto-picker') && !!document.querySelector('#stage')`), true);
      assert.equal(await probe('window.opener'), null);
    } finally {
      ws.close();
      await fetch(`http://127.0.0.1:${b.port}/json/close/${tab.id}`).catch(() => {});
    }
    assert.equal(await checked('q01'), '');
  };
  it('prototype button opens a tab and picks nothing', async () => {
    await fresh();
    await protoClick();
  });
  it('picks an option from a card click and from the strip button', async () => {
    await fresh();
    await b.click(card('q01', 2) + ' > div:first-child');
    assert.equal(await checked('q01'), 'b');
    assert.equal(await b.eval(`document.querySelector('${card('q01', 2)}').dataset.chosen`), 'true');
    await b.click('#q01-opt-a');
    assert.equal(await checked('q01'), 'a');
    assert.equal(await b.eval(`document.querySelector('${card('q01', 2)}').dataset.chosen`), 'false');
    await b.click(card('q01', 3) + ' > div:first-child');
    assert.equal(await checked('q01'), 'c');
  });

  it('does not pick from a file link, Xem toàn file or the end of a text selection', async () => {
    await fresh();
    const len = await b.eval('history.length');
    await b.click(card('q01', 2) + ' a[title="Xem file"]');
    assert.equal(await b.eval(`document.querySelector('dialog').open`), true);
    assert.equal(await b.eval('history.length'), len, 'a file link does not navigate');
    assert.equal(await b.eval(`location.href.endsWith('#')`), false);
    assert.equal(await checked('q01'), '');
    await b.eval(`document.querySelector('dialog').close()`);
    await b.click('#card-q01 figcaption button');
    assert.equal(await b.eval(`document.querySelector('dialog').open`), true);
    assert.equal(await checked('q01'), '');
    await b.eval(`document.querySelector('dialog').close()`);
    await b.eval(`(() => { const p = document.querySelector('${card('q02', 2)} p'); getSelection().selectAllChildren(p); p.click(); })()`);
    assert.equal(await checked('q02'), '');
    await b.eval('getSelection().removeAllRanges()');
    await b.click('#card-q01 iframe[data-mock]');
    assert.equal(await checked('q01'), '', 'a mockup click picks nothing');
  });

  it('multi choice toggles; Khác and Cần giải thích exclude each other as designed', async () => {
    await fresh();
    await b.click(card('q02', 1) + ' p');
    assert.equal(await checked('q02'), 'x');
    await b.click(card('q02', 1) + ' p');
    assert.equal(await checked('q02'), '', 'a second click unchecks');
    await b.click(card('q02', 1) + ' p');
    await b.click(card('q02', 2) + ' p');
    assert.equal(await checked('q02'), 'x,y');
    await b.click('#q02-opt-other');
    assert.equal(await checked('q02'), 'x,y,other');
    assert.equal(await b.eval(`document.activeElement.id`), 'q02-other', 'Khác focuses its box');
    await b.click('#q02-opt-explain');
    assert.equal(await checked('q02'), 'explain', 'Cần giải thích clears the rest');
    assert.equal(await b.eval(`document.activeElement.id`), 'q02-note');
    await b.click('#q02-opt-other');
    assert.equal(await checked('q02'), 'other', 'choosing Khác clears Cần giải thích');
    await b.click('#q02-opt-explain');
    await b.click(card('q02', 1) + ' p');
    assert.equal(await checked('q02'), 'x', 'an option clears Cần giải thích');
  });

  it('blocks Khác with an empty box and marks the card', async () => {
    await fresh();
    await b.click('#q01-opt-other');
    assert.equal(await b.eval(`document.querySelector('#q01-other').hidden`), false);
    await b.click('#q02-opt-x');
    await b.click('aside .btn-primary');
    await b.until(`document.querySelector('#card-q01').classList.contains('attention')`);
    assert.equal(await status(), 'Câu 1 chọn "Khác" nhưng chưa gõ phương án.');
    await b.click('#q01-opt-a');
    assert.equal(await b.eval(`document.querySelector('#card-q01').classList.contains('attention')`), false, 'a change clears the mark');
  });

  it('Theo đề xuất picks the recommended option of the unanswered cards', async () => {
    await fresh();
    assert.equal(await b.eval(`[...document.querySelectorAll('aside button')].find((x) => x.textContent === 'Theo đề xuất').hidden`), false);
    await b.eval(`[...document.querySelectorAll('aside button')].find((x) => x.textContent === 'Theo đề xuất').click()`);
    assert.equal(await checked('q01'), 'b');
    assert.equal(await checked('q02'), '', 'a card without a recommendation stays empty');
    assert.match(await status(), /^Đã chọn theo đề xuất cho 1 câu\./);
    assert.equal(await b.eval(`[...document.querySelectorAll('aside button')].find((x) => x.textContent === 'Theo đề xuất').hidden`), true);
  });

  it('shows a check in the top bar and counts answered questions', async () => {
    await fresh();
    assert.equal(await b.eval(`document.querySelector('#qnav a').textContent`), 'Câu 1');
    assert.equal(await b.eval(`document.querySelector('#qnav a').classList.contains('btn-outline')`), true);
    await b.click('#q01-opt-c');
    assert.equal(await b.eval(`document.querySelector('#qnav a').textContent`), 'Câu 1 ✓');
    assert.equal(await b.eval(`document.querySelector('#qnav a').classList.contains('btn-outline')`), false);
    assert.equal(await b.eval(`document.querySelector('aside span').textContent`), '1/2 câu');
    assert.equal(await b.eval(`document.querySelector('#card-q01 header .badge:last-child').textContent`), 'Đã chọn');
    await b.click('#q02-opt-y');
    assert.equal(await b.eval(`document.querySelector('aside span').textContent`), '2/2 câu');
  });

  it('restores the draft after a reload and Xóa nháp clears it', async () => {
    await fresh();
    await b.click('#q01-opt-b');
    await b.click('#q02-opt-x');
    await b.eval(`document.querySelector('#q01-note').focus()`);
    await b.type('lý do của tôi');
    await b.goto(url, { ready });
    assert.equal(await checked('q01'), 'b');
    assert.equal(await checked('q02'), 'x');
    assert.equal(await b.eval(`document.querySelector('#q01-note').value`), 'lý do của tôi');
    assert.equal((await status()).startsWith('Đã khôi phục câu trả lời đã lưu.'), true);
    await b.eval(`[...document.querySelectorAll('aside p[role=status] button')].find((x) => x.textContent === 'Xóa nháp').click()`);
    assert.equal(await checked('q01'), '');
    assert.equal(await b.eval(`document.querySelector('#q01-note').value`), '');
    assert.equal(await b.eval(`localStorage.getItem('interview-draft:' + location.pathname)`), null);
    assert.equal(await status(), '');
  });

  it('asks before leaving until copied; a nav chip jumps without a new history entry', async () => {
    await fresh();
    await b.click('#topic');
    assert.equal(await b.eval(GUARDED), true);
    const len = await b.eval('history.length');
    const view = (height) => b.send('Emulation.setDeviceMetricsOverride', { width: 1400, height, deviceScaleFactor: 1, mobile: false });
    await view(400);
    try {
      await b.click('#qnav a:nth-of-type(2)');
      await b.until(`location.hash === '#card-q02'`);
      const r = await b.eval(`({ len: history.length, top: document.querySelector('#card-q02').getBoundingClientRect().top, header: document.querySelector('header').getBoundingClientRect().bottom })`);
      assert.equal(r.len, len);
      assert.ok(r.top >= r.header - 2 && r.top < r.header + 40, 'the card sits just below the header: ' + JSON.stringify(r));
    } finally { await view(1000); }
    await b.click('#q01-opt-b');
    await b.click('#q02-opt-x');
    await b.eval(COPY_OK);
    await b.eval(`[...document.querySelectorAll('aside button')].find((x) => x.textContent === 'Copy').click()`);
    await b.until(`${STATUS}.startsWith('Đã copy 2 câu trả lời')`);
    assert.equal(await b.eval(GUARDED), false);
    assert.equal(await b.eval('window.__closed'), 0);
  });

  it('writes Other and Needs explanation in the English protocol', async () => {
    await fresh();
    await b.click('#q01-opt-other');
    await b.eval(`(() => { const t = document.querySelector('#q01-other'); t.value = 'my own'; t.dispatchEvent(new Event('input', { bubbles: true })); })()`);
    await b.click('#q02-opt-explain');
    await b.eval(COPY_OK);
    await b.eval(`[...document.querySelectorAll('aside button')].find((x) => x.textContent === 'Copy').click()`);
    await b.until(`${STATUS}.startsWith('Đã copy 2 câu trả lời')`);
    assert.equal(await b.eval('window.__copied'), '[q01] Which adder?\nPick: Other: my own\n\n[q02] Which extras?\nPick: Needs explanation');
  });

  it('falls back to copying when the server is gone', async () => {
    await fresh();
    s.kill();
    await s.done;
    await b.click('#q01-opt-b');
    await b.click('#q02-opt-x');
    assert.equal(await b.eval(GUARDED), true);
    await b.eval(COPY_OK);
    await b.click('aside .btn-primary');
    await b.until(`${STATUS}.startsWith('Không gửi được (Claude không còn chờ trang này). Đã copy')`);
    assert.equal(await b.eval(GUARDED), false);
    assert.equal(await b.eval('window.__closed'), 0);
    assert.equal(await b.eval(`[...document.querySelectorAll('aside button')].find((x) => x.textContent.startsWith('Gửi')).disabled`), false, 'the send button works again');
    const text = await b.eval(`document.querySelector('#out').value`);
    assert.equal(text, '[q01] Which adder?\nPick: B. Checked add\n\n[q02] Which extras?\nPick: A. Extra X');
  });

  it('closes its tab after sending and then stops asking', async () => {
    s = serve(writeRound(dir), dir + '/send/round.html');
    await b.goto(await s.url, { ready });
    await b.click('#topic');
    assert.equal(await b.eval(GUARDED), true);
    await b.click('#q01-opt-b');
    await b.click('#q02-opt-x');
    await b.click('aside .btn-primary');
    const r = await s.answers();
    assert.equal(r.code, 0);
    await b.until(`${STATUS}.startsWith('Đã gửi cho Claude')`);
    assert.equal(await b.eval('window.__closed'), 1);
    assert.equal(await b.eval(GUARDED), false);
    const dialogs = b.leaveDialogs();
    await b.send('Page.navigate', { url: 'about:blank' });
    assert.equal(b.leaveDialogs(), dialogs, 'an answered page leaves without asking');
  });

  it('prototype button works on the file-opened page', async () => {
    const out = dir + '/proto-file/round.html';
    assert.equal(render(writeRound(dir), out).code, 0);
    await b.goto('file://' + out, { ready });
    await protoClickReal();
  });

  it('a file page has no server and offers a copy button', async () => {
    const out = dir + '/file/round.html';
    const r = render(writeRound(dir), out);
    assert.equal(r.code, 0);
    await b.goto('file://' + out, { ready });
    const labels = await b.eval(`[...document.querySelectorAll('aside button')].map((x) => x.textContent)`);
    assert.ok(labels.some((x) => x.startsWith('Copy câu trả lời')));
    assert.ok(!labels.some((x) => x.startsWith('Gửi')));
    await b.click('#q01-opt-a');
    await b.click('#q02-opt-x');
    assert.equal(await b.eval(GUARDED), true);
    await b.eval(COPY_OK);
    await b.eval(`[...document.querySelectorAll('aside button')].find((x) => x.textContent.startsWith('Copy câu trả lời')).click()`);
    await b.until(`${STATUS} !== ''`, { timeout: 6000 });
    assert.match(await status(), /^Đã copy 2 câu trả lời/);
    assert.equal(await b.eval(GUARDED), false);
    assert.equal(await b.eval('window.__closed'), 0);
  });
});
