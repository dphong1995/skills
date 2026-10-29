import { KEY_HINT, KEY_HINT_SHIFT } from './dom.js';
import { fabMsg, say } from './fab.js';

const after = (a, b) => !!(a.compareDocumentPosition(b) & Node.DOCUMENT_POSITION_FOLLOWING);

// The document page's key list: the ## and ### heading rows are the targets. Read-only keeps navigation only.
export const docKeys = (sc, { review, filled, rowNote, commentSelection, general, deliver }) => {
  const nav = sc.nav('Tiêu đề kế', 'Tiêu đề trước', 'Mục ## thứ n');
  if (!review) return nav;
  const rowBefore = (box) => sc.targets().filter((r) => after(r, box)).pop() || null;
  // N goes on from the note it last showed, else from the current heading.
  let last = null;
  let armed = -Infinity;
  const approve = () => {
    const n = filled().length;
    if (n && performance.now() - armed > 3000) {
      armed = performance.now();
      const msg = 'Duyệt kèm ' + n + ' góp ý? Bấm ' + KEY_HINT_SHIFT + ' lần nữa trong 3 giây';
      say('warn', msg);
      setTimeout(() => { if (fabMsg.textContent === msg) say(''); }, 3000);
      return;
    }
    armed = -Infinity;
    deliver('Duyệt');
  };
  return [
    ...nav,
    { k: 'n', group: 'Điều hướng', label: 'Góp ý đã viết kế tiếp', run: () => {
      const all = filled();
      if (!all.length) { say('warn', 'Chưa có góp ý nào'); return; }
      const cur = sc.current();
      const i = last && last.row === cur ? all.indexOf(last.n) : -1;
      const n = i >= 0 ? all[(i + 1) % all.length] : all.find((x) => cur && after(cur, x.box)) || all[0];
      last = { n, row: rowBefore(n.box) };
      sc.go(last.row, { to: n.box, block: 'center' });
    } },
    { k: 'c', group: 'Thao tác', label: 'Góp ý đoạn đang chọn, hoặc góp ý của mục đang chọn', run: () => {
      const n = commentSelection() || rowNote.get(sc.current());
      if (!n) return;
      n.box.hidden = false;
      n.ta.focus();
    } },
    { k: 'g', group: 'Thao tác', label: 'Góp ý chung', run: () => general() && general().ta.focus() },
    { k: 'mod+Enter', show: KEY_HINT, group: 'Thao tác', label: 'Gửi góp ý', typing: true, run: () => deliver('Sửa') },
    { k: 'mod+shift+Enter', show: KEY_HINT_SHIFT, group: 'Thao tác', label: 'Duyệt (có góp ý thì bấm hai lần)', typing: true, run: approve },
  ];
};
