// Headless Chrome driven over the DevTools protocol with Node's built-in WebSocket: no npm packages.
// Run the suite from the skill directory (Node 24 does not expand a directory argument, so the glob is quoted):
//   node --test --test-concurrency=1 'tests/**/*.test.mjs'
// One file: node --test tests/page/passages.test.mjs. UPDATE_SNAPSHOTS=1 rewrites tests/snapshots/. CHROME=<path> picks another Chrome.
import { spawn } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { isTyping } from '../../assets/page/keys.js';
import { STATUS } from './page-helpers.mjs';

const CHROME = process.env.CHROME || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
export const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

// Counts every write to a <style> element's text: Tailwind's browser build rewrites its sheet that way when it meets a class it has not compiled.
const TW_HOOK = `window.__tw = { n: 0 };
const ts = Object.getOwnPropertyDescriptor(Node.prototype, 'textContent');
Object.defineProperty(Node.prototype, 'textContent', { get: ts.get, set(v) { if (this.tagName === 'STYLE') __tw.n++; return ts.set.call(this, v); } });`;

// macOS beeps for every key event a page leaves unconsumed. This window listener runs after the page's own (bubble phase) and consumes a key outside a text box
// that the page left alone, except keys with a default action and modifier combos. It records every key in window.__keys, with whether the page prevented it.
const KEY_HOOK = `(() => {
const isTyping = ${isTyping};
addEventListener('keydown', (e) => {
  const mod = e.metaKey || e.ctrlKey || e.altKey;
  const own = ['ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown', 'Tab', ' ', 'Enter', 'PageUp', 'PageDown', 'Home', 'End', 'Escape'].includes(e.key);
  (window.__keys = window.__keys || []).push({ key: e.key, pagePrevented: e.defaultPrevented });
  if (e.defaultPrevented || isTyping(e.target) || mod || own) return;
  e.preventDefault();
}, false);
})();`;

const MODS = { alt: 1, ctrl: 2, meta: 4, shift: 8 };
const NAMED = { Enter: 13, Escape: 27, Tab: 9, Backspace: 8, ArrowLeft: 37, ArrowUp: 38, ArrowRight: 39, ArrowDown: 40 };

