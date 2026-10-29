import { after, before, describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { launch } from '../harness/browser.mjs';
import { GUARDED } from '../harness/page-helpers.mjs';
import { FIXTURES, outDir, render } from '../harness/serve.mjs';

const dir = outDir('doc-read');
let b;
const ready = `document.querySelectorAll('main > article').length === 5`;

describe('read-only document page', () => {
  before(async () => {
    const r = render(FIXTURES + '/doc.md', dir + '/page.html', ['--open']);
    assert.equal(r.code, 0, r.stderr);
    b = await launch();
    await b.goto('file://' + dir + '/page.html', { ready, doc: true });
  });
  after(async () => { await b.close(); });

  it('draws one card per section and a nav link per ## heading', async () => {
    const r = await b.eval(`({
      title: document.title, topic: document.querySelector('#topic').textContent,
      cards: [...document.querySelectorAll('main > article')].map((a) => (a.querySelector('h2') || {}).textContent || '(intro)'),
      h3: [...document.querySelectorAll('main h3')].map((h) => h.textContent),
      nav: [...document.querySelectorAll('#qnav a')].map((a) => a.textContent), navLabel: document.querySelector('#qnav').getAttribute('aria-label'),
      table: document.querySelectorAll('main table').length, tasks: [...document.querySelectorAll('main input[type=checkbox]')].map((i) => i.checked + ':' + i.disabled),
      lists: [document.querySelectorAll('main ul').length, document.querySelectorAll('main ol').length], mermaid: document.querySelectorAll('main figure[data-mmd]').length,
      figures: document.querySelectorAll('main figure').length, h1InBody: document.querySelectorAll('main h1').length,
    })`);
    assert.equal(r.title, 'Sample Spec');
    assert.equal(r.topic, 'Sample Spec');
    assert.deepEqual(r.cards, ['(intro)', 'Overview', 'Design', 'Risks', 'Open']);
    assert.deepEqual(r.h3, ['Goals', 'Non-goals', 'Components']);
    assert.deepEqual(r.nav, ['Overview', 'Design', 'Risks', 'Open']);
    assert.equal(r.navLabel, 'Các mục');
    assert.equal(r.table, 1);
    assert.deepEqual(r.tasks, ['false:true', 'true:true']);
    assert.deepEqual(r.lists, [2, 1], 'two bullet lists (goals, tasks) and one numbered');
    assert.equal(r.mermaid, 1);
    assert.equal(r.figures, 2, 'the ts fence and the mermaid fence');
    assert.equal(r.h1InBody, 0, 'the H1 is the page title, not a card');
  });

  it('nav links jump to their section', async () => {
    const len = await b.eval('history.length');
    await b.click('#qnav a:nth-of-type(3)');
    const href = await b.eval(`document.querySelector('#qnav a:nth-of-type(3)').getAttribute('href')`);
    await b.until(`location.hash === ${JSON.stringify(href)}`);
    assert.equal(await b.eval(`document.querySelector(location.hash).textContent`), 'Risks');
    assert.equal(await b.eval('history.length'), len + 1, 'native links keep their history entry');
    await b.click('#topic');
    assert.equal(await b.eval(GUARDED), false, 'no leave guard');
  });

  it('opens a file span and a Markdown file link in the viewer, and Esc closes it', async () => {
    await b.click('main a[title="Xem file"]');
    await b.until(`document.querySelector('dialog').open`);
    assert.match(await b.eval(`document.querySelector('dialog span').textContent`), /fixtures\/code\.ts · 6 dòng$/);
    assert.equal(await b.eval(`document.querySelector('dialog').textContent.includes('const sum = a + b;')`), true);
    await b.press('Escape');
    await b.until(`!document.querySelector('dialog').open`);
    await b.eval(`[...document.querySelectorAll('main a.link')].find((a) => a.textContent === 'the code file').click()`);
    await b.until(`document.querySelector('dialog').open`);
    assert.match(await b.eval(`document.querySelector('dialog span').textContent`), /code\.ts · 6 dòng$/);
    await b.press('Escape');
    await b.until(`!document.querySelector('dialog').open`);
  });

  it('has no review interface', async () => {
    const r = await b.eval(`({
      aside: !!document.querySelector('aside'), boxes: document.querySelectorAll('textarea').length,
      notes: document.querySelectorAll('main h2 + button, [data-note]').length, badge: !!document.querySelector('header .badge'),
      hint: document.querySelector('main').textContent.includes('Tô đen một đoạn'), banner: !!document.querySelector('main > .alert'),
    })`);
    assert.deepEqual(r, { aside: false, boxes: 0, notes: 0, badge: false, hint: false, banner: false });
    assert.deepEqual(b.unexpectedErrors(), []);
  });
});
