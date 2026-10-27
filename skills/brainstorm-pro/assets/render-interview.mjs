// Usage: node render-interview.mjs <round.json> <out.html>
// Puts one round of questions (see references/html-interview.md) into interview-page.html.
// Code with a "file" (code blocks, both sides of a diff, an option's code) gets the whole file embedded, for the page's "Xem toàn file" viewer.
import { readFileSync, writeFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { dirname, isAbsolute, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const MAX_FILE = 512 * 1024;

const [, , src, out] = process.argv;
if (!src || !out) {
  console.error('Usage: node render-interview.mjs <round.json> <out.html>');
  process.exit(1);
}

const data = JSON.parse(readFileSync(src, 'utf8'));
if (!Array.isArray(data.questions) || !data.questions.length) throw new Error(`${src}: "questions" must be a non-empty array`);
data.questions.forEach((q, i) => {
  for (const key of ['id', 'n', 'title', 'options']) {
    if (q[key] == null) throw new Error(`${src}: questions[${i}] is missing "${key}"`);
  }
});

data.files = {};
const prep = (b) => {
  if (!b || !b.file) return;
  if (!isAbsolute(b.file)) throw new Error(`${src}: code "file" must be an absolute path: ${b.file}`);
  const content = readFileSync(b.file, 'utf8');
  if (b.lines) {
    const [from, to] = (b.lines = b.lines.map(Number));
    const all = content.replace(/\n$/, '').split('\n');
    if (!(from >= 1 && to >= from && to <= all.length)) throw new Error(`${src}: lines ${from}-${to} are outside ${b.file} (${all.length} lines)`);
    b.text = all.slice(from - 1, to).join('\n');
  } else if (b.text == null) {
    throw new Error(`${src}: code block for ${b.file} needs "lines" or "text"`);
  }
  if (b.label == null) b.label = b.file.replace(homedir(), '~') + (b.lines ? `:${b.lines[0]}-${b.lines[1]}` : '');
  if (Buffer.byteLength(content) <= MAX_FILE) data.files[b.file] = content;
  else console.error(`${b.file}: over ${MAX_FILE / 1024} KB, shown without "Xem toàn file"`);
};
for (const q of data.questions) {
  for (const b of q.context || []) {
    if (b && b.t === 'code') prep(b);
    if (b && b.t === 'diff') { prep(b.before); prep(b.after); }
  }
  for (const o of q.options || []) prep(o.code);
}

const template = readFileSync(join(dirname(fileURLToPath(import.meta.url)), 'interview-page.html'), 'utf8');
const placeholder = '/*__INTERVIEW_DATA__*/null';
if (!template.includes(placeholder)) throw new Error('interview-page.html: data placeholder not found');

const json = JSON.stringify(data).replace(/</g, '\\u003c');
writeFileSync(out, template.replace(placeholder, () => json));
console.log(out);
