import { after, before, describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { launch } from '../harness/browser.mjs';
import { outDir, render, serve } from '../harness/serve.mjs';
import { writeRound } from '../fixtures/round.mjs';

const dir = outDir('interview');
let b, s, url;
const ready = `!!document.querySelector('#card-q02') && !!document.querySelector('aside')`;
const card = (q, k) => `#card-${q} [style*="grid-row"]:nth-of-type(${k})`;
const checked = (q) => b.eval(`[...document.querySelectorAll('input[name=choice-${q}]')].filter((i) => i.checked).map((i) => i.value).join(',')`);
const status = () => b.eval(`document.querySelector('aside p[role=status]').textContent`);
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
    await b.click(card('q01', 2) + ' a[title="Xem file"]');
    assert.equal(await b.eval(`document.querySelector('dialog').open`), true);
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

  it('falls back to copying when the server is gone', async () => {
    await fresh();
    s.kill();
    await s.done;
    await b.click('#q01-opt-b');
    await b.click('#q02-opt-x');
    await b.click('aside .btn-primary');
    await b.until(`document.querySelector('aside p[role=status]').textContent.startsWith('Không gửi được (Claude không còn chờ trang này)')`);
    assert.equal(await b.eval(`[...document.querySelectorAll('aside button')].find((x) => x.textContent.startsWith('Gửi')).disabled`), false, 'the send button works again');
    const text = await b.eval(`document.querySelector('#out').value`);
    assert.equal(text, '[q01] Which adder?\nChọn: B. Checked add\n\n[q02] Which extras?\nChọn: A. Extra X');
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
    await b.eval(`[...document.querySelectorAll('aside button')].find((x) => x.textContent.startsWith('Copy câu trả lời')).click()`);
    await b.until(`document.querySelector('aside p[role=status]').textContent !== ''`, { timeout: 6000 });
    assert.match(await status(), /^(Đã copy 2 câu trả lời|Không copy tự động được)/);
  });
});
