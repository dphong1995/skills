// Builds the one self-contained HTML page from assets/page/: shell.html, the two stylesheets and a single script.
// The script is page/main.js and what it imports (one-line `import { a, b } from './x.js';`), ordered so that a file comes after the files it imports, with the import lines and the `export ` prefixes dropped. All files then share one scope, so no two may declare the same top-level name.
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

export const PAGE_DIR = join(dirname(fileURLToPath(import.meta.url)), '..', 'page');

const IMPORT = /^import \{[^}]*\} from '\.\/([\w-]+\.js)';\s*$/;
const DECL = /^(?:async\s+)?(?:const|let|var|function\*?|class)\s+(?:([\w$]+)|[{[]([^}\]]*)[}\]])/;

// { script, names }: the bundled modules, and each top-level name with the file that declares it.
export const bundle = (pageDir = PAGE_DIR) => {
  const done = new Set();
  const chain = [];
  const order = [];
  const visit = (file) => {
    if (chain.includes(file)) throw new Error(`import cycle: ${[...chain.slice(chain.indexOf(file)), file].join(' -> ')}`);
    if (done.has(file)) return;
    chain.push(file);
    const lines = readFileSync(join(pageDir, file), 'utf8').split('\n');
    const kept = [];
    lines.forEach((line, i) => {
      const m = IMPORT.exec(line);
      if (m) { visit(m[1]); return; }
      if (/^export\s+(default|\{|\*)/.test(line)) throw new Error(`${file}:${i + 1}: only "export const|let|function|class" is supported: ${line.trim()}`);
      kept.push(line.replace(/^export /, ''));
    });
    chain.pop();
    done.add(file);
    order.push({ file, lines: kept });
  };
  visit('main.js');

  const names = new Map([['D', 'build.mjs']]);
  const parts = [];
  for (const { file, lines } of order) {
    lines.forEach((line, i) => {
      if (/^\s*(import|export)\b/.test(line)) throw new Error(`${file}:${i + 1}: unsupported import/export: ${line.trim()}`);
      const m = DECL.exec(line);
      if (!m) return;
      for (const name of m[1] ? [m[1]] : m[2].split(',').map((s) => s.split(/[:=]/).pop().trim()).filter(Boolean)) {
        if (names.has(name)) throw new Error(`duplicate top-level name "${name}": ${names.get(name)} and ${file}`);
        names.set(name, file);
      }
    });
    parts.push(lines.join('\n').trimEnd());
  }
  return { script: parts.join('\n\n') + '\n', names };
};

// `bundled` is a bundle() result, to build several pages from one bundle.
export const buildPage = (data, { pageDir = PAGE_DIR, bundled = bundle(pageDir) } = {}) => {
  const read = (name) => readFileSync(join(pageDir, name), 'utf8');
  const json = JSON.stringify(data).replace(/</g, '\\u003c');
  const script = `(() => {\n'use strict';\nconst D = ${json};\n${bundled.script}})();\n`;
  const fill = { '/*__THEME__*/': read('theme.css'), '/*__TAILWIND__*/': read('tailwind.css'), '/*__SCRIPT__*/': script };
  let html = read('shell.html');
  for (const [marker, text] of Object.entries(fill)) {
    if (!html.includes(marker)) throw new Error(`shell.html: ${marker} not found`);
    html = html.replace(marker, () => text);
  }
  return html;
};
