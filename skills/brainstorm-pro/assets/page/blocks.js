import { btnCls, CALLOUT, CAPTION, CAP_TEXT, el, FIG, LIST, writeClipboard } from './dom.js';
import { alignRows, extLang, hlLines, linesView } from './code.js';
import { fileButton, showInViewer, zoomButton } from './viewer.js';
import { inline } from './text.js';
import { t } from './i18n.js';

const copyButton = (text) => {
  let timer;
  const btn = el('button', { type: 'button', class: btnCls('file', true), onmousedown: (e) => e.preventDefault(), onclick: async () => {
    const ok = await writeClipboard(text);
    btn.textContent = t(ok ? 'copied' : 'copyFailedShort');
    clearTimeout(timer);
    timer = setTimeout(() => { btn.textContent = 'Copy'; }, ok ? 1500 : 2000);
  } }, 'Copy');
  return btn;
};

export const codeBlock = (b) => {
  const lang = b.lang || extLang(b.file);
  const btn = fileButton(b, lang);
  const lines = hlLines(b.text || '', lang);
  return el('figure', { class: FIG + ' block bg-base-200' },
    el('figcaption', { class: CAPTION }, el('span', { class: CAP_TEXT, text: b.label || '' }), el('span', { class: 'flex items-center gap-1' }, btn, copyButton(b.text || ''))),
    linesView(lines, b.lines ? lines.map((_, i) => b.lines[0] + i) : null, null));
};

export const diffBlock = (b) => {
  const sides = [b.before || {}, b.after || {}];
  const texts = sides.map((s) => s.text || '');
  const rows = alignRows(texts[0].split('\n'), texts[1].split('\n'));
  const cols = sides.map((s, k) => {
    const lang = s.lang || b.lang || extLang(s.file);
    const hl = hlLines(texts[k], lang);
    const key = k ? 'b' : 'a';
    return el('div', { class: 'min-w-0' + (k ? ' border-l border-base-300 max-[720px]:border-l-0 max-[720px]:border-t' : '') },
      el('div', { class: CAPTION }, el('span', { class: CAP_TEXT, text: t(k ? 'after' : 'before') + (s.label ? ' · ' + s.label : '') }), fileButton(s, lang)),
      linesView(rows.map((r) => (r[key] == null ? '' : hl[r[key]])),
        s.lines ? rows.map((r) => (r[key] == null ? null : s.lines[0] + r[key])) : null,
        rows.map((r) => (r[key] == null ? 'pad' : r.same ? null : k ? 'add' : 'del'))));
  });
  return el('figure', { class: FIG + ' block bg-base-200' }, b.label ? el('figcaption', { class: CAPTION }, el('span', { class: CAP_TEXT, text: b.label })) : null,
    el('div', { class: 'grid grid-cols-2 max-[720px]:grid-cols-1' }, cols));
};

// Diagrams: Mermaid loads from the CDN only when the page has one; until it draws (or when it cannot) the source shows as text.
export const loadScript = (src) => new Promise((resolve, reject) => {
  const s = el('script', { src });
  s.onload = resolve;
  s.onerror = reject;
  document.head.append(s);
});
let mermaidLoad = null;
let mermaidQueue = Promise.resolve();
let diagrams = 0;
const loadMermaid = () => mermaidLoad || (mermaidLoad = loadScript('https://cdnjs.cloudflare.com/ajax/libs/mermaid/10.9.1/mermaid.min.js').then(() => {
  window.mermaid.initialize({ startOnLoad: false, securityLevel: 'strict', fontFamily: 'Be Vietnam Pro, system-ui, sans-serif',
    theme: matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'default' });
  return window.mermaid;
}));
export const mermaidBlock = (b) => {
  const text = b.text || '';
  const body = el('div', { class: 'p-2.5 overflow-x-auto [&>svg]:mx-auto [&>svg]:max-w-full [&>svg]:h-auto' }, linesView(hlLines(text, 'plaintext'), null, null));
  const id = 'mmd-' + ++diagrams;
  mermaidQueue = mermaidQueue.then(loadMermaid)
    .then((m) => m.parse(text).then(() => m.render(id, text)))
    .then(({ svg }) => { body.innerHTML = svg; })
    .catch(() => {
      const left = document.getElementById('d' + id);
      if (left) left.remove();
      body.prepend(el('p', { class: CALLOUT + ' mb-2', text: t('diagramFailed') }));
    });
  return el('figure', { class: FIG + ' block bg-base-100' }, b.label ? el('figcaption', { class: CAPTION }, el('span', { class: CAP_TEXT, text: b.label })) : null, body);
};

