import { $, el } from './dom.js';
import { CARD, main } from './fab.js';
import { docPage } from './doc.js';
import { interviewPage } from './interview.js';
import { setLang, t } from './i18n.js';

// The shell is static: the language, the default title and topic and the nav label come from here.
setLang(D && D.lang);
document.documentElement.lang = D && D.lang === 'en' ? 'en' : 'vi';
document.title = t('defaultTopic');
$('#topic').textContent = t('defaultTopic');
$('#qnav').setAttribute('aria-label', t('questionsNav'));

if (D && D.doc) docPage(D.doc);
else if (!D || !D.questions || !D.questions.length) {
  main.append(el('article', { class: CARD }, el('p', { text: t('sampleFile') })));
}
else interviewPage();
