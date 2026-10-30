// --serve: the page on 127.0.0.1 under a random token; it takes the answers (printed as ANSWERS, exit 0) and pasted images (saved beside the page), and gives up after eight hours (exit 3).
import { execFile } from 'node:child_process';
import { randomBytes } from 'node:crypto';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { createServer } from 'node:http';
import { basename, dirname, join, resolve } from 'node:path';
import { object } from './round.mjs';

const MAX_BODY = 1024 * 1024;
const MAX_UPLOAD = 20 * 1024 * 1024;
const WAIT_MS = 8 * 60 * 60 * 1000;
const UPLOAD_TYPES = { 'image/png': '.png', 'image/jpeg': '.jpg', 'image/gif': '.gif', 'image/webp': '.webp' };

// Exits 4 when no browser opens.
export const openBrowser = (target) => {
  execFile(process.platform === 'darwin' ? 'open' : 'xdg-open', [target], (err) => {
    if (err) {
      console.error(`Could not open a browser (${err.message})`);
      process.exit(4);
    }
  });
};

// A review note as the page sends it: strings for kind, key and text, other fields strings or absent.
const validNote = (n) => object(n) && ['kind', 'key', 'text'].every((k) => typeof n[k] === 'string')
  && ['section', 'general', 'passage', 'edit'].includes(n.kind) && (n.kind !== 'edit' || typeof n.to === 'string')
  && Object.values(n).every((v) => typeof v === 'string');

// page({ send, upload }) returns the HTML with the two endpoints the page posts to.
export const serve = ({ src, out, page, onNotes }) => {
  const base = `/${randomBytes(16).toString('hex')}/`;
  let origin = '';
  let html = '';
  const uploads = new Map();
  const server = createServer((req, res) => {
    if (req.method === 'GET' && req.url === base) {
      res.writeHead(200, { 'Content-Type': 'text/html; charset=utf-8', 'Cache-Control': 'no-store' });
      res.end(html);
      return;
    }
    if (req.method === 'GET' && req.url.startsWith(base + 'images/')) {
      const file = uploads.get(req.url.slice(base.length + 7));
      if (!file) {
        res.writeHead(404).end();
        return;
      }
      res.writeHead(200, { 'Content-Type': file.type, 'Cache-Control': 'no-store' });
      res.end(readFileSync(file.path));
      return;
    }
    const route = req.method === 'POST' && { [base + 'answers']: answers, [base + 'image']: upload }[req.url];
    if (!route) {
      res.writeHead(404).end();
      return;
    }
    // Only the page itself may answer: another site open in the browser cannot know the token or send this Origin.
    if (req.headers.origin !== origin) {
      res.writeHead(403).end();
      return;
    }
    route(req, res);
  });
  // The request body, or null once the response is over (413 when it passes `max` bytes).
  const readBody = (req, res, max) => new Promise((done) => {
    const chunks = [];
    let size = 0;
    let over = false;
    req.on('data', (chunk) => {
      if (over) return;
      size += chunk.length;
      if (size > max) {
        over = true;
        res.writeHead(413).end();
        req.destroy();
        done(null);
        return;
      }
      chunks.push(chunk);
    });
    req.on('end', () => done(res.writableEnded ? null : Buffer.concat(chunks)));
  });
  const answers = async (req, res) => {
    if (!/^application\/json\b/.test(req.headers['content-type'] || '')) {
      res.writeHead(415).end();
      return;
    }
    const body = await readBody(req, res, MAX_BODY);
    if (!body) return;
    let text = null, notes = null;
    try {
      ({ text, notes } = JSON.parse(body.toString('utf8')));
    } catch (e) {
      text = null;
    }
    if (typeof text !== 'string' || !text.trim()) {
      res.writeHead(400).end();
      return;
    }
    if (onNotes && Array.isArray(notes)) {
      try { onNotes(notes.filter(validNote)); } catch (e) { console.error(`Could not save the review notes: ${e.message}`); }
    }
    res.writeHead(200, { 'Content-Type': 'application/json' });
    res.end('{"ok":true}', () => {
      process.stdout.write(`ANSWERS ${resolve(src)}\n${text.trim()}\n`, () => process.exit(0));
    });
  };
  // A pasted or dropped image, saved beside the page as <out>-<n>.<ext>; the page shows it back from images/<name>.
  const upload = async (req, res) => {
    const type = (req.headers['content-type'] || '').split(';')[0].trim().toLowerCase();
    if (!UPLOAD_TYPES[type]) {
      res.writeHead(415).end();
      return;
    }
    const body = await readBody(req, res, MAX_UPLOAD);
    if (!body) return;
    if (!body.length) {
      res.writeHead(400).end();
      return;
    }
    const n = uploads.size + 1;
    const name = `${basename(out).replace(/\.html$/i, '')}-${n}${UPLOAD_TYPES[type]}`;
    const dir = join(dirname(resolve(out)), 'images');
    mkdirSync(dir, { recursive: true });
    const path = join(dir, name);
    writeFileSync(path, body);
    uploads.set(name, { path, type });
    res.writeHead(200, { 'Content-Type': 'application/json' });
    res.end(JSON.stringify({ n, path, url: `${base}images/${name}` }));
  };
  server.on('error', (e) => {
    console.error(`Cannot serve the page: ${e.message}`);
    process.exit(1);
  });
  server.listen(0, '127.0.0.1', () => {
    origin = `http://127.0.0.1:${server.address().port}`;
    html = page({ send: base + 'answers', upload: base + 'image' });
    const url = origin + base;
    console.log(`Serving ${url}`);
    openBrowser(url);
  });
  setTimeout(() => {
    console.log(`NO ANSWER after ${WAIT_MS / 3600000}h`);
    process.exit(3);
  }, WAIT_MS);
};
