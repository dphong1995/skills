// The page chrome in English (--lang en) has no Vietnamese; the default test CLAUDE_HOME keeps the Vietnamese page.
import { after, before, describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { launch } from '../harness/browser.mjs';
import { CHOOSER, READY } from '../harness/page-helpers.mjs';
import { FIXTURES, outDir, serve } from '../harness/serve.mjs';
import { writeRound } from '../fixtures/round.mjs';

const dir = outDir('lang');
let b, s;
before(async () => { b = await launch(); });
after(async () => { s && s.kill(); await b.close(); });

// Vietnamese letters in the visible text, in attributes people read (aria-label, title, placeholder, alt) and in the title. The fixtures are English, so everything found is chrome.
const VI = '/[àáạảãâầấậẩẫăằắặẳẵèéẹẻẽêềếệểễìíịỉĩòóọỏõôồốộổỗơờớợởỡùúụủũưừứựửữỳýỵỷỹđ]/i';
const SCAN = `(() => {
  const vi = ${VI};
  const found = [];
  const w = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
  for (let n; (n = w.nextNode());) if (!n.parentElement.closest('script, style') && vi.test(n.data)) found.push(n.data.trim());
  for (const e of document.querySelectorAll('[aria-label], [title], [placeholder], [alt]')) {
    for (const a of ['aria-label', 'title', 'placeholder', 'alt']) if (vi.test(e.getAttribute(a) || '')) found.push(a + '=' + e.getAttribute(a));
  }
  if (vi.test(document.title)) found.push('title: ' + document.title);
  return found;
})()`;
const clickText = (sel, text) => b.eval(`[...document.querySelectorAll(${JSON.stringify(sel)})].find((x) => x.textContent.startsWith(${JSON.stringify(text)})).click()`);
const openHelp = async () => {
  await b.eval(`document.querySelector('header button[title="Keyboard shortcuts"]').click()`);
  await b.until(`!!document.querySelector('dialog[open]:has(dl)')`);
};
const closeDialog = async () => { await b.eval(`document.querySelector('dialog[open]').close()`); };

describe('page language', () => {
  it('en interview page has no Vietnamese chrome', async () => {
    s = serve(writeRound(dir), dir + '/round.html', { args: ['--lang', 'en'] });
    await b.goto(await s.url, { ready: READY.interview });
    assert.equal(await b.eval('document.documentElement.lang'), 'en');
    assert.deepEqual(await b.eval(SCAN), []);
    await openHelp();
    assert.deepEqual(await b.eval(SCAN), [], 'help dialog');
    await closeDialog();
    await clickText('aside button', 'Send answers');
    await b.until(`document.querySelector('aside p[role=status]').textContent !== ''`);
    assert.match(await b.status(), /^Question 1 /);
    assert.deepEqual(await b.eval(SCAN), [], 'status line');
    s.kill();
  });

  it('en review page has no Vietnamese chrome, with the help dialog, a status line and the chooser open', async () => {
    s = serve(FIXTURES + '/doc.md', dir + '/review-01.html', { args: ['--lang', 'en'] });
    await b.goto(await s.url, { ready: `document.querySelectorAll('main > article').length >= 5 && !!document.querySelector('[aria-label="Note: General"]')`, doc: true });
    assert.equal(await b.eval('document.documentElement.lang'), 'en');
    assert.deepEqual(await b.eval(SCAN), []);
    await openHelp();
    assert.deepEqual(await b.eval(SCAN), [], 'help dialog');
    await closeDialog();
    await clickText('aside button', 'Send notes');
    await b.until(`document.querySelector('aside p[role=status]').textContent !== ''`);
    assert.deepEqual(await b.eval(SCAN), [], 'status line');
    await b.eval(`document.querySelector('main h2').parentElement.querySelector('button').click()`);
    await b.until(`document.activeElement.tagName === 'TEXTAREA'`);
    await b.type('a note');
    await clickText('aside button', 'Send notes');
    await b.until(CHOOSER);
    assert.deepEqual(await b.eval(SCAN), [], 'send chooser');
    s.kill();
  });

  it('vi page is unchanged', async () => {
    s = serve(writeRound(dir), dir + '/round-vi.html');
    await b.goto(await s.url, { ready: READY.interview });
    assert.equal(await b.eval('document.documentElement.lang'), 'vi');
    assert.ok((await b.eval(SCAN)).length > 5, 'the scan sees the Vietnamese chrome');
    assert.equal(await b.eval(`[...document.querySelectorAll('aside button')].some((x) => x.textContent.startsWith('Gửi câu trả lời'))`), true);
    s.kill();
  });
});
