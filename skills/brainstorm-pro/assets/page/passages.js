import { btnCls, el, HL_BOX, HL_QUOTE, HL_QUOTE_ROW, MARK, NOTE_BTN } from './dom.js';
import { short } from './strings.js';
import { fit, main } from './fab.js';

// `notes`, `noteBox` and `update` are the review page's note list, box maker and refresh. Installs the Góp ý button that appears by a selection.
export const passages = ({ notes, noteBox, update }) => {
  // Comments on a selected passage: the passage gets a mark and a note opens under it. Offsets count the text a passage can be selected in, so a draft marks the same passage again after a reload.
  const SKIP = 'textarea, input, button, svg, [data-note], [data-mmd]';
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
  const quoteOf = (parts) => parts.map((p, k) => (k && p.parentElement.closest(FLOW) !== parts[k - 1].parentElement.closest(FLOW) ? ' ' : '') + p.data).join('');
  let hls = 0;
  const addComment = (r, text) => {
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
    const n = noteBox('hl:' + id, 'Góp ý cho đoạn này…');
    n.hl = { id, s, e, text: allText().slice(s, e), quote: quoteOf(parts), key:first.closest('[data-k]') ? first.closest('[data-k]').dataset.k : 'Mở đầu',
      lines: a && b ? 'dòng ' + (a[0] === b[1] ? a[0] : a[0] + '-' + b[1]) : '' };
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
        el('span', { class: HL_QUOTE, text: '«' + short(n.hl.quote) + '»' }),
        el('button', { type: 'button', class: NOTE_BTN, onclick: remove }, 'Xóa')),
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
      setTimeout(() => { if (document.hasFocus() && document.activeElement !== n.ta && !n.ta.value.trim()) remove(); }, 0);
    });
    n.ta.addEventListener('keydown', (ev) => { if (ev.key === 'Escape' && !n.ta.value.trim()) { ev.preventDefault(); remove(); } });
    if (text) { n.ta.value = text; n.strip.refresh(); fit(n.ta); }
    return n;
  };

  // Comments on the current selection: the new note, or null when nothing in it can take one.
  const commentSelection = () => {
    const sel = getSelection();
    const n = sel.rangeCount ? addComment(sel.getRangeAt(0)) : null;
    adder.hidden = true;
    if (n) sel.removeAllRanges();
    return n;
  };
  // The Góp ý button by a selection; mousedown keeps the selection.
  const adder = el('button', { type: 'button', class: btnCls('primary', true) + ' absolute z-20 shadow-lg', hidden: true, onmousedown: (e) => e.preventDefault(), onclick: () => {
    const n = commentSelection();
    if (n) n.ta.focus();
  } }, 'Góp ý');
  document.body.append(adder);
  let placing = 0;
  const place = () => {
    const sel = getSelection();
    const r = sel.rangeCount && !sel.isCollapsed ? sel.getRangeAt(0) : null;
    adder.hidden = !r || !selectedTexts(r).length;
    if (adder.hidden) return;
    const rects = r.getClientRects();
    const b = rects.length ? rects[rects.length - 1] : r.getBoundingClientRect();
    adder.style.left = scrollX + Math.max(8, Math.min(b.right + 4, document.documentElement.clientWidth - adder.offsetWidth - 8)) + 'px';
    adder.style.top = scrollY + b.bottom + 4 + 'px';
  };
  const later = (ms) => { clearTimeout(placing); placing = setTimeout(place, ms); };
  // Timing as in Hypothesis: nothing while the mouse is down, 10 ms after mouseup, 100 ms after other selection changes (a change right after mouseup keeps 10 ms).
  let down = false, upAt = -Infinity;
  document.addEventListener('mousedown', (e) => {
    if (e.button !== 0 || adder.contains(e.target)) return;
    down = true;
    clearTimeout(placing);
    adder.hidden = true;
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