export const imgBlock = (b) => {
  const title = b.label || b.file || 'Ảnh';
  const alt = b.alt || b.label || '';
  const open = () => showInViewer(title, el('div', { class: 'overflow-auto min-h-0 p-4' }, el('img', { src: b.src, alt, class: 'max-w-none mx-auto' })));
  return el('figure', { class: FIG + ' block bg-base-100' },
    el('figcaption', { class: CAPTION }, el('span', { class: CAP_TEXT, text: title }), zoomButton(open)),
    el('div', { class: 'p-2.5' }, el('img', { src: b.src, alt, class: 'block mx-auto max-w-full max-h-[480px] w-auto h-auto object-contain cursor-zoom-in', onclick: open })));
};

// Mockups: each one is its own sandboxed page with Tailwind and daisyUI's wireframe theme; it reports its height so the frame fits it.
const MOCK_HEAD = '<!doctype html><html data-theme="wireframe"><head><meta charset="utf-8">'
  + '<link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/daisyui@5.7.47/daisyui.css">'
  + '<link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/daisyui@5.7.47/themes.css">'
  + '<script src="https://cdn.jsdelivr.net/npm/@tailwindcss/browser@4.3.3"><\/script>'
  + '<style>body{margin:0;padding:8px;font-family:system-ui,sans-serif;font-size:14px}</style></head><body>';
const MOCK_TAIL = '<script>new ResizeObserver(() => parent.postMessage({ mockH: Math.ceil(document.body.getBoundingClientRect().height) }, "*")).observe(document.body);<\/script></body></html>';
addEventListener('message', (e) => {
  if (!e.data || typeof e.data.mockH !== 'number') return;
  for (const f of document.querySelectorAll('iframe[data-mock]')) if (f.contentWindow === e.source) f.style.height = Math.min(Math.max(e.data.mockH, 24), 4000) + 'px';
});
const mockFrame = (html) => {
  const f = el('iframe', { sandbox: 'allow-scripts', 'data-mock': '', title: t('mockup'), class: 'block w-full h-20 border-0' });
  f.srcdoc = MOCK_HEAD + html + MOCK_TAIL;
  return f;
};
export const mockBlock = (html, label) => {
  const title = label || t('mockup');
  // White below the frame too, so a mockup in a taller option row reads as one box.
  return el('figure', { class: FIG + ' bg-base-100 flex flex-col items-stretch justify-start' },
    el('figcaption', { class: CAPTION }, el('span', { class: CAP_TEXT, text: title }),
      zoomButton(() => showInViewer(title, el('div', { class: 'overflow-auto min-h-0 p-4' }, mockFrame(html))))),
    el('div', { class: 'flex-1 bg-white' }, mockFrame(html)));
};

export const block = (b) => {
  switch (b && b.t) {
    case 'p': return el('p', { class: 'whitespace-pre-line' }, inline(b.text));
    case 'list': return el(b.ordered ? 'ol' : 'ul', { class: LIST + (b.ordered ? ' list-decimal' : ' list-disc') }, (b.items || []).map((i) => el('li', {}, inline(i))));
    case 'code': return codeBlock(b);
    case 'diff': return diffBlock(b);
    case 'mermaid': return mermaidBlock(b);
    case 'img': return b.src ? imgBlock(b) : null;
    case 'mockup': return mockBlock(b.html || '', b.label);
    case 'table': return el('div', { class: 'overflow-x-auto border border-base-300 rounded-box' },
      el('table', { class: 'table table-sm text-[13px] [&_thead]:whitespace-normal [&_th]:align-top [&_td]:align-top' },
        el('thead', {}, el('tr', { class: 'bg-base-200' }, (b.head || []).map((h) => el('th', { class: 'text-base-content' }, inline(h))))),
        el('tbody', {}, (b.rows || []).map((r) => el('tr', {}, r.map((c) => el('td', {}, inline(c))))))));
    case 'note': return el('div', { class: CALLOUT + ' whitespace-pre-line' }, el('strong', { class: 'text-warning', text: t('noteLead') }), inline(b.text));
    default: return b && b.text ? el('p', { class: 'whitespace-pre-line' }, inline(b.text)) : null;
  }
};
