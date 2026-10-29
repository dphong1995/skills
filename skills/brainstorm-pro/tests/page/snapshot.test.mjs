// The DOM of each page after load, as text. A snapshot is written when missing or when UPDATE_SNAPSHOTS=1; otherwise the page must match it.
import { after, before, describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { launch } from '../harness/browser.mjs';
import { READY } from '../harness/page-helpers.mjs';
import { FIXTURES, TESTS, outDir, render, serve, serveRound2 } from '../harness/serve.mjs';
import { writeRound } from '../fixtures/round.mjs';

const dir = outDir('snapshot');
const SNAPSHOTS = TESTS + '/snapshots';
const update = process.env.UPDATE_SNAPSHOTS === '1';
let b;

// Mermaid drawings, mockup frame heights and machine-specific paths and tokens are replaced; everything else is the page as built.
const BODY = `(() => {
  const c = document.body.cloneNode(true);
  for (const x of c.querySelectorAll('script')) x.remove();
  for (const f of c.querySelectorAll('iframe[data-mock]')) f.removeAttribute('style');
  for (const d of c.querySelectorAll('div[class*="[&>svg]:mx-auto"]')) d.innerHTML = '<!--mermaid drawing-->';
  return { html: c.outerHTML, tags: [...document.body.children].map((e) => e.tagName.toLowerCase() + (e.id ? '#' + e.id : '')).join('\\n') };
})()`;
const drawn = `[...document.querySelectorAll('div[class*="[&>svg]:mx-auto"]')].every((d) => d.querySelector(':scope > svg, :scope > p.alert'))`;

const normalize = (html) => html
  .replaceAll(FIXTURES, '<FIXTURES>').replaceAll(FIXTURES.replace(homedir(), '~'), '<FIXTURES>')
  .replace(/mmd-\d+/g, 'mmd-N').replace(/[0-9a-f]{32}/g, 'TOKEN')
  .replace(/></g, '>\n<');

const check = (name, actual) => {
  const file = `${SNAPSHOTS}/${name}`;
  if (update || !existsSync(file)) { mkdirSync(SNAPSHOTS, { recursive: true }); writeFileSync(file, actual); return; }
  const want = readFileSync(file, 'utf8');
  if (want === actual) return;
  writeFileSync(`${dir}/${name}`, actual);
  const [a, w] = [actual.split('\n'), want.split('\n')];
  const i = a.findIndex((l, k) => l !== w[k]);
  assert.fail(`${name} differs from its snapshot at line ${i + 1}; actual is in tests/out/snapshot/${name}\n  snapshot: ${(w[i] || '').slice(0, 300)}\n  actual:   ${(a[i] || '').slice(0, 300)}`);
};

const snap = async (name, target, ready, doc) => {
  await b.goto(target, { ready: ready + ' && ' + drawn, doc });
  const { html, tags } = await b.eval(BODY);
  check(name + '.html', normalize(html) + '\n');
  check(name + '.tags.txt', tags + '\n');
};

describe('page snapshots', () => {
  before(async () => { b = await launch(); });
  after(async () => { await b.close(); });

  it('the interview page opened as a file', async () => {
    const out = dir + '/interview/round.html';
    assert.equal(render(writeRound(outDir('snapshot/interview')), out).code, 0);
    await snap('interview', 'file://' + out, `!!document.querySelector('#card-q02')`, false);
  });

  it('the interview page served', async () => {
    const d = outDir('snapshot/interview-served');
    const s = serve(writeRound(d), d + '/round.html');
    try { await snap('interview-served', await s.url, `!!document.querySelector('#card-q02')`, false); } finally { s.kill(); }
  });

  it('the read-only document', async () => {
    const d = outDir('snapshot/doc-read');
    assert.equal(render(FIXTURES + '/doc.md', d + '/page.html', ['--open']).code, 0);
    await snap('doc-read', 'file://' + d + '/page.html', READY.doc, true);
  });

  it('review round 1', async () => {
    const d = outDir('snapshot/review-1');
    const s = serve(FIXTURES + '/doc.md', d + '/review-01.html');
    try { await snap('review-1', await s.url, READY.review, true); } finally { s.kill(); }
  });

  it('review round 2', async () => {
    const d = outDir('snapshot/review-2');
    const s = await serveRound2(d);
    try { await snap('review-2', await s.url, READY.doc + ` && !!document.querySelector('main > .alert')`, true); } finally { s.kill(); }
  });
});
