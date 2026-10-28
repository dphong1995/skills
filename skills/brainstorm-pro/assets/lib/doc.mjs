// A Markdown document (references/html-doc.md): its page data, and the review rounds that mark what changed between two servings.
import { readFileSync, writeFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { basename, dirname, join, resolve } from 'node:path';
import { createEmbed } from './embed.mjs';

// "## A" and "### B" headings outside code fences, keyed "A" and "A › B" as the page keys them, each with the text up to the next one.
export const sections = (md) => {
  const map = new Map();
  let fence = null, h2 = '', key = null, body = [];
  const flush = () => { if (key != null) map.set(key, body.join('\n').trim()); };
  for (const line of md.split('\n')) {
    const f = /^ {0,3}(`{3,}|~{3,})/.exec(line);
    if (f && (!fence || f[1][0] === fence)) fence = fence ? null : f[1][0];
    const h = !fence && !f && /^ {0,3}(#{2,3})\s+(.+?)(?:\s+#+)?\s*$/.exec(line);
    if (!h) { body.push(line); continue; }
    flush();
    const text = h[2].replace(/`|\*\*/g, '');
    if (h[1].length === 2) key = h2 = text;
    else key = h2 ? h2 + ' › ' + text : text;
    body = [];
  }
  flush();
  return map;
};

export const loadDoc = (src, { review }) => {
  const md = readFileSync(src, 'utf8');
  const h1 = /^ {0,3}#\s+(.+?)\s*$/m.exec(md);
  const data = { doc: { md, path: resolve(src).replace(homedir(), '~'), title: h1 ? h1[1].replace(/`|\*\*/g, '') : basename(src), review } };
  const { link, docLinks, files, paths, links } = createEmbed({ root: process.cwd(), src });
  data.files = files;
  data.paths = paths;
  link(md);
  data.links = links;
  docLinks(md);
  return data;
};

// Review rounds of a document: the state file beside the page keeps the Markdown last served, so the next round marks what changed.
// Returns the changes for the page and a function that saves the state once the page is written.
export const reviewRound = (out, src, md) => {
  const file = join(dirname(resolve(out)), '.review-state.json');
  let state = {};
  try { state = JSON.parse(readFileSync(file, 'utf8')); } catch (e) { state = {}; }
  const key = resolve(src);
  const prev = state[key];
  const changes = { round: prev ? prev.round + 1 : 1 };
  if (prev) {
    const [a, b] = [sections(prev.md), sections(md)];
    changes.changed = [...b.keys()].filter((k) => a.has(k) && a.get(k) !== b.get(k));
    changes.added = [...b.keys()].filter((k) => !a.has(k));
    changes.removed = [...a.keys()].filter((k) => !b.has(k));
  }
  state[key] = { round: changes.round, md };
  return { changes, save: () => writeFileSync(file, JSON.stringify(state)) };
};
