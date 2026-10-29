import { $, btnCls, el } from './dom.js';
import { isTyping, keyName, step } from './keys.js';

const GROUPS = ['Điều hướng', 'Thao tác', 'Chung'];
const kbd = (text, size = 'kbd-sm') => el('kbd', { class: 'kbd ' + size, text });

// The one keyboard engine of every mode. `targets()` are the items J/K walk, in page order; `level1()` those the digits pick; `owner(node)` the target a node belongs to.
// `keys(sc)` returns the mode's list of { k, show?, group, label, run, typing? }: k is a keyName() result (or a list of them), typing true lets it run inside a text box.
// The ring is the data-current attribute on the current target, drawn from the first shortcut on; the `?` dialog is built from the same list.
export const shortcuts = ({ targets, level1 = targets, owner, keys }) => {
  let started = false;
  let current = null;
  let locked = false;
  let lockTimer = 0;
  let frame = 0;
  const setCurrent = (t) => {
    if (t === current) return;
    if (current) delete current.dataset.current;
    current = t;
    if (t) t.dataset.current = 'true';
  };
  // The last target whose top is above the point just under the header.
  const topTarget = () => {
    const all = targets();
    const pad = parseFloat(getComputedStyle(document.documentElement).scrollPaddingTop) || 0;
    const y = Math.max($('header').getBoundingClientRect().bottom, pad) + 8;
    let at = all[0] || null;
    for (const t of all) if (t.getBoundingClientRect().top <= y) at = t; else break;
    return at;
  };
  // Scrolling the page started itself must not move the ring: tracking waits until the scrolling has been quiet for 150 ms (1 s when nothing scrolls).
  const unlock = () => { locked = false; clearTimeout(lockTimer); };
  const go = (t, { to = t, block = 'start' } = {}) => {
    if (!t) return;
    setCurrent(t);
    locked = true;
    clearTimeout(lockTimer);
    lockTimer = setTimeout(unlock, 1000);
    to.scrollIntoView({ behavior: matchMedia('(prefers-reduced-motion: reduce)').matches ? 'auto' : 'smooth', block });
  };
  const sc = {
    current: () => current,
    targets,
    go,
    move: (d) => {
      const all = targets();
      go(all[step(all.indexOf(current), all.length, d)]);
    },
    nav: (next, prev, nth) => [
      { k: 'j', group: 'Điều hướng', label: next, run: () => sc.move(1) },
      { k: 'k', group: 'Điều hướng', label: prev, run: () => sc.move(-1) },
      { k: ['1', '2', '3', '4', '5', '6', '7', '8', '9'], show: '1–9', group: 'Điều hướng', label: nth, run: (k) => go(level1()[+k - 1]) },
    ],
  };

  const own = owner || ((n) => targets().find((t) => t.contains(n)) || null);
  const track = (e) => { if (started && e.target instanceof Element) { const t = own(e.target); if (t) setCurrent(t); } };
  document.addEventListener('focusin', track);
  document.addEventListener('click', track);
  addEventListener('scroll', () => {
    if (locked) { clearTimeout(lockTimer); lockTimer = setTimeout(unlock, 150); return; }
    if (!started || frame) return;
    frame = requestAnimationFrame(() => { frame = 0; if (!locked) setCurrent(topTarget()); });
  }, { passive: true });

  const help = el('dialog', { class: 'modal' });
  const toggleHelp = () => { if (help.open) help.close(); else if (!$('dialog[open]')) help.showModal(); };
  const list = [...keys(sc),
    { k: '?', group: 'Chung', label: 'Mở hoặc đóng bảng phím', run: toggleHelp },
    { k: 'Escape', show: 'Esc', group: 'Chung', label: 'Rời ô đang gõ, hoặc bỏ chọn chữ', typing: true,
      run: () => { if (isTyping(document.activeElement)) document.activeElement.blur(); else getSelection().removeAllRanges(); } }];
  const byKey = new Map(list.flatMap((x) => [].concat(x.k).map((k) => [k, x])));

  help.append(
    el('div', { class: 'modal-box max-w-xl grid gap-3' },
      el('h2', { class: 'text-[16px] font-semibold', text: 'Phím tắt' }),
      GROUPS.map((g) => (list.some((x) => x.group === g) ? el('section', { class: 'grid gap-1.5' },
        el('h3', { class: 'font-mono text-[11px] font-medium tracking-[.06em] uppercase text-muted', text: g }),
        el('dl', { class: 'm-0 grid grid-cols-[auto_1fr] items-center gap-x-3 gap-y-1.5' },
          list.filter((x) => x.group === g).flatMap((x) => [el('dt', {}, kbd(x.show || [].concat(x.k)[0].toUpperCase())), el('dd', { class: 'm-0', text: x.label })]))) : null)),
      el('form', { method: 'dialog', class: 'flex justify-end' }, el('button', { class: btnCls() }, 'Đóng', kbd('Esc', 'kbd-xs')))),
    el('form', { method: 'dialog', class: 'modal-backdrop' }, el('button', { text: 'Đóng' })));
  document.body.append(help);
  const button = el('button', { type: 'button', class: 'btn btn-xs btn-ghost', title: 'Phím tắt', 'aria-label': 'Phím tắt', onclick: toggleHelp }, kbd('?', 'kbd-xs'));
  // A document header holds the title row and a full-width nav, so the button joins the title row there.
  if (D.doc) ($('#topic + .badge') || $('#topic')).after(button);
  else $('.navbar').append(button);

  document.addEventListener('keydown', (e) => {
    if (e.defaultPrevented) return;
    const k = keyName(e);
    const x = k && byKey.get(k);
    if (!x) return;
    const open = $('dialog[open]');
    if (open && !(open === help && k === '?')) return;
    if (isTyping(document.activeElement) && !x.typing) return;
    e.preventDefault();
    if (!started && !x.typing && k !== '?') { started = true; setCurrent(topTarget()); }
    x.run(k);
  });
  return sc;
};
