import { KEY_HINT } from './dom.js';
import { nextWhere } from './keys.js';
import { say, sendMode } from './fab.js';

// The interview page's key list: the cards are the targets, the answer keys act on the current card.
export const interviewKeys = (sc, { cards, submit }) => {
  const at = () => cards.findIndex((c) => c.card === sc.current());
  const onCard = (f) => () => { const c = cards[at()]; if (c) f(c); };
  return [
    ...sc.nav('Câu kế', 'Câu trước', 'Câu thứ n'),
    { k: 'n', group: 'Điều hướng', label: 'Câu chưa trả lời kế tiếp', run: () => {
      const i = nextWhere(cards, at(), (c) => !c.done());
      if (i < 0) say('ok', 'Đã trả lời hết, ' + KEY_HINT + ' để gửi');
      else sc.go(cards[i].card);
    } },
    { k: ['a', 'b', 'c', 'd'], show: 'A–D', group: 'Thao tác', label: 'Chọn phương án (câu nhiều lựa chọn: bấm lại để bỏ)', run: (k) => { const c = cards[at()]; if (c) c.choose(c.ids[k.charCodeAt(0) - 97]); } },
    { k: 'r', group: 'Thao tác', label: 'Chọn phương án Claude đề xuất', run: onCard((c) => {
      if (c.rec == null) { say('warn', 'Câu này không có đề xuất'); return; }
      if (!c.isChosen(c.rec)) c.choose(c.rec);
    }) },
    { k: 'x', group: 'Thao tác', label: 'Khác (tự nhập)', run: onCard((c) => c.choose('other')) },
    { k: 'e', group: 'Thao tác', label: 'Cần giải thích thêm', run: onCard((c) => c.choose('explain')) },
    { k: 'g', group: 'Thao tác', label: 'Ghi chú của câu', run: onCard((c) => c.note.focus()) },
    { k: 'mod+Enter', show: KEY_HINT, group: 'Thao tác', label: sendMode ? 'Gửi câu trả lời' : 'Copy câu trả lời', typing: true, run: submit },
  ];
};
