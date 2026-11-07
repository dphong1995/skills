import { el, THUMB, THUMB_DEL, THUMB_IMG, THUMB_NUM } from './dom.js';
import { say } from './fab.js';
import { showInViewer } from './viewer.js';
import { t } from './i18n.js';

// Images pasted or dropped into a text box: the serve script saves each one and the text gets [Image #N] at the caret. Only references still in the text are sent, as "Image #N: <path>" lines.
export const images = new Map();
const IMG_TYPES = ['image/png', 'image/jpeg', 'image/gif', 'image/webp'];
const IMG_MAX = 20 * 1024 * 1024;
const IMG_REF = /\[(?:Image|Ảnh) #(\d+)\]/g;
const imgRefs = (text) => [...new Set([...text.matchAll(IMG_REF)].map((m) => +m[1]))].filter((n) => images.has(n));
export const imgLines = (text) => imgRefs(text).map((n) => 'Image #' + n + ': ' + images.get(n).path);
// One thumbnail per reference in the linked boxes (a hidden box does not count, its text is not sent); × removes the reference from the text.
export const imageStrip = (tas) => {
  const strip = el('div', { class: 'flex flex-wrap gap-1.5 empty:hidden' });
  const thumb = (n) => {
    const { url } = images.get(n);
    const title = t('thumb', n);
    return el('figure', { class: THUMB },
      el('img', { src: url, alt: title, title, class: THUMB_IMG, onclick: () => {
        showInViewer(title, el('div', { class: 'overflow-auto min-h-0 p-4' }, el('img', { src: url, alt: title, class: 'max-w-none mx-auto' })));
      } }),
      el('span', { class: THUMB_NUM, text: '#' + n }),
      el('button', { type: 'button', class: THUMB_DEL, 'aria-label': t('removeImage', title), onclick: () => {
        for (const ta of tas) {
          const ref = new RegExp('\\[(?:Image|Ảnh) #' + n + '\\]', 'g');
          if (!ref.test(ta.value)) continue;
          ta.value = ta.value.replace(ref, '');
          ta.dispatchEvent(new Event('input', { bubbles: true }));
        }
      } }, '×'));
  };
  strip.refresh = () => {
    const ns = imgRefs(tas.map((ta) => (ta.hidden ? '' : ta.value)).join('\n'));
    if (strip.dataset.refs === ns.join()) return;
    strip.dataset.refs = ns.join();
    strip.replaceChildren(...ns.map(thumb));
  };
  for (const ta of tas) ta.addEventListener('input', strip.refresh);
  return strip;
};
export const imageInput = (ta, strip) => {
  const take = async (files) => {
    for (const file of files) {
      if (!IMG_TYPES.includes(file.type)) { say('warn', t('imageTypes')); continue; }
      if (file.size > IMG_MAX) { say('warn', t('imageTooBig')); continue; }
      try {
        const r = await fetch(D.upload, { method: 'POST', headers: { 'Content-Type': file.type }, body: file });
        if (!r.ok) throw new Error('HTTP ' + r.status);
        const { n, path, url } = await r.json();
        images.set(n, { path, url });
        ta.setRangeText('[Image #' + n + ']', ta.selectionStart, ta.selectionEnd, 'end');
        ta.dispatchEvent(new Event('input', { bubbles: true }));
      } catch (e) { say('warn', t('imageSaveFailed')); }
    }
  };
  const handle = (e, files, always) => {
    if (!files.length) return;
    if (!(D && D.upload)) {
      if (always) e.preventDefault();
      say('warn', t('imageFileMode'));
      return;
    }
    e.preventDefault();
    take(files);
  };
  ta.addEventListener('paste', (e) => handle(e, [...e.clipboardData.files].filter((f) => f.type.startsWith('image/')), false));
  ta.addEventListener('drop', (e) => handle(e, [...e.dataTransfer.files], true));
};
// A file dropped outside a text box would open over the page.
const hasFiles = (e) => [...(e.dataTransfer ? e.dataTransfer.types : [])].includes('Files');
document.addEventListener('dragover', (e) => { if (hasFiles(e)) e.preventDefault(); });
document.addEventListener('drop', (e) => {
  if (!hasFiles(e) || e.defaultPrevented) return;
  e.preventDefault();
  say('warn', t('imageDropHint'));
});
