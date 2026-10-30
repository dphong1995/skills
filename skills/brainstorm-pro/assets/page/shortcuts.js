import { $, closeForm, el, kbd, modal } from './dom.js';
import { isTyping, keyName, step } from './keys.js';

const GROUPS = ['Điều hướng', 'Thao tác', 'Chung'];

// The one keyboard engine of every mode. `targets()` are the items J/K walk, in page order; `level1()` those the digits pick; `owner(node)` the target a node belongs to.
// `keys(sc)` returns the mode's list of { k, show?, group, label, run, typing? }: k is a keyName() result (or a list of them), typing true lets it run inside a text box.
// The ring is the data-current attribute on the current target, drawn from the first shortcut on; the `?` dialog is built from the same list.
export const shortcuts = ({ targets, level1 = targets, owner, keys, helpSlot }) => {
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
  // The y just under the header, measured again only when the header or the page is resized. The same resize sets the scroll padding to the header's height plus a small gap, so a link, J or a digit never lands a heading under the header at any width (the static scroll-pt class is the first value).
  let under = null;
  const measure = new ResizeObserver(() => {
    under = null;
    document.documentElement.style.scrollPaddingTop = Math.ceil($('header').getBoundingClientRect().height) + 12 + 'px';
  });
  measure.observe($('header'));
  measure.observe(document.documentElement);
  const topY = () => {
    if (under == null) {
      const pad = parseFloat(getComputedStyle(document.documentElement).scrollPaddingTop) || 0;
      under = Math.max($('header').getBoundingClientRect().bottom, pad) + 8;
    }
    return under;
  };
  // The last target whose top is above that point.
  const topTarget = () => {
    const all = targets();
    const y = topY();
    let at = all[0] || null;
    for (const t of all) if (t.getBoundingClientRect().top <= y) at = t; else break;
    return at;
  };
  // Scrolling the page started itself must not move the ring: tracking waits until the scrolling has been quiet for 150 ms (1 s when nothing scrolls).
  const unlock = () => { locked = false; clearTimeout(lockTimer); };
  // hold() is for a scroll the page causes by itself, such as focusing a box.
  const hold = () => {
    locked = true;
    clearTimeout(lockTimer);
    lockTimer = setTimeout(unlock, 1000);
  };
  const go = (t, { to = t, block = 'start' } = {}) => {
    if (!t && !to) return;
    if (t) setCurrent(t);
    hold();
    to.scrollIntoView({ behavior: matchMedia('(prefers-reduced-motion: reduce)').matches ? 'auto' : 'smooth', block });
  };
  const sc = {
    current: () => current,
    clear: () => setCurrent(null),
    go,
    hold,
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
  // The user's own scroll, even inside the lock window: a vertical wheel turn (not a pinch zoom), a touch drag or a scrolling key outside a text box.
  addEventListener('wheel', (e) => { if (e.deltaY !== 0 && !e.ctrlKey) unlock(); }, { passive: true });
  addEventListener('touchmove', unlock, { passive: true });
  const SCROLL_KEYS = ['PageUp', 'PageDown', ' ', 'ArrowUp', 'ArrowDown', 'Home', 'End'];
  // Space and the up/down arrows act on a focused input, button, select or summary instead of scrolling; a key a handler already took does not scroll either.
  addEventListener('keydown', (e) => {
    const a = document.activeElement;
    if (e.defaultPrevented || !SCROLL_KEYS.includes(e.key) || isTyping(a)) return;
    if ([' ', 'ArrowUp', 'ArrowDown'].includes(e.key) && a.closest('input, button, select, summary')) return;
    unlock();
  });
  addEventListener('scroll', () => {
    if (locked) { clearTimeout(lockTimer); lockTimer = setTimeout(unlock, 150); return; }
    if (!started || frame) return;
    frame = requestAnimationFrame(() => { frame = 0; if (!locked) setCurrent(topTarget()); });
  }, { passive: true });

  const toggleHelp = () => { if (help.open) help.close(); else help.showModal(); };
  const list = [...keys(sc),
    { k: '?', group: 'Chung', label: 'Mở hoặc đóng bảng phím', run: toggleHelp },
    { k: 'Escape', show: 'Esc', group: 'Chung', label: 'Rời ô đang gõ, hoặc bỏ chọn chữ', typing: true,
      run: () => { if (isTyping(document.activeElement)) document.activeElement.blur(); else getSelection().removeAllRanges(); } }];
  const byKey = new Map(list.filter((x) => x.k != null).flatMap((x) => [].concat(x.k).map((k) => [k, x])));

  // One outer grid, so the key column is as wide as the widest key in every group (each group's section and list are subgrids).
  const help = modal('max-w-xl grid gap-3',
    el('h2', { class: 'text-[16px] font-semibold', text: 'Phím tắt' }),
    el('div', { class: 'grid grid-cols-[auto_1fr] gap-x-3' }, GROUPS.flatMap((g) => {
      const rows = list.filter((x) => x.group === g);
      return rows.length ? [el('section', { class: 'col-span-2 grid grid-cols-subgrid gap-y-1.5 [&:not(:first-child)]:mt-2' },
        el('h3', { class: 'col-span-2 font-mono text-[11px] font-medium tracking-[.06em] uppercase text-muted', text: g }),
        el('dl', { class: 'm-0 col-span-2 grid grid-cols-subgrid items-center gap-y-1.5' },
          ...rows.flatMap((x) => [el('dt', {}, kbd(x.show || [].concat(x.k)[0].toUpperCase())), el('dd', { class: 'm-0', text: x.label })])))] : [];
    })),
    closeForm('flex justify-end'));
  document.body.append(help);
  // The page says where the button goes (`helpSlot`, an element it is appended to) and may move it later.
  const helpBtn = el('button', { type: 'button', class: 'btn btn-xs btn-ghost -my-0.5', title: 'Phím tắt', 'aria-label': 'Phím tắt', onclick: toggleHelp }, kbd('?', 'kbd-xs'));
  helpSlot.append(helpBtn);

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
  return { helpBtn };
};
