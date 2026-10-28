import { PATH } from './strings.js';
import { CODE, el } from './dom.js';
import { extLang } from './code.js';
import { openFile } from './viewer.js';

// Inline `code` and **bold** only; everything else is plain text. A `path:N-M` the render script found on disk opens in the file viewer.
export const codeSpan = (t) => {
  const m = PATH.exec(t);
  const abs = D && D.paths && D.paths[m[1]];
  return abs
    ? el('a', { href: '#', class: CODE + ' link link-primary decoration-dotted underline-offset-2 hover:decoration-solid', title: 'Xem file',
      onclick: (e) => { e.preventDefault(); openFile(abs, extLang(abs), m[2] ? [+m[2], +(m[3] || m[2])] : null); } }, t)
    : el('code', { class: CODE, text: t });
};
export const inline = (text) => {
  const frag = document.createDocumentFragment();
  String(text ?? '').split(/(`[^`]+`|\*\*[^*]+\*\*)/).forEach((part) => {
    if (!part) return;
    if (part.length > 2 && part[0] === '`' && part.endsWith('`')) frag.append(codeSpan(part.slice(1, -1)));
    else if (part.length > 4 && part.startsWith('**') && part.endsWith('**')) frag.append(el('strong', { text: part.slice(2, -2) }));
    else frag.append(document.createTextNode(part));
  });
  return frag;
};
