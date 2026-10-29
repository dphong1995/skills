import { after, KEY_HINT, KEY_HINT_SHIFT } from './dom.js';
import { sayFor, say } from './fab.js';

// The document page's key list: the ## and ### heading rows are the targets. `review` is null on the read-only page, which keeps navigation only.
export const docKeys = (sc, review) => {
  const nav = sc.nav('Tiêu đề kế', 'Tiêu đề trước', 'Mục ## thứ n');
  if (!review) return nav;
  const { filled, rowNote, rowBefore, commentSelection, general, deliver, sent } = review;
  // N goes on from the note it last showed, else from the current heading.
  let last = null;
  let armed = -Infinity;
  const approve = () => {
    if (sent()) return;
    const n = filled().length;
    if (n && performance.now() - armed > 3000) {
      armed = performance.now();
      sayFor('warn', 'Duyệt kèm ' + n + ' góp ý? Bấm ' + KEY_HINT_SHIFT + ' lần nữa trong 3 giây', 3000);
      return;
    }
    armed = -Infinity;
    deliver('Duyệt');
  };
  // A box of a note, focused: hold the ring so the scroll to it does not move it.
  const focusBox = (n) => {
    n.box.hidden = false;
    sc.hold();
    n.ta.focus();
  };
  return [
    ...nav,
    { k: 'n', group: 'Điều hướng', label: 'Góp ý đã viết kế tiếp', run: () => {
      const all = filled();
      if (!all.length) { say('warn', 'Chưa có góp ý nào'); return; }
      const cur = sc.current();
      const i = last && last.row === cur ? all.indexOf(last.n) : -1;
      const n = i >= 0 ? all[(i + 1) % all.length] : all.find((x) => cur && after(cur, x.box)) || all[0];
      // A note before the first heading has no row: the ring is cleared (the next J goes to the first heading) and the page still scrolls to the box.
      const row = rowBefore(n.box);
      if (!row) sc.clear();
      sc.go(row, { to: n.box, block: 'center' });
      last = { n, row };
    } },
    { k: 'c', group: 'Thao tác', label: 'Góp ý đoạn đang chọn, hoặc góp ý của mục đang chọn', run: () => {
      const n = commentSelection() || rowNote.get(sc.current());
      if (n) focusBox(n);
    } },
    { k: 'g', group: 'Thao tác', label: 'Góp ý chung', run: () => { const n = general(); if (n) focusBox(n); } },
    { k: 'mod+Enter', show: KEY_HINT, group: 'Thao tác', label: 'Gửi góp ý', typing: true, run: () => deliver('Sửa') },
    { k: 'mod+shift+Enter', show: KEY_HINT_SHIFT, group: 'Thao tác', label: 'Duyệt (có góp ý thì bấm hai lần)', typing: true, run: approve },
  ];
};
