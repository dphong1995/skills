import { btnCls, el, kbd, modal } from './dom.js';
import { say } from './fab.js';
import { keyName } from './keys.js';

// The review page's send chooser: with notes, every send asks Duyệt or Cần sửa (the dialog takes D and S itself; autofocus on its box means Enter picks nothing).
// `count()` is the number of notes, `send(decision)` sends them, `busy()` is true while a send is running. Returns submit(decision), what the buttons and keys call.
export const sendChooser = ({ count, send, busy }) => {
  const title = el('span', { class: 'text-[16px] font-semibold' });
  const choose = (decision) => { dialog.close(); send(decision); };
  const dialog = modal('max-w-sm grid gap-3', title,
    el('div', { class: 'flex gap-2' },
      el('button', { type: 'button', class: btnCls() + ' btn-success', onclick: () => choose('Duyệt') }, 'Duyệt', kbd('D', 'kbd-xs')),
      el('button', { type: 'button', class: btnCls('primary'), onclick: () => choose('Sửa') }, 'Cần sửa', kbd('S', 'kbd-xs'))),
    el('span', { class: 'text-[13px] text-muted', text: 'Duyệt: Claude áp dụng góp ý rồi đi tiếp. Cần sửa: Claude sửa rồi cho bạn xem lần 2.' }));
  dialog.firstElementChild.setAttribute('tabindex', '-1');
  dialog.firstElementChild.setAttribute('autofocus', '');
  dialog.addEventListener('keydown', (e) => {
    const k = keyName(e);
    if (k !== 'd' && k !== 's') return;
    e.preventDefault();
    choose(k === 'd' ? 'Duyệt' : 'Sửa');
  });
  document.body.append(dialog);
  return (decision) => {
    if (busy()) return;
    const n = count();
    if (n) {
      title.textContent = 'Gửi ' + n + ' góp ý như:';
      dialog.showModal();
    } else if (decision === 'Duyệt') send('Duyệt');
    else say('warn', 'Chưa có góp ý: tô đen một đoạn rồi bấm "Góp ý", bấm "Góp ý" ở mục cần sửa, hoặc ghi vào Góp ý chung cuối trang.');
  };
};
