import { KEY_HINT } from './dom.js';
import { nextWhere } from './keys.js';
import { say, sendMode } from './fab.js';
import { t } from './i18n.js';

// The interview page's key list: the cards are the targets, the answer keys act on the current card.
export const interviewKeys = (sc, { cards, submit }) => {
  const at = () => cards.findIndex((c) => c.card === sc.current());
  const onCard = (f) => (k) => { const c = cards[at()]; if (c) f(c, k); };
  return [
    ...sc.nav(t('navNextQuestion'), t('navPrevQuestion'), t('navNthQuestion')),
    { k: 'n', group: t('groupNav'), label: t('keyNextUnanswered'), run: () => {
      const i = nextWhere(cards, at(), (c) => !c.done());
      if (i < 0) say('ok', t('allAnswered', KEY_HINT));
      else sc.go(cards[i].card);
    } },
    { k: ['a', 'b', 'c', 'd'], show: 'A–D', group: t('groupAct'), label: t('keyChoose'), run: onCard((c, k) => c.choose(c.ids[k.charCodeAt(0) - 97])) },
    { k: 'r', group: t('groupAct'), label: t('keyRecommended'), run: onCard((c) => {
      if (c.rec == null) say('warn', t('noRecommendation'));
      else c.pick(c.rec);
    }) },
    // Checks the option if it is not yet (a multi card would otherwise toggle it off) and puts the cursor in its box.
    { k: 'x', group: t('groupAct'), label: t('optOther'), run: onCard((c) => { c.pick('other'); c.otherBox.focus(); }) },
    { k: 'e', group: t('groupAct'), label: t('optExplain'), run: onCard((c) => { c.pick('explain'); c.note.focus(); }) },
    { k: 'g', group: t('groupAct'), label: t('keyCardNote'), run: onCard((c) => c.note.focus()) },
    { k: 'mod+Enter', show: KEY_HINT, group: t('groupAct'), label: t(sendMode ? 'sendAnswers' : 'copyAnswers'), typing: true, run: submit },
  ];
};
