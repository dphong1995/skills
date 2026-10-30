import { btnCls, closeForm, EDIT_TAG, el, HL_BOX, HL_QUOTE, HL_QUOTE_ROW, MARK, modal, NOTE_BTN, THUMB, THUMB_DEL, THUMB_IMG, THUMB_NUM } from './dom.js';
import { hlLines, LH, linesView, PAD } from './code.js';

// Elements built when the user acts take their classes from here, so Tailwind compiles them at load (@source inline alone still rebuilds on first use).
document.body.append(el('div', { hidden: true, class: [MARK, EDIT_TAG, HL_BOX, HL_QUOTE_ROW, HL_QUOTE, NOTE_BTN, THUMB, THUMB_IMG, THUMB_NUM, THUMB_DEL].join(' ') }));

// One modal shows a whole file, an image or a mockup at full size; Esc, the backdrop or Đóng closes it.
const viewerTitle = el('span', { class: 'flex-1 min-w-0 font-mono text-[12px] wrap-anywhere' });
const viewerBody = el('div', { class: 'grid min-h-0' });
const viewer = modal('p-0 w-[min(1400px,calc(100vw-32px))] max-w-none h-[calc(100vh-48px)] max-h-none bg-base-200 border border-base-300 grid grid-rows-[auto_minmax(0,1fr)] overflow-hidden',
  el('div', { class: 'flex items-center gap-3 py-1.5 pr-1.5 pl-3.5 bg-base-100 border-b border-base-300' }, viewerTitle, closeForm()),
  viewerBody);
document.body.append(viewer);
export const showInViewer = (title, node) => {
  viewerTitle.textContent = title;
  viewerBody.replaceChildren(node);
  viewer.showModal();
};
export const openFile = (file, lang, mark) => {
  const text = D.files[file].replace(/\n$/, '');
  const lines = hlLines(text, lang);
  const view = linesView(lines, lines.map((_, i) => i + 1), mark ? lines.map((_, i) => (i + 1 >= mark[0] && i + 1 <= mark[1] ? 'mark' : null)) : null);
  view.classList.add('overflow-auto', 'min-h-0');
  showInViewer(file + ' · ' + lines.length + ' dòng', view);
  view.scrollTop = mark ? Math.max(0, PAD + (mark[0] - 5) * LH) : 0;
};
export const fileButton = (s, lang) => (s.file && D.files && D.files[s.file] != null
  ? el('button', { type: 'button', class: btnCls('file', true), onclick: () => openFile(s.file, lang, s.lines) }, 'Xem toàn file') : null);
export const zoomButton = (onclick) => el('button', { type: 'button', class: btnCls('file', true), onclick }, 'Phóng to');
