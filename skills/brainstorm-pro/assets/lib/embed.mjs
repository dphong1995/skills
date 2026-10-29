// Embeds what a page needs from disk: code files (with the lines a block shows), the files `code` spans and Markdown links name, and images as data URIs.
import { readFileSync, statSync } from 'node:fs';
import { homedir } from 'node:os';
import { dirname, extname, isAbsolute, join, resolve } from 'node:path';
import { PATH } from '../page/strings.js';

const MAX_FILE = 512 * 1024;
const MAX_IMAGE = 3 * 1024 * 1024;
const IMAGE_TYPES = { '.png': 'image/png', '.jpg': 'image/jpeg', '.jpeg': 'image/jpeg', '.gif': 'image/gif', '.webp': 'image/webp', '.svg': 'image/svg+xml' };

export const createEmbed = ({ root, src }) => {
  if (!isAbsolute(root)) throw new Error(`${src}: "root" must be an absolute path: ${root}`);
  const resolvePath = (p) => (p.startsWith('~/') ? join(homedir(), p.slice(2)) : isAbsolute(p) ? p : resolve(root, p));

  const files = {};
  const paths = {};
  const links = {};

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
    if (Buffer.byteLength(content) <= MAX_FILE) files[b.file] = content;
    else console.error(`${b.file}: over ${MAX_FILE / 1024} KB, shown without "Xem toàn file"`);
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
    if (!b.file) throw new Error(`${src}: img block needs "file"`);
    const abs = resolvePath(b.file);
    const type = IMAGE_TYPES[extname(abs).toLowerCase()];
    if (!type) throw new Error(`${src}: ${b.file} is not an image the page shows (png, jpg, gif, webp, svg)`);
    const buf = readFileSync(abs);
    if (buf.length > MAX_IMAGE) {
      throw new Error(`${src}: ${b.file} is ${(buf.length / 1048576).toFixed(1)} MB, over ${MAX_IMAGE / 1048576} MB; resize a copy first: sips --resampleWidth 1600 <image> --out <copy>`);
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

  return { link, prep, image, docLinks, files, paths, links };
};
