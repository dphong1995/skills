import { el, HL_BOX, NOTE_BTN } from './dom.js';
import { short } from './strings.js';
import { t } from './i18n.js';

const DIFF_TINT = { del: 'line-through bg-del', add: 'bg-add' };

// A note (the `noteData` shape) as one line. `head` adds the "[§ section · lines] " prefix; `brief` shortens a passage's quote (an edit keeps its whole quote).
export const formatNote = (n, { head = false, brief = false } = {}) => {
  const at = head ? '[§ ' + n.key + (n.lines ? ' · ' + n.lines : '') + '] ' : '';
  if (n.kind === 'passage') return at + '«' + (brief ? short(n.quote) : n.quote) + '» → ' + n.text;
  if (n.kind === 'edit') return at + '«' + n.quote + '» ⇒ «' + n.to + '»';
  return at + n.text;
};

// A filled note as the server saves it for the next round.
export const noteData = (n, general) => {
  const text = n.ta.value.trim();
  if (!n.hl) return n === general ? { kind: 'general', key: 'General', text } : { kind: 'section', key: n.key, text };
  const at = { key: n.hl.key, lines: n.hl.lines, quote: n.hl.quote };
  return n.hl.kind === 'edit' ? { kind: 'edit', ...at, to: text, text: '' } : { kind: 'passage', ...at, text };
};

// Round 2 on: what the page shows of the previous round (`ch` is the document's changes). All text is the user's or the document's: textContent only.
export const previousRound = (ch) => {
  const notes = ch.notes || [];
  const shown = new Set();
  const notesBox = (list) => el('div', { class: HL_BOX, 'data-prev': '' }, el('span', { class: 'text-[12px] font-semibold text-muted', text: t('prevNotes') }),
    list.map((n) => el('p', { class: 'm-0 text-[13px] whitespace-pre-line wrap-anywhere', text: formatNote(n) })));
  const diffBox = (parts) => el('div', { class: HL_BOX + ' font-mono text-[12.5px] leading-normal whitespace-pre-wrap wrap-anywhere', 'data-diff': '', hidden: true },
    parts.map((p) => el('span', { class: DIFF_TINT[p.t], 'data-t': p.t, text: p.s })));
  return {
    // The toggle for the heading row and the boxes that follow its note box.
    under: (key) => {
      const old = notes.filter((n) => n.kind !== 'general' && n.key === key);
      if (old.length) shown.add(key);
      const diff = ch.diffs && ch.diffs[key] ? diffBox(ch.diffs[key]) : null;
      const toggle = diff && el('button', { type: 'button', class: NOTE_BTN, onclick: (e) => {
        diff.hidden = !diff.hidden;
        e.currentTarget.textContent = t(diff.hidden ? 'showChanges' : 'hideChanges');
      } }, t('showChanges'));
      return { toggle, boxes: [old.length ? notesBox(old) : null, diff] };
    },
    // General notes and those of a heading the page no longer has (after every heading was drawn).
    orphans: () => {
      const lost = notes.filter((n) => n.kind === 'general' || !shown.has(n.key));
      return lost.length ? el('div', { class: 'mt-1 grid gap-0.5', 'data-prev': '' }, el('strong', { text: t('prevOrphans') }),
        lost.map((n) => el('p', { class: 'm-0 whitespace-pre-line wrap-anywhere', text: formatNote(n, { head: true }) }))) : null;
    },
  };
};
