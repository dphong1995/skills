import { btnCls, EDIT_TAG, el, HL_BOX, HL_QUOTE, HL_QUOTE_ROW, MARK, NOTE_BTN } from './dom.js';
import { short } from './strings.js';
import { fit, main, say } from './fab.js';
import { t } from './i18n.js';

// `notes`, `noteBox` and `update` are the review page's note list, box maker and refresh. Installs the Góp ý and Sửa thành buttons that appear by a selection.
export const passages = ({ notes, noteBox, update }) => {
  // Comments on a selected passage: the passage gets a mark and a note opens under it. Offsets count the text a passage can be selected in, so a draft marks the same passage again after a reload.
  const SKIP = 'textarea, input, button, svg, [data-note], [data-mmd], [data-prev], [data-diff]';
  const selectable = (t) => !t.parentElement.closest(SKIP) && !!t.parentElement.closest('#main > article');
  const walk = () => document.createTreeWalker(main, NodeFilter.SHOW_TEXT,
    { acceptNode: (t) => (t.parentElement.closest(SKIP) ? NodeFilter.FILTER_REJECT : NodeFilter.FILTER_ACCEPT) });
  const allText = () => { let s = ''; for (let w = walk(), t; (t = w.nextNode());) s += t.data; return s; };
  const offsetOf = (node, k) => { let o = 0; for (let w = walk(), t; (t = w.nextNode());) { if (t === node) return o + k; o += t.length; } return -1; };
  const rangeAt = (s, e) => {
    const r = document.createRange();
    let o = 0, started = false;
    for (let w = walk(), t; (t = w.nextNode());) {
      if (!started && s < o + t.length) { r.setStart(t, s - o); started = true; }
      if (started && e <= o + t.length) { r.setEnd(t, e - o); return r; }
      o += t.length;
    }
    return null;
  };
  // The selected, non-blank part of each text node in the range that a comment can mark.
  const selectedTexts = (r) => {
    const root = r.commonAncestorContainer;
    const all = [];
    if (root.nodeType === 3) all.push(root);
    else for (let w = document.createTreeWalker(root, NodeFilter.SHOW_TEXT), t; (t = w.nextNode());) if (r.intersectsNode(t)) all.push(t);
    return all.filter(selectable).map((t) => ({ t, s: t === r.startContainer ? r.startOffset : 0, e: t === r.endContainer ? r.endOffset : t.length }))
      .filter((x) => x.t.data.slice(x.s, x.e).trim());
  };
  // The passage as read: a space where it crosses from one block, cell or code line to the next.
  const FLOW = 'p, li, td, th, h1, h2, h3, h4, h5, h6, blockquote, span.block, div';
  // Between two code lines of one figure the break stays a line break.
  const sep = (p, q) => {
    const [a, b] = [p, q].map((x) => x.parentElement.closest(FLOW));
    if (a === b) return '';
    return a && b && a.matches('span.block') && b.matches('span.block') && a.closest('figure') === b.closest('figure') ? '\n' : ' ';
  };
  const quoteOf = (parts) => parts.map((p, k) => (k ? sep(parts[k - 1], p) : '') + p.data).join('');
  // One block: the first and the last selected text sit in the same [data-l] element (paragraph, list item, table row, code figure).
  const oneBlock = (all) => {
    if (!all.length) return false;
    const a = all[0].t.parentElement.closest('[data-l]');
    const z = all[all.length - 1].t.parentElement;
    const cell = (x) => x.closest('td, th');
    return !!a && a === z.closest('[data-l]') && cell(all[0].t.parentElement) === cell(z);
  };
  let hls = 0;
  // kind 'edit': the box opens with the passage itself, and counts only once its text differs from it.
  const addComment = (r, text, kind = 'passage') => {
    const id = String(++hls);
    const parts = selectedTexts(r).map(({ t, s, e }) => {
      if (e < t.length) t.splitText(e);
      const part = s ? t.splitText(s) : t;
      const m = el('mark', { 'data-c': id, class: MARK });
      part.replaceWith(m);
      m.append(part);
      return part;
    });
    if (!parts.length) return null;
    const first = parts[0].parentElement, last = parts[parts.length - 1].parentElement;
    const s = offsetOf(parts[0], 0), e = offsetOf(parts[parts.length - 1], parts[parts.length - 1].length);
    const [a, b] = [first.closest('[data-l]'), last.closest('[data-l]')].map((x) => x && x.dataset.l.split('-'));
    const n = noteBox('hl:' + id, t(kind === 'edit' ? 'editToPlaceholder' : 'passagePlaceholder'), kind === 'edit');
    n.hl = { id, kind, s, e, text: allText().slice(s, e), quote: quoteOf(parts), key:first.closest('[data-k]') ? first.closest('[data-k]').dataset.k : 'Intro',
      lines: a && b ? 'lines ' + (a[0] === b[1] ? a[0] : a[0] + '-' + b[1]) : '' };
    if (kind === 'edit') n.empty = () => !n.ta.value.trim() || n.ta.value.trim() === n.hl.quote.trim();
    const marks = () => main.querySelectorAll('mark[data-c="' + id + '"]');
    const remove = () => {
      if (!notes.includes(n)) return;
      for (const m of marks()) { const p = m.parentNode; m.replaceWith(...m.childNodes); p.normalize(); }
      n.hl.box.remove();
      notes.splice(notes.indexOf(n), 1);
      update();
    };
    n.hl.box = el('div', { class: HL_BOX, 'data-note': '', 'data-hl': id },
      el('div', { class: HL_QUOTE_ROW },
        kind === 'edit' ? el('span', { class: EDIT_TAG, text: t('editTo') }) : el('span', { class: HL_QUOTE, text: '«' + short(n.hl.quote) + '»' }),
        el('button', { type: 'button', class: NOTE_BTN, onclick: remove }, t('remove'))),
      n.box);
    // Under the bullet the passage ends in, else after its block (and after the comments already there).
    const li = last.closest('li');
    if (li) li.append(n.hl.box);
    else {
      let at = last.closest('#main > article > *');
      while (at.nextElementSibling && at.nextElementSibling.matches('[data-hl]')) at = at.nextElementSibling;
      at.after(n.hl.box);
    }
    // An empty comment goes away with its mark when it loses focus (not when the window does) or on Esc.
    n.ta.addEventListener('focus', () => { for (const m of marks()) m.dataset.active = 'true'; });
    n.ta.addEventListener('blur', () => {
      for (const m of marks()) delete m.dataset.active;
      setTimeout(() => { if (document.hasFocus() && document.activeElement !== n.ta && n.empty()) remove(); }, 0);
    });
    n.ta.addEventListener('keydown', (ev) => { if (ev.key === 'Escape' && n.empty()) { ev.preventDefault(); remove(); } });
    const start = text ?? (kind === 'edit' ? n.hl.quote : '');
    if (start) { n.ta.value = start; n.ta.setSelectionRange(start.length, start.length); n.strip.refresh(); fit(n.ta); }
    return n;
  };

  // Comments on the current selection: the new note, or null when nothing in it can take one.
  const commentSelection = (kind = 'passage') => {
    const sel = getSelection();
    const r = sel.rangeCount ? sel.getRangeAt(0) : null;
    const n = r && (kind !== 'edit' || oneBlock(selectedTexts(r))) ? addComment(r, undefined, kind) : null;
    if (n) {
      adders.hidden = true;
      sel.removeAllRanges();
    } else if (kind === 'edit' && r && !sel.isCollapsed) say('warn', t('editOneBlock'));
    return n;
  };
  // The Góp ý and Sửa thành buttons by a selection, in one box that is placed; mousedown keeps the selection.
  const adderBtn = (kind, label, primary) => el('button', { type: 'button', 'data-adder': kind, class: btnCls(primary, true) + ' shadow-lg', onmousedown: (e) => e.preventDefault(), onclick: () => {
    const n = commentSelection(kind);
    if (n) n.ta.focus();
  } }, label);
  const editAdder = adderBtn('edit', t('editTo'), '');
  const adders = el('div', { class: 'absolute z-20 flex gap-1 whitespace-nowrap', 'data-adders': '', hidden: true }, adderBtn('passage', t('note'), 'primary'), editAdder);
  document.body.append(adders);
  let placing = 0;
  const place = () => {
    const sel = getSelection();
    const r = sel.rangeCount && !sel.isCollapsed ? sel.getRangeAt(0) : null;
    const texts = r ? selectedTexts(r) : [];
    adders.hidden = !texts.length;
    editAdder.hidden = !oneBlock(texts);
    if (adders.hidden) return;
    const rects = r.getClientRects();
    const b = rects.length ? rects[rects.length - 1] : r.getBoundingClientRect();
    adders.style.left = scrollX + Math.max(8, Math.min(b.right + 4, document.documentElement.clientWidth - adders.offsetWidth - 8)) + 'px';
    adders.style.top = scrollY + b.bottom + 4 + 'px';
  };
  const later = (ms) => { clearTimeout(placing); placing = setTimeout(place, ms); };
  // Timing as in Hypothesis: nothing while the mouse is down, 10 ms after mouseup, 100 ms after other selection changes (a change right after mouseup keeps 10 ms).
  let down = false, upAt = -Infinity;
  document.addEventListener('mousedown', (e) => {
    if (e.button !== 0 || adders.contains(e.target)) return;
    down = true;
    clearTimeout(placing);
    adders.hidden = true;
  });
  document.addEventListener('mouseup', () => { down = false; upAt = performance.now(); later(10); });
  document.addEventListener('dragend', () => { down = false; later(10); });
  // Ctrl+click on macOS reports button 0 but opens the context menu, which swallows mouseup.
  document.addEventListener('contextmenu', () => { down = false; });
  document.addEventListener('selectionchange', () => { if (!down) later(performance.now() - upAt < 50 ? 10 : 100); });
  addEventListener('resize', () => later(100), { passive: true });
  main.addEventListener('click', (e) => {
    const m = e.target.closest('mark[data-c]');
    if (!m || String(getSelection()).trim()) return;
    const n = notes.find((x) => x.hl && x.hl.id === m.dataset.c);
    if (n) n.ta.focus();
  });

  return { addComment, allText, commentSelection, rangeAt };
};
