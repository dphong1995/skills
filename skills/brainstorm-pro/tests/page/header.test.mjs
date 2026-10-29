// The header keeps the rows the page had before the `?` button: the button shares a row with another header item and adds no height.
// The heights are those of the page before the shortcuts work (commit 114d1fd), measured in this headless Chrome.
import { after, before, describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { writeFileSync } from 'node:fs';
import { launch } from '../harness/browser.mjs';
import { READY } from '../harness/page-helpers.mjs';
import { outDir, render } from '../harness/serve.mjs';

const dir = outDir('header');
const BTN = 'header button[title="Phím tắt"]';
const LONG_TITLE = '# A rather long specification title that will need to wrap on a phone screen for sure\n\nIntro.\n\n## Overview\n\nText.\n\n## Design\n\nMore.\n\n## Risks\n\nR.\n';
const questions = Array.from({ length: 9 }, (_, i) => ({ id: 'q0' + (i + 1), n: i + 1, title: 'Question ' + (i + 1), options: [{ id: 'a', label: 'A', means: 'a' }, { id: 'b', label: 'B', means: 'b' }] }));

const pages = {};
const build = (name, src, isRound) => {
  const f = `${dir}/${name}`;
  writeFileSync(`${f}.${isRound ? 'json' : 'md'}`, isRound ? JSON.stringify({ root: dir, ...src }) : src);
  assert.equal(render(`${f}.${isRound ? 'json' : 'md'}`, `${f}.html`).code, 0);
  pages[name] = { url: `file://${f}.html`, ready: isRound ? READY.interview.replace('#card-q02', '#card-q09') : `document.querySelectorAll('main > article').length >= 3` };
};
build('long-doc', LONG_TITLE, false);
build('nine', { topic: 'Round with many questions', questions }, true);
build('nine-long', { topic: 'A long round topic that takes quite some room in the header bar', questions }, true);

// Rects of the header, the button and the last item of the nav (the last chip, or the section link).
const measure = () => b.eval(`(() => { const r = (e) => { const x = e.getBoundingClientRect(); return { top: Math.round(x.top), bottom: Math.round(x.bottom), height: Math.round(x.height) }; };
  const nav = document.querySelector('#qnav'); const last = nav.querySelector('a:last-of-type');
  return { header: r(document.querySelector('header')), btn: r(document.querySelector('${BTN}')), last: r(last) }; })()`);
let b;
const check = async (name, height) => {
  await b.goto(pages[name].url, { ready: pages[name].ready, doc: name === 'long-doc' });
  const m = await measure();
  assert.ok(Math.abs(m.header.height - height) <= 1, `${name}: header is ${m.header.height} px, the old page's is ${height}`);
  assert.ok(m.btn.top < m.last.bottom && m.last.top < m.btn.bottom, `${name}: the ? button shares its row with the last nav item`);
};

for (const [width, doc, nine, long] of [[390, 90, 95, 116], [600, 90, null, null], [1400, 69, 41, 41]]) {
  describe(`header at ${width} px`, () => {
    before(async () => { b = await launch({ width }); });
    after(async () => { await b.close(); });
    it('a read-only document with a wrapping title keeps its height', () => check('long-doc', doc));
    if (nine != null) {
      it('an interview with 9 questions keeps its height', () => check('nine', nine));
      it('an interview with a long topic keeps its height', () => check('nine-long', long));
    }
  });
}
