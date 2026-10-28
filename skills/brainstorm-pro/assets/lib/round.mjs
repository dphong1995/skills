// One round of questions (references/html-interview.md): the JSON is checked, then everything it points at is embedded.
import { readFileSync } from 'node:fs';
import { createEmbed } from './embed.mjs';

export const validateRound = (data, src) => {
  if (!Array.isArray(data.questions) || !data.questions.length) throw new Error(`${src}: "questions" must be a non-empty array`);
  data.questions.forEach((q, i) => {
    for (const key of ['id', 'n', 'title', 'options']) {
      if (q[key] == null) throw new Error(`${src}: questions[${i}] is missing "${key}"`);
    }
    const k = q.options.filter((o) => o && o.recommended).length;
    if (k > 1) throw new Error(`${src}: questions[${i}] marks ${k} options "recommended"; mark one`);
    if (q.recommendation && !k) throw new Error(`${src}: questions[${i}] has "recommendation" but no option with "recommended": true`);
  });
};

export const loadRound = (src) => {
  const data = JSON.parse(readFileSync(src, 'utf8'));
  validateRound(data, src);
  const { link, prep, image, files, paths } = createEmbed({ root: data.root || process.cwd(), src });
  data.files = files;
  data.paths = paths;
  (data.decided || []).forEach(link);
  for (const q of data.questions) {
    [q.title, q.why, q.reversibleWhy, q.recommendation].forEach(link);
    for (const b of q.context || []) {
      if (!b) continue;
      if (b.t === 'code') prep(b);
      if (b.t === 'diff') { prep(b.before); prep(b.after); }
      if (b.t === 'img') image(b);
      if (b.t === 'p' || b.t === 'note') link(b.text);
      if (b.t === 'list') (b.items || []).forEach(link);
      if (b.t === 'table') [b.head || [], ...(b.rows || [])].flat().forEach(link);
    }
    for (const o of q.options || []) {
      prep(o.code);
      [o.label, o.means, o.example, o.effort, ...(o.pros || []), ...(o.cons || [])].forEach(link);
    }
  }
  return data;
};