export async function launch({ dark = false, width = 1400, height = 1000 } = {}) {
  const dir = mkdtempSync(join(tmpdir(), 'bp-chrome-'));
  const proc = spawn(CHROME, ['--headless=new', '--remote-debugging-port=0', '--user-data-dir=' + dir, '--no-first-run',
    '--no-default-browser-check', `--window-size=${width},${height}`, 'about:blank'], { stdio: 'ignore' });
  const exited = new Promise((r) => proc.on('exit', r));
  let port;
  for (let i = 0; i < 200 && !port; i++) {
    try { port = readFileSync(dir + '/DevToolsActivePort', 'utf8').split('\n')[0]; } catch (e) { await sleep(100); }
  }
  let target;
  for (let i = 0; i < 50 && !target; i++) {
    try { target = (await (await fetch(`http://127.0.0.1:${port}/json`)).json()).find((t) => t.type === 'page'); } catch (e) { await sleep(100); }
  }
  if (!target) { proc.kill(); throw new Error('Chrome did not start (CHROME=' + CHROME + ')'); }
  const ws = new WebSocket(target.webSocketDebuggerUrl);
  await new Promise((r) => { ws.onopen = r; });
  let id = 0;
  const pending = new Map();
  const errors = [];
  let leaveDialogs = 0;
  ws.onmessage = (m) => {
    const d = JSON.parse(m.data);
    if (d.id && pending.has(d.id)) { pending.get(d.id)(d); pending.delete(d.id); return; }
    // The leave guard asks before a reload or navigation (Page.navigate waits for the answer); the harness leaves, like a click on Leave.
    if (d.method === 'Page.javascriptDialogOpening' && d.params.type === 'beforeunload') {
      leaveDialogs++;
      send('Page.handleJavaScriptDialog', { accept: true }).catch(() => {});
    }
    if (d.method === 'Runtime.exceptionThrown') errors.push(d.params.exceptionDetails.exception?.description || d.params.exceptionDetails.text);
    if (d.method === 'Runtime.consoleAPICalled' && d.params.type === 'error') errors.push(d.params.args.map((a) => a.value ?? a.description).join(' '));
    if (d.method === 'Log.entryAdded' && d.params.entry.level === 'error') errors.push(d.params.entry.text + ' ' + (d.params.entry.url || ''));
  };
  const send = (method, params = {}) => new Promise((res, rej) => {
    const i = ++id;
    pending.set(i, (d) => (d.error ? rej(new Error(method + ': ' + d.error.message)) : res(d.result)));
    ws.send(JSON.stringify({ id: i, method, params }));
  });
  await send('Runtime.enable');
  await send('Page.enable');
  await send('Log.enable');
  await send('Emulation.setEmulatedMedia', { features: [{ name: 'prefers-color-scheme', value: dark ? 'dark' : 'light' }] });
  await send('Emulation.setDeviceMetricsOverride', { width, height, deviceScaleFactor: 1, mobile: false });
  await send('Emulation.setFocusEmulationEnabled', { enabled: true });
  await send('Page.addScriptToEvaluateOnNewDocument', { source: TW_HOOK });
  await send('Page.addScriptToEvaluateOnNewDocument', { source: KEY_HOOK });
  // Pages close their own tab after sending; the harness drives a single tab.
  await send('Page.addScriptToEvaluateOnNewDocument', { source: 'window.__closed = 0; window.close = () => { window.__closed++; };' });

  const evaluate = async (expr) => {
    const r = await send('Runtime.evaluate', { expression: expr, awaitPromise: true, returnByValue: true });
    if (r.exceptionDetails) throw new Error(r.exceptionDetails.exception?.description || r.exceptionDetails.text);
    return r.result.value;
  };
  // Polls a page expression until it is truthy; throws with the last value when the time is up.
  const until = async (expr, { timeout = 8000, interval = 50, what = expr } = {}) => {
    const t0 = Date.now();
    let last;
    for (;;) {
      try { last = await evaluate(expr); if (last) return last; } catch (e) { last = e.message; }
      if (Date.now() - t0 > timeout) throw new Error(`until timed out after ${timeout} ms: ${what} (last value: ${JSON.stringify(last)})`);
      await sleep(interval);
    }
  };
  const mouse = (type, x, y, extra = {}) => send('Input.dispatchMouseEvent', { type, x, y, button: 'left', ...extra });
  // Scrolls the element to the middle and waits until its box stops moving (images, diagrams and mockups resize the page as they load).
  const centerOf = (sel) => evaluate(`(async () => {
    const e = document.querySelector(${JSON.stringify(sel)});
    if (!e) throw new Error('no element: ' + ${JSON.stringify(sel)});
    const frame = () => new Promise((r) => requestAnimationFrame(r));
    let last = '';
    for (let i = 0, same = 0; i < 120 && same < 4; i++) {
      e.scrollIntoView({ block: 'center' });
      await frame();
      const r = e.getBoundingClientRect();
      const now = [r.x, r.y, r.width, r.height].join();
      same = now === last ? same + 1 : 0;
      last = now;
    }
    const r = e.getBoundingClientRect();
    return { x: r.x + r.width / 2, y: r.y + r.height / 2 };
  })()`);

  const api = {
    send, eval: evaluate, until, errors, mouse,
    // Console errors and exceptions, without the favicon request every file page makes.
    leaveDialogs: () => leaveDialogs,
    unexpectedErrors: () => errors.filter((e) => !/favicon/.test(e)),
    // Waits for the load event and the caller's `ready` expression; `doc` pages also need marked from the CDN.
    goto: async (url, { ready = 'true', doc = false } = {}) => {
      await send('Page.navigate', { url });
      await until(`document.readyState === 'complete' && location.href !== 'about:blank'`, { timeout: 20000, what: 'page load' });
      const cdn = await until(`(async () => {
        const t0 = Date.now();
        while (Date.now() - t0 < 12000) {
          if (!window.hljs) return 'hljs';
          const tw = [...document.querySelectorAll('style')].some((s) => s.textContent.includes('tailwindcss v4'));
          if (tw && (!${doc} || window.marked)) return 'ok';
          if (document.body.innerText.includes('Không dựng được trang từ Markdown')) return 'marked';
          await new Promise((r) => setTimeout(r, 50));
        }
        return window.marked || !${doc} ? 'tailwind' : 'marked';
      })()`, { timeout: 20000, what: 'CDN scripts' }).catch(() => 'hljs');
      if (cdn !== 'ok') throw new Error('CDN not loaded (offline?): ' + cdn);
      await until(ready, { timeout: 20000, what: 'page ready: ' + ready });
    },
    shot: async (path, full = false) => {
      const r = await send('Page.captureScreenshot', { format: 'png', captureBeyondViewport: full });
      writeFileSync(path, Buffer.from(r.data, 'base64'));
    },
    // A real mouse click at the center of the first element matching the selector.
    click: async (sel) => {
      const p = await centerOf(sel);
      await mouse('mouseMoved', p.x, p.y);
      for (const type of ['mousePressed', 'mouseReleased']) await mouse(type, p.x, p.y, { clickCount: 1 });
    },
    // Press at `from`, move in steps to `to` (pausing midway for pauseMs, calling onPause then), release.
    drag: async (from, to, { steps = 8, pauseMs = 0, onPause, release = true } = {}) => {
      await mouse('mousePressed', from.x, from.y, { clickCount: 1 });
      for (let i = 1; i <= steps; i++) {
        await mouse('mouseMoved', from.x + ((to.x - from.x) * i) / steps, from.y + ((to.y - from.y) * i) / steps, { buttons: 1 });
        await sleep(16);
        if (i === steps / 2 && pauseMs) { await sleep(pauseMs); if (onPause) await onPause(); }
      }
      if (release) await mouse('mouseReleased', to.x, to.y, { clickCount: 1 });
    },
    // One key as the browser sends it: press('Enter', { meta: true }), press('j'), press('ArrowRight', { shift: true }). A printable key is one keyDown with its text, as Puppeteer sends it.
    press: async (key, mods = {}) => {
      const modifiers = Object.entries(MODS).reduce((m, [k, v]) => m | (mods[k] ? v : 0), 0);
      const printable = key.length === 1;
      const code = printable ? (/[a-z]/i.test(key) ? 'Key' + key.toUpperCase() : /\d/.test(key) ? 'Digit' + key : '') : key;
      const vk = NAMED[key] || (printable ? key.toUpperCase().charCodeAt(0) : 0);
      const base = { key, code, windowsVirtualKeyCode: vk, nativeVirtualKeyCode: vk, modifiers };
      const text = printable && !mods.ctrl && !mods.meta;
      await send('Input.dispatchKeyEvent', { type: text ? 'keyDown' : 'rawKeyDown', ...base, ...(text ? { text: key } : {}) });
      await send('Input.dispatchKeyEvent', { type: 'keyUp', ...base });
    },
    type: (text) => send('Input.insertText', { text }),
    twWrites: () => evaluate('__tw.n'),
    twReset: () => evaluate('(__tw.n = 0, 1)'),
    // Waits until Tailwind has stopped rewriting its sheet after a load, then counts from zero.
    twSettle: async () => {
      let n;
      do { n = await api.twWrites(); await sleep(500); } while (n !== (await api.twWrites()));
      await api.twReset();
    },
    // The floating bar's message.
    status: () => evaluate(STATUS),
    close: async () => {
      try { ws.close(); } catch (e) { /* already closed */ }
      proc.kill();
      await Promise.race([exited, sleep(5000)]);
      rmSync(dir, { recursive: true, force: true, maxRetries: 5, retryDelay: 200 });
    },
  };
  return api;
}
