// Embeds what a page needs from disk: code files (with the lines a block shows), the files `code` spans and Markdown links name, and images as data URIs.
import { readFileSync, statSync } from 'node:fs';
import { homedir } from 'node:os';
import { dirname, extname, isAbsolute, join, resolve } from 'node:path';
import { PATH } from '../page/strings.js';

const MAX_FILE = 512 * 1024;
const MAX_IMAGE = 3 * 1024 * 1024;
const IMAGE_TYPES = { '.png': 'image/png', '.jpg': 'image/jpeg', '.jpeg': 'image/jpeg', '.gif': 'image/gif', '.webp': 'image/webp', '.svg': 'image/svg+xml' };

// A problem with what the round or document points at: printed as one line, no stack.
export class InputError extends Error {}

// readFileSync, with a failure reported as an InputError naming `label` (the path by default).
export const readInput = (path, src, enc, label = path) => {
  try { return readFileSync(path, enc); } catch (e) { throw new InputError(`${src}: cannot read ${label} (${e.code})`); }
};

export const createEmbed = ({ root, src }) => {
  if (!isAbsolute(root)) throw new InputError(`${src}: "root" must be an absolute path: ${root}`);
  const resolvePath = (p) => (p.startsWith('~/') ? join(homedir(), p.slice(2)) : isAbsolute(p) ? p : resolve(root, p));

  const files = {};
  const paths = {};
  const links = {};
  const protos = {};

  const prep = (b) => {
    if (!b || !b.file) return;
    if (!isAbsolute(b.file)) throw new InputError(`${src}: code "file" must be an absolute path: ${b.file}`);
    const content = readInput(b.file, src, 'utf8');
    if (b.lines) {
      const [from, to] = (b.lines = b.lines.map(Number));
      const all = content.replace(/\n$/, '').split('\n');
      if (!(from >= 1 && to >= from && to <= all.length)) throw new InputError(`${src}: lines ${from}-${to} are outside ${b.file} (${all.length} lines)`);
      b.text = all.slice(from - 1, to).join('\n');
    } else if (b.text == null) {
      throw new InputError(`${src}: code block for ${b.file} needs "lines" or "text"`);
    }
    if (b.label == null) b.label = b.file.replace(homedir(), '~') + (b.lines ? `:${b.lines[0]}-${b.lines[1]}` : '');
    if (Buffer.byteLength(content) <= MAX_FILE) files[b.file] = content;
    else console.error(`${b.file}: over ${MAX_FILE / 1024} KB, shown without "View whole file"`);
  };

  // Puts the regular file at abs into `files`; false when it is missing, not a file or over the limit (onBig then says so).
  const embedFile = (abs, onBig) => {
    try {
      const st = statSync(abs);
      if (!st.isFile()) return false;
      if (st.size > MAX_FILE) {
        if (onBig) onBig();
        return false;
      }
    } catch (e) {
      return false;
    }
    files[abs] ??= readFileSync(abs, 'utf8');
    return true;
  };

  // `path`, `path:N` or `path:N-M` in text becomes a link when the path has a directory and names a file.
  const link = (text) => {
    if (typeof text !== 'string') return;
    for (const [, span] of text.matchAll(/`([^`]+)`/g)) {
      if (/\s/.test(span)) continue;
      const path = PATH.exec(span)[1];
      if (paths[path] || !(path.includes('/') || path.startsWith('~'))) continue;
      const abs = resolvePath(path);
      if (!embedFile(abs, () => console.error(`${path}: over ${MAX_FILE / 1024} KB, left as plain text`))) continue;
      paths[path] = abs;
    }
  };

  const image = (b) => {
    if (!b.file) throw new InputError(`${src}: img block needs "file"`);
    const abs = resolvePath(b.file);
    const type = IMAGE_TYPES[extname(abs).toLowerCase()];
    if (!type) throw new InputError(`${src}: ${b.file} is not an image the page shows (png, jpg, gif, webp, svg)`);
    const buf = readInput(abs, src, undefined, b.file);
    if (buf.length > MAX_IMAGE) {
      throw new InputError(`${src}: ${b.file} is ${(buf.length / 1048576).toFixed(1)} MB, over ${MAX_IMAGE / 1048576} MB; resize a copy first: sips --resampleWidth 1600 <image> --out <copy>`);
    }
    b.src = `data:${type};base64,${buf.toString('base64')}`;
  };

  // A document's Markdown links to local files, resolved against its own directory.
  const docLinks = (md) => {
    for (const [, href] of md.matchAll(/\]\(<?([^)\s>]+)>?(?:\s+"[^"]*")?\)/g)) {
      if (links[href] || /^[a-z][\w+.-]*:/i.test(href) || href.startsWith('#') || IMAGE_TYPES[extname(href).toLowerCase()]) continue;
      const abs = resolve(dirname(resolve(src)), decodeURIComponent(href.split('#')[0]));
      if (!embedFile(abs)) continue;
      links[href] = abs;
    }
  };

  // A question's "prototype": an .html built per references/prototype.md, embedded as page data (not in `files`).
  const prototype = (q) => {
    if (!q.prototype) return;
    const at = `${src}: ${q.id} "prototype" ${q.prototype}`;
    if (!isAbsolute(q.prototype) && !q.prototype.startsWith('~/')) throw new InputError(`${at} must be an absolute path or start with ~/`);
    const abs = resolvePath(q.prototype);
    if (extname(abs).toLowerCase() !== '.html') throw new InputError(`${at} must be an .html file; build it per ~/.claude/skills/brainstorm-pro/references/prototype.md`);
    const html = readInput(abs, src, 'utf8', q.prototype);
    const size = Buffer.byteLength(html);
    if (size > MAX_FILE) throw new InputError(`${at} is ${Math.ceil(size / 1024)} KB, over ${MAX_FILE / 1024} KB; trim the variants and drop inlined base64 images`);
    if (!html.includes('class="proto-picker"')) throw new InputError(`${at} has no class="proto-picker"; start from ~/.claude/skills/brainstorm-pro/assets/prototype/harness.html`);
    if (!html.includes('id="stage"')) throw new InputError(`${at} has no id="stage"; start from ~/.claude/skills/brainstorm-pro/assets/prototype/harness.html`);
    const n = html.split('class="proto-picker-item"').length - 1;
    // The harness keeps one `() => …` entry per variant in `const variants = [ … ];`; a picker button with no entry does nothing.
    const list = /const variants = \[([\s\S]*?)\n\];/.exec(html);
    if (!list) throw new InputError(`${at} has no "const variants = [ … ];" array; start from ~/.claude/skills/brainstorm-pro/assets/prototype/harness.html`);
    const v = (list[1].match(/\(\)\s*=>/g) || []).length;
    if (v !== n) throw new InputError(`${at} has ${v} entries in variants but ${n} picker buttons; give every variant one button, in the same order`);
    const want = (q.options || []).length;
    if (n !== want) throw new InputError(`${at} has ${n} variants but ${q.id} has ${want} options; make them equal`);
    protos[q.id] = html;
  };

  return { link, prep, image, prototype, docLinks, files, paths, links, protos };
};
