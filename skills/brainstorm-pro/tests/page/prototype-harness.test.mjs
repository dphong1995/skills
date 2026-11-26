import { after, before, describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { createServer } from 'node:http';
import { pathToFileURL } from 'node:url';
import { launch } from '../harness/browser.mjs';

const skill = new URL('../../', import.meta.url).pathname;
const fixture = skill + 'tests/fixtures/prototype.html';
const harnessFile = skill + 'assets/prototype/harness.html';
const pickerMd = new URL('../fixtures/picker-64eb3de.md', import.meta.url).pathname;
let b;

// This harness's goto waits for CDN scripts the prototype page does not load, so navigate directly.
const open = async (url) => {
  await b.send('Page.navigate', { url });
  await b.until(`document.readyState === 'complete' && !!document.querySelector('.proto-picker')`, { what: 'harness load' });
};
const variant = () => b.eval(`document.querySelector('#stage [data-variant]')?.dataset.variant ?? null`);
const mounted = (name) => b.until(`document.querySelector('#stage [data-variant]')?.dataset.variant === ${JSON.stringify(name)}`, { what: 'variant ' + name });
const active = () => b.eval(`[...document.querySelectorAll('.proto-picker-item[data-active]')].map((e) => e.getAttribute('aria-current'))`);
const search = () => b.eval('location.search');

describe('prototype harness', () => {
  before(async () => { b = await launch(); });
  after(async () => { await b.close(); });

  it('harness switches variants with keys', async () => {
    await open(pathToFileURL(fixture).href);
    await mounted('Quiet');
    await b.press('2');
    await mounted('Editorial');
    assert.deepEqual(await active(), ['true']);
    assert.equal(await search(), '?v=2');
    await b.press('ArrowRight'); await mounted('Playful');
    await b.press('ArrowRight'); await mounted('Quiet');
    await b.press('ArrowLeft'); await mounted('Playful');
    assert.deepEqual(b.unexpectedErrors(), []);
  });

  it('harness replays with R', async () => {
    await open(pathToFileURL(fixture).href + '?v=2');
    await mounted('Editorial');
    await b.eval(`document.querySelector('#stage [data-variant]').setAttribute('data-marker', '1')`);
    await b.press('r');
    await b.until(`!document.querySelector('#stage [data-variant]')?.hasAttribute('data-marker') && !!document.querySelector('#stage [data-variant]')`, { what: 're-mount' });
    assert.equal(await variant(), 'Editorial');
    assert.deepEqual(b.unexpectedErrors(), []);
  });

  it('harness falls back to variant 1 on an out-of-range v', async () => {
    await open(pathToFileURL(fixture).href + '?v=9');
    await mounted('Quiet');
    assert.deepEqual(await active(), ['true']);
    assert.deepEqual(b.unexpectedErrors(), []);
  });

  it('harness runs delegated variant listeners', async () => {
    await open(pathToFileURL(fixture).href + '?v=3');
    await mounted('Playful');
    await b.click('[data-toggle]');
    await b.until(`document.querySelector('[data-toggle]').hasAttribute('data-on')`, { what: 'data-on' });
    assert.deepEqual(b.unexpectedErrors(), []);
  });

  // The interview page is served over http, so its blob URL is blob:http://…, where history.replaceState with a new query throws.
  it('harness switches variants when opened from a blob URL made by an http page', async () => {
    const server = createServer((_req, res) => res.end('<!doctype html><title>opener</title>'));
    await new Promise((r) => server.listen(0, '127.0.0.1', r));
    server.unref();
    await b.send('Page.navigate', { url: 'http://127.0.0.1:' + server.address().port + '/' });
    await b.until(`document.readyState === 'complete' && document.title === 'opener'`);
    const html = readFileSync(fixture, 'utf8');
    await b.eval(`location.href = URL.createObjectURL(new Blob([${JSON.stringify(html)}], { type: 'text/html' }))`);
    await b.until(`location.href.startsWith('blob:http') && !!document.querySelector('.proto-picker')`, { what: 'blob load' });
    await mounted('Quiet');
    await b.press('2');
    await mounted('Editorial');
    assert.deepEqual(await active(), ['true']);
    server.close();
  });

  it('harness ships the verbatim picker', () => {
    const md = readFileSync(pickerMd, 'utf8');
    const norm = (s) => s.replace(/\s+/g, ' ').trim();
    const block = (lang) => md.match(new RegExp('```' + lang + '\\n([\\s\\S]*?)```', 'g')).map((x) => x.replace(/^```\w+\n|```$/g, ''));
    const markup = block('html')[0], css = block('css')[0], js = block('js')[0];
    for (const f of [harnessFile, fixture]) {
      // The one allowed deviation: replaceState wrapped in try/catch (it throws on a blob:http page).
      const text = norm(readFileSync(f, 'utf8')).replace("try { history.replaceState(null, '', url); } catch {}", "history.replaceState(null, '', url);");
      for (const part of [markup, css, js]) assert.ok(text.includes(norm(part)), f + ' lacks a verbatim picker part: ' + norm(part).slice(0, 60));
    }
    assert.match(readFileSync(harnessFile, 'utf8'), /Copyright \(c\) 2026 Emil Kowalski/);
  });
});
