// validateRound and the embedding of files, no browser.
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { mkdirSync, writeFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { join, relative } from 'node:path';
import { outDir, render } from '../harness/serve.mjs';
import { createEmbed, InputError } from '../../assets/lib/embed.mjs';
import { loadRound, validateRound } from '../../assets/lib/round.mjs';

const q = (extra) => ({ id: 'q1', n: 1, title: 'T', options: [{ id: 'a', label: 'A' }, { id: 'b', label: 'B' }], recommendation: { option: 'b', why: 'Smallest.' }, ...extra });
const bad = (data, message) => assert.throws(() => validateRound(data, 'r.json'), message instanceof RegExp ? { message } : { message });

describe('validateRound', () => {
  it('accepts a good round', () => {
    validateRound({ questions: [q(), q({ recommendation: { option: 'a', why: 'A' } })] }, 'r.json');
  });
  it('wants a non-empty questions array', () => {
    bad({}, 'r.json: "questions" must be a non-empty array');
    bad({ questions: [] }, 'r.json: "questions" must be a non-empty array');
    bad({ questions: {} }, 'r.json: "questions" must be a non-empty array');
  });
  it('names the missing field and the question', () => {
    for (const key of ['id', 'n', 'title', 'options']) {
      const x = q();
      delete x[key];
      bad({ questions: [q(), x] }, `r.json: questions[1] is missing "${key}"`);
    }
  });
  it('refuses the old "recommended" flag and points at the new form', () => {
    const old = q({ recommendation: undefined, options: [{ id: 'a' }, { id: 'b', recommended: true }] });
    bad({ questions: [old] }, 'r.json: questions[0].options[1] (b) has "recommended"; name the pick in the question\'s "recommendation": { "option": "b", "why": "…" }');
  });
  it('hints at a field whose two adjacent letters are swapped', () => {
    bad({ questions: [q({ titel: 'x' })] }, 'r.json: questions[0] (q1) has unknown field "titel"; did you mean "title"?');
  });
  it('refuses an unknown field with a hint when one is close', () => {
    bad({ questions: [q({ recomendation: { option: 'a', why: 'x' } })] }, 'r.json: questions[0] (q1) has unknown field "recomendation"; did you mean "recommendation"?');
    bad({ questions: [q({ zzzzzz: 1 })] }, /unknown field "zzzzzz"; allowed: id, n,/);
    bad({ topc: 'x', questions: [q()] }, 'r.json: the round has unknown field "topc"; did you mean "topic"?');
    bad({ questions: [q({ options: [{ id: 'a', mean: 'x' }] })] }, 'r.json: questions[0].options[0] (a) has unknown field "mean"; did you mean "means"?');
    bad({ questions: [q({ recommendation: { option: 'a', why: 'x', whyy: 1 } })] }, /"recommendation" has unknown field "whyy"; did you mean "why"\?/);
  });
  it('offers a hint only for a close name, else lists the allowed ones', () => {
    bad({ questions: [q({ options: [{ id: 'a', note: 'x' }] })] }, /unknown field "note"; allowed: id, label,/);
    bad({ questions: [q({ recomendation: { option: 'a', why: 'x' } })] }, /did you mean "recommendation"\?/);
  });
  it('refuses a wrong-typed field in one line', () => {
    const ctx = (b) => ({ questions: [q({ context: [b] })] });
    bad({ questions: [q({ context: {} })] }, 'r.json: questions[0] (q1) "context" must be an array');
    bad({ decided: 'x', questions: [q()] }, 'r.json: "decided" must be an array');
    bad({ questions: [q({ options: [{ id: 'a', pros: 'x' }] })] }, /options\[0\] \(a\) "pros" must be an array/);
    bad({ questions: [q({ options: [{ id: 'a', cons: {} }] })] }, /"cons" must be an array/);
    bad(ctx({ t: 'list', items: 'x' }), /"items" must be an array/);
    bad(ctx({ t: 'table', head: 'x', rows: [] }), /"head" must be an array/);
    bad(ctx({ t: 'table', head: [], rows: 'x' }), /"rows" must be an array/);
    bad(ctx({ t: 'table', head: [], rows: ['x'] }), /rows\[0\] must be an array/);
    bad(ctx({ t: 'code', file: '/a', lines: '1-5' }), /context\[0\] "lines" must be \[from, to\] numbers/);
    bad(ctx({ t: 'diff', before: { file: '/a', lines: [1] } }), /before "lines" must be \[from, to\] numbers/);
    bad({ questions: [q({ options: [{ id: 'a', code: { file: '/a', lines: 'x' } }] })] }, /code "lines" must be/);
  });
  it('the CLI prints one line and exits 1 for a wrong-typed field', () => {
    const dir = outDir('unit-wrong-type');
    const f = join(dir, 'round.json');
    writeFileSync(f, JSON.stringify({ questions: [q({ context: {} })] }));
    for (const body of [{ questions: [q({ context: {} })] }, { questions: [q({ context: [{ t: 'code', file: '/a', lines: '1-5' }] })] }]) {
      writeFileSync(f, JSON.stringify(body));
      const r = render(f, join(dir, 'out.html'));
      assert.equal(r.code, 1);
      assert.equal(r.stderr.trim().split('\n').length, 1, r.stderr);
      assert.ok(!/\bat /.test(r.stderr), r.stderr);
    }
  });
  it('checks the fields of each block type and of code sides', () => {
    const ctx = (b) => ({ questions: [q({ context: [b] })] });
    bad(ctx({ t: 'p', txt: 'x' }), 'r.json: questions[0].context[0] (p) has unknown field "txt"; did you mean "text"?');
    bad(ctx({ t: 'code', file: '/a', lins: [1, 2] }), /\(code\) has unknown field "lins"; did you mean "lines"\?/);
    bad(ctx({ t: 'diff', before: { text: 'a', lng: 'ts' } }), /context\[0\]\.before has unknown field "lng"; did you mean "lang"\?/);
    bad({ questions: [q({ options: [{ id: 'a', code: { file: '/a', x: 1 } }] })] }, /options\[0\] \(a\)\.code has unknown field "x"/);
    validateRound(ctx({ t: 'diff', label: 'L', before: { text: 'a', file: '/a', lines: [1, 1], label: 'x', lang: 'ts' }, after: { text: 'b' } }), 'r.json');
  });
  it('refuses an unknown block type and lists the valid ones', () => {
    bad({ questions: [q({ context: [{ t: 'quote', text: 'x' }] })] }, 'r.json: questions[0].context[0] has unknown block type "quote"; valid types: p, note, list, code, diff, mermaid, img, mockup, table');
    bad({ questions: [q({ context: [{ text: 'x' }] })] }, /unknown block type undefined/);
  });
  it('wants a recommendation with a string option and why', () => {
    bad({ questions: [q({ recommendation: 'B' })] }, /"recommendation" must be an object/);
    bad({ questions: [q({ recommendation: { option: 'a' } })] }, 'r.json: questions[0] (q1) "recommendation" needs a string "why"');
    bad({ questions: [q({ recommendation: { why: 'x' } })] }, /needs a string "option"/);
  });
  it('wants the recommended option to be an option id', () => {
    bad({ questions: [q({ recommendation: { option: 'z', why: 'x' } })] }, 'r.json: questions[0] (q1) "recommendation" option "z" is not an option id; ids: a, b');
  });
});

describe('loadRound', () => {
  const dir = outDir('unit-load-round');
  it('refuses invalid JSON and a missing file as input errors', () => {
    const file = join(dir, 'bad.json');
    writeFileSync(file, '{ "questions": [');
    assert.throws(() => loadRound(file), (e) => e instanceof InputError && e.message.startsWith(file + ': not valid JSON: ') && !e.message.includes('\n'));
    assert.throws(() => loadRound(join(dir, 'none.json')), (e) => e instanceof InputError && /cannot read/.test(e.message));
  });
  it('refuses a code file that cannot be read', () => {
    const file = join(dir, 'nofile.json');
    const missing = join(dir, 'missing.ts');
    writeFileSync(file, JSON.stringify({ root: dir, questions: [q({ context: [{ t: 'code', file: missing, text: 'x' }] })] }));
    assert.throws(() => loadRound(file), { message: `${file}: cannot read ${missing} (ENOENT)` });
  });
  it('links the recommendation text', () => {
    mkdirSync(join(dir, 'sub'), { recursive: true });
    writeFileSync(join(dir, 'sub', 'a.txt'), 'A');
    const file = join(dir, 'link.json');
    writeFileSync(file, JSON.stringify({ root: dir, questions: [q({ recommendation: { option: 'a', why: 'see `sub/a.txt`' } })] }));
    assert.deepEqual(loadRound(file).paths, { 'sub/a.txt': join(dir, 'sub', 'a.txt') });
  });
});

describe('createEmbed', () => {
  const dir = outDir('unit-embed');
  const text = Array.from({ length: 10 }, (_, i) => 'line ' + (i + 1)).join('\n') + '\n';
  const file = join(dir, 'ten.txt');
  writeFileSync(file, text);
  const embed = () => createEmbed({ root: dir, src: 'r.json' });
  const quiet = (fn) => {
    const log = console.error;
    const lines = [];
    console.error = (l) => lines.push(l);
    try { fn(); } finally { console.error = log; }
    return lines;
  };

  it('wants an absolute root', () => {
    assert.throws(() => createEmbed({ root: 'rel', src: 'r.json' }), { message: 'r.json: "root" must be an absolute path: rel' });
  });

  describe('prep', () => {
    it('cuts the lines and builds the label', () => {
      const e = embed();
      const b = { file, lines: ['3', 5] };
      e.prep(b);
      assert.deepEqual(b.lines, [3, 5]);
      assert.equal(b.text, 'line 3\nline 4\nline 5');
      assert.equal(b.label, file.replace(homedir(), '~') + ':3-5');
      assert.equal(e.files[file], text);
    });
    it('keeps its own text and label', () => {
      const e = embed();
      const b = { file, text: 'mine', label: 'L' };
      e.prep(b);
      assert.equal(b.text, 'mine');
      assert.equal(b.label, 'L');
    });
    it('rejects what cannot be shown', () => {
      const e = embed();
      assert.throws(() => e.prep({ file: 'rel.txt' }), { message: 'r.json: code "file" must be an absolute path: rel.txt' });
      assert.throws(() => e.prep({ file, lines: [4, 11] }), { message: `r.json: lines 4-11 are outside ${file} (10 lines)` });
      assert.throws(() => e.prep({ file, lines: [5, 4] }), /are outside/);
      assert.throws(() => e.prep({ file }), { message: `r.json: code block for ${file} needs "lines" or "text"` });
    });
    it('ignores a block without a file', () => {
      embed().prep({ text: 'x' });
      embed().prep(null);
    });
  });

  describe('link', () => {
    mkdirSync(join(dir, 'sub'), { recursive: true });
    writeFileSync(join(dir, 'sub', 'a.txt'), 'A');
    it('links `dir/file`, with or without a line range', () => {
      const e = embed();
      e.link('see `sub/a.txt:2-3` and `sub/a.txt`');
      assert.deepEqual(e.paths, { 'sub/a.txt': join(dir, 'sub', 'a.txt') });
      assert.equal(e.files[join(dir, 'sub', 'a.txt')], 'A');
    });
    it('needs a directory in the path, a file on disk and no spaces', () => {
      const e = embed();
      e.link('`a.txt` `sub/missing.txt` `sub` `sub/a.txt x`');
      assert.deepEqual(e.paths, {});
      e.link(42);
      assert.deepEqual(e.files, {});
    });
    it('resolves ~/ against the home directory', () => {
      const e = embed();
      e.link('`~/' + relative(homedir(), join(dir, 'sub', 'a.txt')) + '`');
      assert.equal(Object.keys(e.paths).length, 1);
      assert.equal(Object.values(e.paths)[0], join(dir, 'sub', 'a.txt'));
    });
    it('leaves a file over 512 KB as plain text', () => {
      writeFileSync(join(dir, 'sub', 'big.txt'), 'x'.repeat(512 * 1024 + 1));
      const e = embed();
      const lines = quiet(() => e.link('`sub/big.txt`'));
      assert.deepEqual(e.paths, {});
      assert.deepEqual(lines, ['sub/big.txt: over 512 KB, left as plain text']);
    });
  });
});
