// Page-side helper scripts, evaluated in the page under test, and expressions shared by the test files.

// What `ready` waits for in b.goto: the interview page, the read-only document and the review page.
export const READY = {
  interview: `!!document.querySelector('#card-q02') && !!document.querySelector('aside')`,
  doc: `document.querySelectorAll('main > article').length >= 5`,
  review: `document.querySelectorAll('main > article').length >= 5 && !!document.querySelector('[aria-label="Góp ý: Chung"]')`,
};

// The floating bar's message as a page expression (b.status() reads it).
export const STATUS = `document.querySelector('aside p[role=status]').textContent`;

// A key hint with the platform's spelling replaced: MOD for ⌘ Enter / Ctrl Enter, CHORD for ⌘⇧ Enter / Ctrl Shift Enter.
export const hint = (t) => t.replace(/⌘⇧ Enter|Ctrl Shift Enter/, 'CHORD').replace(/⌘ Enter|Ctrl Enter/, 'MOD');

// window.mk(color) makes a real PNG; window.send(sel, 'paste' | 'drop', color, type, size) puts it into a textarea as a clipboard or drop event and resolves
// with the bytes sent, whether the event was prevented and the textarea's value once it changed (or after 2 s).
export const IMAGE_HELPERS = `window.mk = (c) => new Promise((res) => { const cv = document.createElement('canvas'); cv.width = 40; cv.height = 30;
  const g = cv.getContext('2d'); g.fillStyle = c; g.fillRect(0, 0, 40, 30); cv.toBlob(async (bl) => res(new Uint8Array(await bl.arrayBuffer())), 'image/png'); });
window.send = async (sel, kind, color, type, size) => {
  const ta = document.querySelector(sel); const bytes = await mk(color);
  const dt = new DataTransfer(); dt.items.add(new File([size ? new Blob([new ArrayBuffer(size)]) : bytes], 'a.png', { type: type || 'image/png' }));
  const before = ta.value;
  const ev = kind === 'paste' ? new ClipboardEvent('paste', { clipboardData: dt, bubbles: true, cancelable: true }) : new DragEvent('drop', { dataTransfer: dt, bubbles: true, cancelable: true });
  ta.dispatchEvent(ev);
  for (let i = 0; i < 40 && ta.value === before; i++) await new Promise((r) => setTimeout(r, 50));
  return { b64: btoa(String.fromCharCode(...bytes)), prevented: ev.defaultPrevented, value: ta.value };
};`;

// window.T: selecting a phrase of the document and reading the comment boxes of the review page.
export const PASSAGE_HELPERS = `window.T = {
  node(phrase) {
    const w = document.createTreeWalker(document.querySelector('#main'), NodeFilter.SHOW_TEXT);
    for (let t; (t = w.nextNode());) { if (t.parentElement.closest('[data-note], button')) continue; const i = t.data.indexOf(phrase); if (i >= 0) return [t, i]; }
    throw new Error('not found: ' + phrase);
  },
  select(a, b) {
    const [t1, i1] = T.node(a); const [t2, i2] = b ? T.node(b) : [t1, i1];
    t1.parentElement.scrollIntoView({ block: 'center' });
    const r = document.createRange(); r.setStart(t1, i1); r.setEnd(t2, i2 + (b || a).length);
    const s = getSelection(); s.removeAllRanges(); s.addRange(r);
  },
  // Where a mouse drag selects the phrase: from its first character to its last.
  points(phrase) {
    const [t, i] = T.node(phrase);
    t.parentElement.scrollIntoView({ block: 'center' });
    const r = document.createRange(); r.setStart(t, i); r.setEnd(t, i + phrase.length);
    const rs = r.getClientRects(); const a = rs[0], z = rs[rs.length - 1];
    return { from: { x: a.left + 1, y: a.top + a.height / 2 }, to: { x: z.right - 1, y: z.top + z.height / 2 } };
  },
  adder() { return [...document.querySelectorAll('body > button.absolute')].find((b) => !b.hidden) ? 'shown' : 'hidden'; },
  boxes() { return [...document.querySelectorAll('[data-hl]')].map((b) => b.dataset.hl); },
  ta(id) { return document.querySelector('[data-hl="' + id + '"] textarea'); },
  type(id, text) { const ta = T.ta(id); ta.value = text; ta.dispatchEvent(new Event('input', { bubbles: true })); },
  marks(id) { return document.querySelectorAll('mark[data-c="' + id + '"]').length; },
};`;
