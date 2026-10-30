// One round of questions (references/html-interview.md): the JSON is checked, then everything it points at is embedded.
import { readFileSync } from 'node:fs';
import { createEmbed, InputError } from './embed.mjs';

const CODE = ['file', 'lines', 'text', 'label', 'lang'];
const FIELDS = {
  round: ['root', 'topic', 'decided', 'questions'],
  question: ['id', 'n', 'stage', 'title', 'why', 'reversible', 'reversibleWhy', 'context', 'options', 'multi', 'recommendation'],
  option: ['id', 'label', 'means', 'example', 'pros', 'cons', 'effort', 'code', 'mockup'],
  code: CODE,
  recommendation: ['option', 'why'],
  block: {
    p: ['text'],
    note: ['text'],
    list: ['items', 'ordered'],
    code: CODE,
    diff: ['label', 'lang', 'before', 'after'],
    mermaid: ['text', 'label'],
    img: ['file', 'label', 'alt'],
    mockup: ['html', 'label'],
    table: ['head', 'rows'],
  },
};

// Optimal string alignment distance: Levenshtein plus the swap of two adjacent letters as one edit ("titel" is 1 from "title").
const distance = (a, b) => {
  const d = Array.from({ length: a.length + 1 }, (_, i) => [i]);
  for (let j = 1; j <= b.length; j++) d[0][j] = j;
  for (let i = 1; i <= a.length; i++) {
    for (let j = 1; j <= b.length; j++) {
      d[i][j] = Math.min(d[i - 1][j] + 1, d[i][j - 1] + 1, d[i - 1][j - 1] + (a[i - 1] === b[j - 1] ? 0 : 1));
      if (i > 1 && j > 1 && a[i - 1] === b[j - 2] && a[i - 2] === b[j - 1]) d[i][j] = Math.min(d[i][j], d[i - 2][j - 2] + 1);
    }
  }
  return d[a.length][b.length];
};

const checkKeys = (obj, allowed, where, src) => {
  for (const k of Object.keys(obj)) {
    if (allowed.includes(k)) continue;
    const best = allowed.map((a) => [distance(k, a), a]).sort((x, y) => x[0] - y[0])[0];
    throw new InputError(`${src}: ${where} has unknown field "${k}"` + (best && best[0] <= Math.min(2, Math.floor(k.length / 3)) ? `; did you mean "${best[1]}"?` : `; allowed: ${allowed.join(', ')}`));
  }
};

export const object = (v) => v && typeof v === 'object' && !Array.isArray(v);

const array = (v, where, src) => {
  if (v != null && !Array.isArray(v)) throw new InputError(`${src}: ${where} must be an array`);
};

const checkLines = (c, where, src) => {
  if (c.lines != null && !(Array.isArray(c.lines) && c.lines.length === 2 && c.lines.every((n) => typeof n === 'number'))) throw new InputError(`${src}: ${where} "lines" must be [from, to] numbers`);
};

const checkCode = (c, where, src) => {
  if (!object(c)) throw new InputError(`${src}: ${where} must be an object`);
  checkKeys(c, FIELDS.code, where, src);
  checkLines(c, where, src);
};

export const validateRound = (data, src) => {
  if (!object(data)) throw new InputError(`${src}: the round must be a JSON object`);
  checkKeys(data, FIELDS.round, 'the round', src);
  array(data.decided, '"decided"', src);
  if (!Array.isArray(data.questions) || !data.questions.length) throw new InputError(`${src}: "questions" must be a non-empty array`);
  data.questions.forEach((q, i) => {
    if (!object(q)) throw new InputError(`${src}: questions[${i}] must be an object`);
    const at = `questions[${i}]` + (typeof q.id === 'string' ? ` (${q.id})` : '');
    checkKeys(q, FIELDS.question, at, src);
    for (const key of ['id', 'n', 'title', 'options']) {
      if (q[key] == null) throw new InputError(`${src}: questions[${i}] is missing "${key}"`);
    }
    if (!Array.isArray(q.options)) throw new InputError(`${src}: ${at} "options" must be an array`);
    array(q.context, `${at} "context"`, src);
    q.options.forEach((o, j) => {
      const oat = `questions[${i}].options[${j}]` + (o && typeof o.id === 'string' ? ` (${o.id})` : '');
      if (!object(o)) throw new InputError(`${src}: ${oat} must be an object`);
      if ('recommended' in o) throw new InputError(`${src}: ${oat} has "recommended"; name the pick in the question's "recommendation": { "option": "${o.id}", "why": "…" }`);
      checkKeys(o, FIELDS.option, oat, src);
      array(o.pros, `${oat} "pros"`, src);
      array(o.cons, `${oat} "cons"`, src);
      if (o.code != null) checkCode(o.code, `${oat}.code`, src);
    });
    (q.context || []).forEach((b, j) => {
      const bat = `questions[${i}].context[${j}]`;
      if (!object(b)) throw new InputError(`${src}: ${bat} must be an object`);
      const fields = FIELDS.block[b.t];
      if (!fields) throw new InputError(`${src}: ${bat} has unknown block type ${JSON.stringify(b.t)}; valid types: ${Object.keys(FIELDS.block).join(', ')}`);
      checkKeys(b, ['t', ...fields], `${bat} (${b.t})`, src);
      if (b.t === 'code') checkLines(b, bat, src);
      array(b.items, `${bat} "items"`, src);
      array(b.head, `${bat} "head"`, src);
      array(b.rows, `${bat} "rows"`, src);
      (b.rows || []).forEach((row, r) => { if (!Array.isArray(row)) throw new InputError(`${src}: ${bat}.rows[${r}] must be an array`); });
      if (b.t === 'diff') for (const side of ['before', 'after']) if (b[side] != null) checkCode(b[side], `${bat}.${side}`, src);
    });
    if (q.recommendation != null) {
      const r = q.recommendation;
      const rat = `${at} "recommendation"`;
      if (!object(r)) throw new InputError(`${src}: ${rat} must be an object: { "option": "<option id>", "why": "…" }`);
      checkKeys(r, FIELDS.recommendation, rat, src);
      for (const key of FIELDS.recommendation) {
        if (typeof r[key] !== 'string') throw new InputError(`${src}: ${rat} needs a string "${key}"`);
      }
      const ids = q.options.map((o) => o.id);
      if (!ids.includes(r.option)) throw new InputError(`${src}: ${rat} option "${r.option}" is not an option id; ids: ${ids.join(', ')}`);
    }
  });
};

export const loadRound = (src) => {
  let data;
  try { data = JSON.parse(readFileSync(src, 'utf8')); } catch (e) {
    throw new InputError(`${src}: ${e instanceof SyntaxError ? 'not valid JSON: ' : 'cannot read: '}${e.message}`.replace(/\s+/g, ' '));
  }
  validateRound(data, src);
  const { link, prep, image, files, paths } = createEmbed({ root: data.root || process.cwd(), src });
  data.files = files;
  data.paths = paths;
  (data.decided || []).forEach(link);
  for (const q of data.questions) {
    [q.title, q.why, q.reversibleWhy, q.recommendation && q.recommendation.why].forEach(link);
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
