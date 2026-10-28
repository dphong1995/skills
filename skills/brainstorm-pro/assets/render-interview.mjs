// Usage: node render-interview.mjs <round.json> <out.html> [--serve]
// Puts one round of questions (see references/html-interview.md) into interview-page.html.
// Code with a "file" (code blocks, both sides of a diff, an option's code) and file paths written as `inline code` get the whole file embedded, for the page's file viewer; images get embedded as data URIs.
// --serve then serves the page on 127.0.0.1 under a random token, opens it, prints the answers the page sends back and exits (4 when no browser opens).
import { execFile } from 'node:child_process';
import { randomBytes } from 'node:crypto';
import { readFileSync, statSync, writeFileSync } from 'node:fs';
import { createServer } from 'node:http';
import { homedir } from 'node:os';
import { dirname, extname, isAbsolute, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const MAX_FILE = 512 * 1024;
const MAX_IMAGE = 3 * 1024 * 1024;
const MAX_BODY = 1024 * 1024;
const WAIT_MS = 8 * 60 * 60 * 1000;
const IMAGE_TYPES = { '.png': 'image/png', '.jpg': 'image/jpeg', '.jpeg': 'image/jpeg', '.gif': 'image/gif', '.webp': 'image/webp', '.svg': 'image/svg+xml' };

const args = process.argv.slice(2);
const serve = args.includes('--serve');
const [src, out] = args.filter((a) => a !== '--serve');
if (!src || !out) {
  console.error('Usage: node render-interview.mjs <round.json> <out.html> [--serve]');
  process.exit(1);
}

const data = JSON.parse(readFileSync(src, 'utf8'));
if (!Array.isArray(data.questions) || !data.questions.length) throw new Error(`${src}: "questions" must be a non-empty array`);
data.questions.forEach((q, i) => {
  for (const key of ['id', 'n', 'title', 'options']) {
    if (q[key] == null) throw new Error(`${src}: questions[${i}] is missing "${key}"`);
  }
});

const root = data.root || process.cwd();
if (!isAbsolute(root)) throw new Error(`${src}: "root" must be an absolute path: ${root}`);
const resolvePath = (p) => (p.startsWith('~/') ? join(homedir(), p.slice(2)) : isAbsolute(p) ? p : resolve(root, p));

data.files = {};
data.paths = {};
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

// `path`, `path:N` or `path:N-M` in text becomes a link when the path has a directory and names a file.
const link = (text) => {
  if (typeof text !== 'string') return;
  for (const [, span] of text.matchAll(/`([^`]+)`/g)) {
    const path = /^(.+?)(?::(\d+)(?:-(\d+))?)?$/.exec(span)[1];
    if (data.paths[path] || /\s/.test(path) || !(path.includes('/') || path.startsWith('~'))) continue;
    const abs = resolvePath(path);
    let size;
    try {
      const st = statSync(abs);
      if (!st.isFile()) continue;
      size = st.size;
    } catch (e) {
      continue;
    }
    if (size > MAX_FILE) {
      console.error(`${path}: over ${MAX_FILE / 1024} KB, left as plain text`);
      continue;
    }
    data.files[abs] ??= readFileSync(abs, 'utf8');
    data.paths[path] = abs;
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

const template = readFileSync(join(dirname(fileURLToPath(import.meta.url)), 'interview-page.html'), 'utf8');
const placeholder = '/*__INTERVIEW_DATA__*/null';
if (!template.includes(placeholder)) throw new Error('interview-page.html: data placeholder not found');
const page = (d) => template.replace(placeholder, () => JSON.stringify(d).replace(/</g, '\\u003c'));

writeFileSync(out, page(data));
console.log(out);

if (serve) {
  const base = `/${randomBytes(16).toString('hex')}/`;
  let origin = '';
  let html = '';
  const server = createServer((req, res) => {
    if (req.method === 'GET' && req.url === base) {
      res.writeHead(200, { 'Content-Type': 'text/html; charset=utf-8', 'Cache-Control': 'no-store' });
      res.end(html);
      return;
    }
    if (req.method !== 'POST' || req.url !== base + 'answers') {
      res.writeHead(404).end();
      return;
    }
    // Only the page itself may answer: another site open in the browser cannot know the token or send this Origin.
    if (req.headers.origin !== origin) {
      res.writeHead(403).end();
      return;
    }
    if (!/^application\/json\b/.test(req.headers['content-type'] || '')) {
      res.writeHead(415).end();
      return;
    }
    let body = '';
    req.setEncoding('utf8');
    req.on('data', (chunk) => {
      body += chunk;
      if (Buffer.byteLength(body) > MAX_BODY) {
        res.writeHead(413).end();
        req.destroy();
      }
    });
    req.on('end', () => {
      if (res.writableEnded) return;
      let text = null;
      try {
        text = JSON.parse(body).text;
      } catch (e) {
        text = null;
      }
      if (typeof text !== 'string' || !text.trim()) {
        res.writeHead(400).end();
        return;
      }
      res.writeHead(200, { 'Content-Type': 'application/json' });
      res.end('{"ok":true}', () => {
        process.stdout.write(`ANSWERS ${resolve(src)}\n${text.trim()}\n`, () => process.exit(0));
      });
    });
  });
  server.on('error', (e) => {
    console.error(`Cannot serve the page: ${e.message}`);
    process.exit(1);
  });
  server.listen(0, '127.0.0.1', () => {
    origin = `http://127.0.0.1:${server.address().port}`;
    html = page({ ...data, send: base + 'answers' });
    const url = origin + base;
    console.log(`Serving ${url}`);
    execFile(process.platform === 'darwin' ? 'open' : 'xdg-open', [url], (err) => {
      if (err) {
        console.error(`Could not open a browser (${err.message})`);
        process.exit(4);
      }
    });
  });
  setTimeout(() => {
    console.log(`NO ANSWER after ${WAIT_MS / 3600000}h`);
    process.exit(3);
  }, WAIT_MS);
}
