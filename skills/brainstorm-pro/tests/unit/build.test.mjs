// lib/build.mjs on temporary page directories and on the real one, no browser.
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { outDir } from '../harness/serve.mjs';
import { buildPage, bundle, PAGE_DIR } from '../../assets/lib/build.mjs';

let n = 0;
// A page directory holding the given files (and an empty shell and stylesheets unless given).
const pageDir = (files) => {
  const dir = outDir('unit-build/p' + ++n);
  mkdirSync(dir, { recursive: true });
  const all = { 'shell.html': '<s>/*__THEME__*/|/*__TAILWIND__*/|/*__SCRIPT__*/</s>', 'theme.css': 'T', 'tailwind.css': 'W', ...files };
  for (const [name, text] of Object.entries(all)) writeFileSync(join(dir, name), text);
  return dir;
};

describe('bundle', () => {
  it('puts a file after the files it imports, each once, without import and export', () => {
    const dir = pageDir({
      'main.js': "import { b } from './b.js';\nimport { c } from './c.js';\nconsole.log(b, c);\n",
      'b.js': "import { c } from './c.js';\nexport const b = c + 1;\n",
      'c.js': 'export const c = 1;\n',
    });
    const { script, names } = bundle(dir);
    assert.equal(script, 'const c = 1;\n\nconst b = c + 1;\n\nconsole.log(b, c);\n');
    assert.deepEqual([...names], [['D', 'build.mjs'], ['c', 'c.js'], ['b', 'b.js']]);
  });
  it('reads the names of a destructuring and of functions and classes', () => {
    const dir = pageDir({ 'main.js': "const { x, y: z } = o;\nfunction f() {}\nclass K {}\nlet $ = 1;\n" });
    assert.deepEqual([...bundle(dir).names.keys()], ['D', 'x', 'z', 'f', 'K', '$']);
  });
  it('throws on a cycle and lists the files in it', () => {
    const dir = pageDir({
      'main.js': "import { a } from './a.js';\n",
      'a.js': "import { b } from './b.js';\nexport const a = 1;\n",
      'b.js': "import { a } from './a.js';\nexport const b = 1;\n",
    });
    assert.throws(() => bundle(dir), { message: 'import cycle: a.js -> b.js -> a.js' });
  });
  it('throws on a name two files declare, naming both files', () => {
    const dir = pageDir({
      'main.js': "import { a } from './a.js';\nconst dup = 2;\n",
      'a.js': 'export const a = 1;\nconst dup = 1;\n',
    });
    assert.throws(() => bundle(dir), { message: 'duplicate top-level name "dup": a.js and main.js' });
  });
  it('does not take the data name D for its own', () => {
    assert.throws(() => bundle(pageDir({ 'main.js': 'const D = 1;\n' })), /duplicate top-level name "D"/);
  });
  it('throws on a leftover export default, with file and line', () => {
    const dir = pageDir({ 'main.js': "import { a } from './a.js';\n", 'a.js': 'export const a = 1;\nexport default a;\n' });
    assert.throws(() => bundle(dir), { message: /^a\.js:2: only "export const/ });
  });
  it('throws on an import it cannot read, with file and line', () => {
    for (const line of ["import x from './a.js';", "import * as x from './a.js';", "import {\n  a,\n} from './a.js';"]) {
      const dir = pageDir({ 'main.js': 'const k = 1;\n' + line + '\n', 'a.js': 'export const a = 1;\n' });
      assert.throws(() => bundle(dir), { message: /^main\.js:2: unsupported import\/export: import/ });
    }
  });
  it('reads every name of a one-line const list and of destructuring defaults', () => {
    const dir = pageDir({ 'main.js': "const a = 1, b = [1, 2], c = 'x, d = 3', e = (p, q) => p;\nconst { f = 1, g: h = 2 } = o;\n" });
    assert.deepEqual([...bundle(dir).names.keys()], ['D', 'a', 'b', 'c', 'e', 'f', 'h']);
  });
  it('throws on a duplicate that is not the first name of its line', () => {
    const dir = pageDir({ 'main.js': "import { a } from './a.js';\nconst x = 1, a2 = 2, dup = 3;\n", 'a.js': 'export const a = 1;\nconst dup = 1;\n' });
    assert.throws(() => bundle(dir), { message: 'duplicate top-level name "dup": a.js and main.js' });
  });
  it('fails on a redeclaration the name scan cannot see, saying the bundle does not parse', () => {
    const dir = pageDir({ 'main.js': 'const { a = [1, 2], b } = o;\nlet b = 1;\n' });
    assert.throws(() => bundle(dir), { message: /^bundle does not parse: .*'b'/ });
  });
  it('fails on a syntax error', () => {
    assert.throws(() => bundle(pageDir({ 'main.js': 'const = ;\n' })), { message: /^bundle does not parse: / });
  });
  it('throws when the script holds </script or <!--', () => {
    for (const text of ["const a = '</script>';\n", 'const a = "</SCRIPT";\n', "const a = '<!--';\n"]) {
      assert.throws(() => bundle(pageDir({ 'main.js': text })), { message: /the script holds "<\/script" or "<!--"/ });
    }
  });
  it('throws on an imported name the file does not declare, with file and line', () => {
    const dir = pageDir({ 'main.js': "const k = 1;\nimport { a, nope } from './a.js';\n", 'a.js': 'export const a = 1;\n' });
    assert.throws(() => bundle(dir), { message: 'main.js:2: "nope" is not declared by a.js' });
  });
  it('throws on a name that another file declares, imported from the wrong file', () => {
    const dir = pageDir({ 'main.js': "import { a } from './b.js';\n", 'a.js': 'export const a = 1;\n', 'b.js': "import { a } from './a.js';\nexport const b = 1;\n" });
    assert.throws(() => bundle(dir), { message: 'main.js:1: "a" is not declared by b.js' });
  });
  it('throws on a missing file', () => {
    assert.throws(() => bundle(pageDir({ 'main.js': "import { a } from './nope.js';\n" })), { code: 'ENOENT' });
  });
});

describe('the real page', () => {
  it('bundles without any import or export line', () => {
    const { script } = bundle();
    assert.ok(script.length > 10000);
    assert.equal(/^\s*(import|export)\b/m.test(script), false);
  });
  it('keeps every page file under about 300 lines', async () => {
    const { readdirSync, readFileSync } = await import('node:fs');
    for (const f of readdirSync(PAGE_DIR)) {
      assert.ok(readFileSync(join(PAGE_DIR, f), 'utf8').split('\n').length <= 300, f);
    }
  });
});

describe('buildPage', () => {
  it('fills the shell with the stylesheets and one script that starts with the data', () => {
    const dir = pageDir({ 'main.js': 'log(D);\n' });
    assert.equal(buildPage({ a: 1 }, { pageDir: dir }), "<s>T|W|(() => {\n'use strict';\nconst D = {\"a\":1};\nlog(D);\n})();\n</s>");
  });
  it('escapes < in the data so that no text can end the script', () => {
    const html = buildPage({ md: '</script><!-- x' }, { pageDir: pageDir({ 'main.js': '' }) });
    assert.ok(html.includes('"\\u003c/script>\\u003c!-- x"'));
    assert.equal(html.split('</script>').length, 1);
  });
  it('does not expand $ patterns of the data, and does not touch markers inside it', () => {
    const html = buildPage({ s: "$& $' /*__THEME__*/" }, { pageDir: pageDir({ 'main.js': '' }) });
    assert.ok(html.includes('"$& $\' /*__THEME__*/"'));
  });
  it('builds from a bundle it is given', () => {
    const bundled = { script: 'X;\n', names: new Map() };
    assert.ok(buildPage({}, { pageDir: pageDir({}), bundled }).includes('const D = {};\nX;\n})();'));
  });
  it('names the marker a shell lacks', () => {
    assert.throws(() => buildPage({}, { pageDir: pageDir({ 'main.js': '', 'shell.html': '<s>/*__THEME__*/</s>' }) }), { message: 'shell.html: /*__TAILWIND__*/ not found' });
  });
  it('builds one page from the real directory', () => {
    const html = buildPage({ questions: [] });
    assert.ok(html.startsWith('<!doctype html>'));
    assert.equal(html.split('<script>\n(() =>').length, 2);
    assert.equal(/\/\*__[A-Z]+__\*\//.test(html), false);
  });
});
