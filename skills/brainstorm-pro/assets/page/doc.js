import { $, after, btnCls, CALLOUT, el, KEY_HINT, KEY_HINT_SHIFT, kbd, LIST, NOTE_BTN, RING, TEXTAREA } from './dom.js';
import { esc, plain } from './strings.js';
import { extLang } from './code.js';
import { openFile } from './viewer.js';
import { codeSpan } from './text.js';
import { codeBlock, loadScript, mermaidBlock } from './blocks.js';
import { CARD, copyText, fabMsg, fit, main, out, postText, say, sendMode, storage } from './fab.js';
import { imageInput, images, imageStrip, imgLines } from './images.js';
import { passages } from './passages.js';
import { formatNote, noteData, previousRound } from './rounds.js';
import { sendChooser } from './chooser.js';
import { shortcuts } from './shortcuts.js';
import { docKeys } from './doc-keys.js';
import { closeTab, guardLeave } from './leave.js';

// Document mode: a Markdown file (spec, design, plan, report) drawn with marked, one card per ## section. In review mode every ## and ### heading and every selected passage takes a note, and the bar approves or sends the notes.
export const docPage = (doc) => {
  const review = !!doc.review;
  const ch = doc.changes || {};
  const store = storage('doc-draft:' + location.pathname);
  const notes = [];
  const ids = new Map();
  const rowNote = new Map();
  let general = null;
  const nav = $('#qnav');
  let md = null;
  let ps = null;
  // What the key list needs on the review page; null on the read-only page.
  let reviewKeys = null;

  document.title = (review ? 'Duyệt · ' : '') + doc.title;
  document.documentElement.classList.replace('scroll-pt-14', 'scroll-pt-24');
  $('#topic').textContent = doc.title;
  if (review) $('#topic').after(el('span', { class: 'badge badge-sm badge-warning badge-soft whitespace-nowrap', text: 'Chờ duyệt · lần ' + (ch.round || 1) }));
  nav.setAttribute('aria-label', 'Các mục');
  // The nav and the shortcuts button (added by shortcuts) share one row below the title and the badge, so the button never takes a row of its own.
  nav.className = 'flex-1 min-w-0 flex gap-1 overflow-x-auto pb-0.5';
  const navRow = el('div', { class: 'w-full flex items-center gap-1' });
  nav.replaceWith(navRow);
  navRow.append(nav);

  // Notes in document order, so comments on passages and section notes read top to bottom.
  const filled = () => notes.filter((n) => !n.empty())
    .sort((a, b) => (after(a.box, b.box) ? -1 : 1));
  const count = el('span', { class: 'font-mono text-[12px] text-muted whitespace-nowrap' });
  const update = () => {
    const n = filled();
    const total = n.length + ' góp ý';
    if (count.textContent !== total) count.textContent = total;
    for (const x of notes) if (x.btn) {
      const label = x.ta.value.trim() ? 'Góp ý ✓' : 'Góp ý';
      if (x.btn.textContent !== label) x.btn.textContent = label;
    }
    store.set(n.length ? { n: Object.fromEntries(n.filter((x) => !x.hl).map((x) => [x.key, x.ta.value])),
      h: n.filter((x) => x.hl).map((x) => ({ s: x.hl.s, e: x.hl.e, q: x.hl.text, t: x.ta.value, k: x.hl.kind })), img: Object.fromEntries(images) } : null);
    say('');
  };
  const noteBox = (key, placeholder, noImages) => {
    const ta = el('textarea', { rows: '2', class: TEXTAREA, placeholder, 'aria-label': 'Góp ý: ' + key });
    ta.addEventListener('input', () => { fit(ta); update(); });
    const strip = imageStrip([ta]);
    if (!noImages) imageInput(ta, strip);
    const n = { key, ta, strip, box: el('div', { class: 'grid gap-1.5', 'data-note': '' }, ta, strip), btn: null, empty: () => !ta.value.trim() };
    notes.push(n);
    return n;
  };
  const noteFor = (key) => {
    const n = noteBox(key, 'Góp ý cho mục này…');
    n.box.hidden = true;
    n.btn = el('button', { type: 'button', class: NOTE_BTN, onclick: () => {
      if (n.box.hidden) { n.box.hidden = false; n.ta.focus(); }
      else if (!n.ta.value.trim()) n.box.hidden = true;
      else n.ta.focus();
    } }, 'Góp ý');
    return n;
  };
  const changeBadge = (key) => ((ch.changed || []).includes(key) ? el('span', { class: 'badge badge-xs badge-warning badge-soft', text: 'Đã sửa' })
    : (ch.added || []).includes(key) ? el('span', { class: 'badge badge-xs badge-success badge-soft', text: 'Mới' }) : null);

  // marked's HTML gets the page's classes; file paths and links to embedded files open in the viewer, other relative links are dropped.
  const decorate = (root) => {
    for (const a of root.querySelectorAll('a')) {
      const href = a.getAttribute('href') || '';
      const abs = D.links && D.links[href];
      a.className = 'link link-primary';
      if (abs) a.addEventListener('click', (e) => { e.preventDefault(); openFile(abs, extLang(abs)); });
      else if (/^(https?:|mailto:)/i.test(href)) { a.target = '_blank'; a.rel = 'noopener'; }
      else if (!href.startsWith('#')) a.removeAttribute('href');
    }
    for (const c of root.querySelectorAll('code')) if (!c.closest('pre')) c.replaceWith(codeSpan(c.textContent));
    for (const p of root.querySelectorAll('pre')) {
      const c = p.querySelector('code') || p;
      const lang = (/language-([\w-]+)/.exec(c.className) || [])[1];
      p.replaceWith(lang === 'mermaid' ? mermaidBlock({ text: c.textContent }) : codeBlock({ text: c.textContent.replace(/\n$/, ''), lang }));
    }
    for (const l of root.querySelectorAll('ul, ol')) {
      l.className = LIST + (l.tagName === 'OL' ? ' list-decimal' : l.querySelector(':scope > li > input[type=checkbox]') ? ' list-none pl-0' : ' list-disc');
    }
    for (const i of root.querySelectorAll('li > input[type=checkbox]')) i.className = 'checkbox checkbox-xs align-[-2px] me-1.5 disabled:opacity-100 disabled:cursor-default';
    for (const b of root.querySelectorAll('blockquote')) b.className = 'border-l-[3px] border-base-300 pl-3 text-muted';
    for (const h of root.querySelectorAll('hr')) h.className = 'border-base-300';
    for (const t of root.querySelectorAll('table')) {
      t.className = 'table table-sm text-[13px] [&_thead]:whitespace-normal [&_th]:align-top [&_td]:align-top';
      for (const r of t.querySelectorAll('thead tr')) r.className = 'bg-base-200';
      for (const c of t.querySelectorAll('th')) c.classList.add('text-base-content');
      const wrap = el('div', { class: 'overflow-x-auto border border-base-300 rounded-box' });
      t.replaceWith(wrap);
      wrap.append(t);
    }
  };

  const prev = previousRound(ch);
  // A ## or ### heading, keyed as "Section" or "Section › Subsection" (the render script compares rounds with the same keys).
  let h2key = '';
  let secKey = 'Mở đầu';
  let secs = 0;
  const heading = (t) => {
    const text = plain(t.text);
    const key = t.depth === 2 ? (h2key = text) : t.depth === 3 && h2key ? h2key + ' › ' + text : text;
    if (t.depth <= 3) secKey = key;
    const id = 'sec-' + ++secs;
    ids.set(key, id);
    const h = el(t.depth === 2 ? 'h2' : t.depth === 3 ? 'h3' : 'h4', { id,
      class: t.depth === 2 ? 'text-[18px] leading-[1.35] font-bold' : t.depth === 3 ? 'text-[15.5px] leading-[1.35] font-semibold' : 'text-[14px] font-semibold' });
    h.innerHTML = md.parseInline(t.text);
    decorate(h);
    if (t.depth > 3) return [h];
    const n = review ? noteFor(key) : null;
    const under = review ? prev.under(key) : { toggle: null, boxes: [] };
    const row = el('div', { class: 'flex flex-wrap items-center gap-x-2 gap-y-1 ' + RING }, h, changeBadge(key), under.toggle, n && n.btn);
    if (n) rowNote.set(row, n);
    return [row, n && n.box, ...under.boxes].filter(Boolean);
  };

  // Each block carries its section key (data-k) and its lines in the .md (data-l, "from-to"), found from marked's token text; bullets and table rows get their own lines.
  const src = doc.md.replace(/\r\n?/g, '\n');
  let lineAt = 0, line = 1;
  const lineOf = (i) => { for (; lineAt < i; lineAt++) if (src[lineAt] === '\n') line++; return line; };
  const span = (raw, i) => { const a = lineOf(i); return a + '-' + (a + raw.replace(/\n+$/, '').split('\n').length - 1); };
  const tag = (node, lines) => { node.dataset.k = secKey; if (lines) node.dataset.l = lines; return node; };

  const render = (marked) => {
    md = new marked.Marked({ gfm: true, renderer: { html: ({ text }) => esc(text) } });
    let card = null;
    let titled = false;
    let pos = 0;
    for (const t of md.lexer(src)) {
      const i = src.indexOf(t.raw, pos);
      if (i >= 0) pos = i + t.raw.length;
      if (t.type === 'space') continue;
      const lines = i < 0 ? null : span(t.raw, i);
      if (t.type === 'heading' && t.depth === 1 && !titled) { titled = true; continue; }
      if (!card || (t.type === 'heading' && t.depth === 2)) {
        card = el('article', { class: CARD });
        main.append(card);
        if (t.type === 'heading' && t.depth === 2) {
          const [row, ...rest] = heading(t);
          card.append(tag(row, lines), ...rest);
          nav.append(el('a', { href: '#' + ids.get(h2key), class: 'btn btn-xs btn-ghost font-normal whitespace-nowrap', text: h2key }));
          continue;
        }
      }
      if (t.type === 'heading') {
        const [row, ...rest] = heading(t);
        card.append(tag(row, lines), ...rest);
      }
      else if (t.type === 'code') {
        const lang = (t.lang || '').split(/\s/)[0] || undefined;
        const fig = lang === 'mermaid' ? mermaidBlock({ text: t.text }) : codeBlock({ text: t.text, lang });
        if (lang === 'mermaid') fig.dataset.mmd = '';
        card.append(tag(fig, lines));
      }
      else {
        const box = tag(el('div', { class: 'grid gap-1.5 min-w-0' }), lines);
        box.innerHTML = md.parser([t]);
        if (t.type === 'list' && i >= 0) {
          const items = box.querySelectorAll(':scope > ul > li, :scope > ol > li');
          let p = i;
          t.items.forEach((it, k) => {
            const j = src.indexOf(it.raw, p);
            if (j < 0 || !items[k]) return;
            p = j + it.raw.length;
            tag(items[k], span(it.raw, j));
          });
        }
        if (t.type === 'table' && lines) {
          const a = +lines.split('-')[0];
          box.querySelectorAll('thead tr').forEach((r) => tag(r, a + '-' + a));
          box.querySelectorAll('tbody tr').forEach((r, k) => tag(r, (a + 2 + k) + '-' + (a + 2 + k)));
        }
        decorate(box);
        card.append(box);
      }
    }
  };

  const finish = () => {
    // Without sections the nav row is empty: the button joins the title row, so the header keeps one row.
    if (!nav.children.length) { navRow.hidden = true; $('header .navbar').append(helpBtn); }
    if (review) main.prepend(el('p', { class: 'text-[13px] text-muted', text: 'Tô đen một đoạn rồi bấm Góp ý để góp ý ngay chỗ đó; ' + KEY_HINT + ' gửi tất cả · ? phím tắt.' }));
    if (ch.round > 1) {
      const link = (k) => (ids.has(k) ? el('a', { href: '#' + ids.get(k), class: 'link link-primary', text: k }) : el('span', { text: k }));
      const list = (label, keys) => (keys && keys.length ? el('p', {}, el('strong', { text: label + ' (' + keys.length + '): ' }), keys.flatMap((k, i) => [i ? ', ' : null, link(k)])) : null);
      const any = ['changed', 'added', 'removed'].some((k) => (ch[k] || []).length);
      main.prepend(el('div', { class: 'alert alert-soft alert-info block text-base-content text-[14px] leading-normal px-3 py-1.5 [&_strong]:text-[var(--color-info)]' },
        el('p', {}, el('strong', { text: 'Lần ' + ch.round + ': ' }), any ? 'các mục thay đổi so với bản bạn xem lần trước.' : 'không mục nào thay đổi so với lần trước.'),
        list('Đã sửa', ch.changed), list('Mới', ch.added), list('Đã bỏ', ch.removed),
        review ? prev.orphans() : null));
    }
    if (!review) return;
    general = noteBox('Chung', 'Góp ý chung cho cả tài liệu…');
    main.append(el('article', { class: CARD, 'data-note': '' }, el('h2', { class: 'text-[18px] leading-[1.35] font-bold', text: 'Góp ý chung' }), general.box));
    const draft = store.get();
    let lost = 0;
    if (draft && draft.n) {
      for (const [k, v] of Object.entries(draft.img || {})) images.set(+k, v);
      for (const n of notes) if (draft.n[n.key]) { n.ta.value = draft.n[n.key]; n.box.hidden = false; n.strip.refresh(); fit(n.ta); }
      const all = ps.allText();
      for (const h of draft.h || []) {
        const r = all.slice(h.s, h.e) === h.q ? ps.rangeAt(h.s, h.e) : null;
        if (!r || !ps.addComment(r, h.t, h.k)) lost++;
      }
    }
    update();
    if (lost) say('warn', 'Không khôi phục được ' + lost + ' góp ý tô đen (tài liệu đã đổi).');
    else if (draft) say('', 'Đã khôi phục góp ý đã lưu.');
  };

  // The ## and ### heading rows, which the shortcuts walk, and the row a note box sits under (none before the first heading).
  const rows = (sel) => () => [...main.querySelectorAll(sel)].map((h) => h.parentElement);
  const targets = rows('h2[id], h3[id]');
  const rowBefore = (box) => targets().filter((r) => after(r, box)).pop() || null;

  if (review) {
    guardLeave();
    const approveBtn = el('button', { type: 'button', class: btnCls() + ' btn-success', onclick: () => submit('Duyệt') }, sendMode ? 'Duyệt' : 'Copy: Duyệt', kbd(KEY_HINT_SHIFT, 'kbd-xs max-sm:hidden'));
    const changeBtn = el('button', { type: 'button', class: btnCls('primary'), onclick: () => submit('Sửa') }, sendMode ? 'Gửi góp ý' : 'Copy góp ý', kbd(KEY_HINT, 'kbd-xs max-sm:hidden'));
    const send = async (decision) => {
      if (approveBtn.disabled) return;
      const list = filled();
      const data = list.map((n) => noteData(n, general));
      const t = ['Quyết định: ' + decision, ...data.map((d) => formatNote(d, { head: true, brief: true })),
        ...imgLines(list.map((n) => n.ta.value).join('\n'))].join('\n');
      if (sendMode) {
        approveBtn.disabled = changeBtn.disabled = true;
        try {
          await postText(t, data);
          store.set(null);
          say('ok', (decision === 'Duyệt' ? 'Đã gửi: Duyệt.' : 'Đã gửi ' + data.length + ' góp ý cho Claude.') + ' Có thể đóng tab này.');
          closeTab();
          return;
        } catch (e) { approveBtn.disabled = changeBtn.disabled = false; }
      }
      const copied = await copyText(t);
      say(copied && !sendMode ? 'ok' : 'warn', (sendMode ? 'Không gửi được (Claude không còn chờ trang này). ' : '')
        + (copied ? 'Đã copy: dán vào terminal của Claude.' : 'Không copy tự động được: chọn đoạn bên dưới, copy rồi dán vào terminal của Claude.'));
    };
    const submit = sendChooser({ count: () => filled().length, send, busy: () => approveBtn.disabled });
    document.body.append(el('aside', { class: 'toast toast-end z-10 items-end', 'aria-label': 'Duyệt tài liệu' }, fabMsg, out,
      el('div', { class: 'flex items-center gap-1.5 bg-base-100 border border-base-300 rounded-box py-1 pr-1 pl-3 shadow-lg' }, count, approveBtn, changeBtn)));
    ps = passages({ notes, noteBox, update });
    reviewKeys = { filled, rowNote, rowBefore, commentSelection: ps.commentSelection, general: () => general, submit };
  }
  // A note box belongs to the heading row before it, except the general note, which is no heading's.
  const { helpBtn } = shortcuts({ targets, level1: rows('h2[id]'), helpSlot: navRow, keys: (sc) => docKeys(sc, reviewKeys),
    owner: (n) => { const box = n.closest('[data-note]'); return targets().find((t) => t.contains(n)) || (box && !box.closest('article[data-note]') ? rowBefore(box) : null); } });

  loadScript('https://cdnjs.cloudflare.com/ajax/libs/marked/15.0.12/marked.min.js').then(() => render(window.marked)).catch(() => {
    main.append(el('article', { class: CARD },
      el('p', { class: CALLOUT, text: 'Không dựng được trang từ Markdown (mất mạng?); bên dưới là nội dung gốc.' }),
      el('pre', { class: 'm-0 whitespace-pre-wrap wrap-anywhere font-mono text-[12.5px]', text: doc.md })));
  }).then(finish);
};
