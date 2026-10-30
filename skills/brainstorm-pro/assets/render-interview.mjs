// Usage: node render-interview.mjs <round.json | document.md> <out.html> [--serve | --open]
// Puts one round of questions (references/html-interview.md), or a Markdown document (references/html-doc.md), into one self-contained page built from page/ by lib/build.mjs.
// Code with a "file" (code blocks, both sides of a diff, an option's code), file paths written as `inline code` and a document's links to local files get the whole file embedded, for the page's file viewer; images get embedded as data URIs.
// --serve then serves the page on 127.0.0.1 under a random token (also taking the images it uploads, saved beside <out.html>), opens it, prints the answers (or the document review) the page sends back and exits (4 when no browser opens).
// --open opens the written page as a file and exits: a document to read only.
// Layout: lib/round.mjs and lib/doc.mjs load the input (lib/embed.mjs reads the files it points at), lib/build.mjs builds the page, lib/server.mjs serves it.
import { mkdirSync, writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { buildPage, bundle } from './lib/build.mjs';
import { loadDoc, reviewRound } from './lib/doc.mjs';
import { InputError } from './lib/embed.mjs';
import { loadRound } from './lib/round.mjs';
import { openBrowser, serve } from './lib/server.mjs';

const args = process.argv.slice(2);
const serving = args.includes('--serve');
const open = args.includes('--open');
const [src, out] = args.filter((a) => a !== '--serve' && a !== '--open');
if (!src || !out || (serving && open)) {
  console.error('Usage: node render-interview.mjs <round.json | document.md> <out.html> [--serve | --open]');
  process.exit(1);
}
const isDoc = /\.(md|markdown)$/i.test(src);

let data, saveRound = null, saveNotes = null;
try {
  data = isDoc ? loadDoc(src, { review: serving }) : loadRound(src);
  if (isDoc && serving) {
    const round = reviewRound(out, src, data.doc.md);
    data.doc.changes = round.changes;
    saveRound = round.save;
    saveNotes = round.saveNotes;
  }
} catch (e) {
  if (!(e instanceof InputError)) throw e;
  console.error(e.message);
  process.exit(1);
}

const bundled = bundle();
mkdirSync(dirname(resolve(out)), { recursive: true });
writeFileSync(out, buildPage(data, { bundled }));
if (saveRound) saveRound();
console.log(out);

if (open) openBrowser(resolve(out));
if (serving) serve({ src, out, onNotes: saveNotes, page: (extra) => buildPage({ ...data, ...extra }, { bundled }) });
