import { el } from './dom.js';
import { CARD, main } from './fab.js';
import { docPage } from './doc.js';
import { interviewPage } from './interview.js';

if (D && D.doc) docPage(D.doc);
else if (!D || !D.questions || !D.questions.length) {
  main.append(el('article', { class: CARD }, el('p', { text: 'File này là mẫu, chưa có câu hỏi. Tạo trang câu hỏi bằng render-interview.mjs.' })));
}
else interviewPage();
