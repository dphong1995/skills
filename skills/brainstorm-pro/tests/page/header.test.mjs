// The `?` button adds no header height: the header is as tall with it as without it. The floating bar fits a phone.
// The scroll padding follows the header, so a heading never lands under it.
import { after, before, describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { writeFileSync } from 'node:fs';
import { launch, sleep } from '../harness/browser.mjs';
import { READY } from '../harness/page-helpers.mjs';
import { outDir, render, serve } from '../harness/serve.mjs';

const dir = outDir('header');
const BTN = 'header button[title="Phím tắt"]';
const LONG_TITLE = '# A rather long specification title that will need to wrap on a phone screen for sure\n\nIntro.\n\n## Overview\n\nText.\n\n## Design\n\nMore.\n\n## Risks\n\nR.\n';
const NO_SECTIONS = '# Title with no sections\n\nJust a paragraph.\n\n### A sub heading\n\nMore.\n';
const questions = Array.from({ length: 9 }, (_, i) => ({ id: 'q0' + (i + 1), n: i + 1, title: 'Question ' + (i + 1), options: [{ id: 'a', label: 'A', means: 'a' }, { id: 'b', label: 'B', means: 'b' }] }));
const DOC_READY = `document.querySelectorAll('main > article').length >= 1`;
const NINE_READY = READY.interview.replace('#card-q02', '#card-q09');

// name -> the page's source (Markdown text or round), what to wait for, and whether it is a review (served).
const sources = {
  'long-doc': { text: LONG_TITLE, ready: DOC_READY, doc: true },
  'review-doc': { text: LONG_TITLE, ready: READY.review.replace('>= 5', '>= 3'), doc: true, review: true },
  'no-sections': { text: NO_SECTIONS, ready: DOC_READY, doc: true },
  'no-sections-review': { text: NO_SECTIONS, ready: DOC_READY, doc: true, review: true },
  nine: { round: { topic: 'Round with many questions', questions }, ready: NINE_READY },
  'nine-long': { round: { topic: 'A long round topic that takes quite some room in the header bar', questions }, ready: NINE_READY },
};
const servers = [];
let b;
const open = async (name, { serving = false } = {}) => {
  const x = sources[name];
  const f = `${dir}/${name}`;
  writeFileSync(`${f}.${x.round ? 'json' : 'md'}`, x.round ? JSON.stringify({ root: dir, ...x.round }) : x.text);
  let url;
  if (x.review || serving) {
    const s = serve(`${f}.${x.round ? 'json' : 'md'}`, `${f}-served.html`);
    servers.push(s);
    url = await s.url;
  } else {
    assert.equal(render(`${f}.${x.round ? 'json' : 'md'}`, `${f}.html`, x.round ? [] : ['--open']).code, 0);
    url = `file://${f}.html`;
  }
  // A CDN request that stalls the load event is retried once (the page itself is not under test then).
  await b.goto(url, { ready: x.ready, doc: !!x.doc }).catch((e) => { if (!/page load/.test(e.message)) throw e; return b.goto(url, { ready: x.ready, doc: !!x.doc }); });
  await b.eval('document.fonts.ready.then(() => 1)');
  await sleep(200);
};
const rect = (sel) => `(() => { const x = document.querySelector('${sel}').getBoundingClientRect(); return { top: x.top, bottom: x.bottom, height: x.height }; })()`;
const frame = () => b.eval('new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)))');
const headerHeight = async () => (await b.eval(rect('header'))).height;
const stopServers = () => { while (servers.length) servers.pop().kill(); };
// Waits until the page has not scrolled for 300 ms.
const still = () => b.eval(`new Promise((r) => { let last = -1, n = 0; (function f() { n = scrollY === last ? n + 1 : 0; last = scrollY; n >= 6 ? r(scrollY) : setTimeout(f, 50); })(); })`);

// The contract: the header is as tall with the button as without it, and the button sits on the title row (no sections) or beside the last nav item.
const sameHeight = async (name) => {
  await open(name);
  const withBtn = await headerHeight();
  const m = await b.eval(`(() => { const r = (e) => e.getBoundingClientRect(); const btn = r(document.querySelector('${BTN}'));
    const last = document.querySelector('#qnav a:last-of-type'); const h1 = r(document.querySelector('#topic'));
    return { btn: { top: btn.top, bottom: btn.bottom }, last: last && { top: r(last).top, bottom: r(last).bottom }, h1: { top: h1.top, bottom: h1.bottom } }; })()`);
  if (m.last) assert.ok(m.btn.top < m.last.bottom && m.last.top < m.btn.bottom, `${name}: the ? button shares its row with the last nav item`);
  else assert.ok(m.btn.top < m.h1.bottom && m.h1.top < m.btn.bottom, `${name}: with no nav the ? button sits on the title row`);
  await b.eval(`document.querySelector('${BTN}').remove()`);
  await frame();
  assert.equal(await headerHeight(), withBtn, `${name}: the header is ${withBtn} px with the ? button and ${await headerHeight()} without it`);
};

for (const [width, names] of [[390, ['long-doc', 'review-doc', 'no-sections', 'no-sections-review', 'nine', 'nine-long']], [600, ['long-doc', 'review-doc', 'no-sections']], [1400, ['long-doc', 'review-doc', 'no-sections', 'nine', 'nine-long']]]) {
  describe(`header at ${width} px`, () => {
    before(async () => { b = await launch({ width }); });
    after(async () => { stopServers(); await b.close(); });
    for (const name of names) it(`${name}: the ? button adds no height`, () => sameHeight(name));
  });
}

// The floating bar: every button inside the viewport at a phone width, with the Mac and the other key hint.
describe('floating bar at 390 px', () => {
  before(async () => { b = await launch({ width: 390 }); });
  after(async () => { stopServers(); await b.close(); });
  for (const platform of ['MacIntel', 'Win32']) {
    for (const [name, serving] of [['review-doc', true], ['nine', false], ['nine', true]]) {
      it(`${name}${serving ? ' (sending)' : ''} on ${platform}: the buttons end inside the viewport`, async () => {
        await b.send('Emulation.setUserAgentOverride', { userAgent: 'Mozilla/5.0', platform });
        await open(name, { serving });
        const r = await b.eval(`(() => { const btns = [...document.querySelectorAll('aside button')]; return { n: btns.length, plat: navigator.platform, vw: innerWidth, right: Math.max(...btns.map((x) => x.getBoundingClientRect().right)) }; })()`);
        assert.equal(r.plat, platform);
        assert.ok(r.n >= 2, 'the bar has its buttons');
        if (name === 'review-doc') assert.equal(await b.eval(`[...document.querySelectorAll('aside button kbd')].every((k) => getComputedStyle(k).display === 'none')`), true, 'the key hints are hidden on a phone');
        assert.ok(r.right <= r.vw, `last button ends at ${r.right} px, the viewport is ${r.vw} px`);
      });
    }
  }
});

// The scroll padding follows the header height: a heading reached by a link or a key lands below the header.
describe('scroll padding follows the header', () => {
  before(async () => { b = await launch({ width: 390 }); });
  after(async () => { stopServers(); await b.close(); });
  const landsBelow = async (what) => {
    await still();
    const t = await b.eval(`(() => { const h = document.querySelector('header').getBoundingClientRect().bottom; return { h, top: window.__target().getBoundingClientRect().top }; })()`);
    assert.ok(t.top >= t.h - 0.5, `${what}: the target's top is ${t.top} px, under the header that ends at ${t.h} px`);
  };
  for (const [width, name] of [[390, 'nine'], [760, 'nine'], [390, 'review-doc'], [520, 'long-doc']]) {
    it(`${name} at ${width} px: a nav link and the 3 key land below the header`, async () => {
      await b.send('Emulation.setDeviceMetricsOverride', { width, height: 800, deviceScaleFactor: 1, mobile: false });
      await open(name);
      await b.eval(`window.__target = () => { const a = document.querySelectorAll('#qnav a')[2]; return document.querySelector(a.getAttribute('href')); }`);
      assert.ok((await b.eval(`parseFloat(getComputedStyle(document.documentElement).scrollPaddingTop)`)) >= (await headerHeight()), 'the padding is at least the header height');
      await b.eval(`document.querySelectorAll('#qnav a')[2].click()`);
      await landsBelow('nav link');
      await b.eval(`scrollTo({ top: 0, behavior: 'instant' }); document.activeElement.blur()`);
      await sleep(300);
      await b.press('3');
      await landsBelow('key 3');
    });
  }
});
