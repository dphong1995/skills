// validateRound and the embedding of files, no browser.
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { mkdirSync, writeFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { join, relative } from 'node:path';
import { outDir } from '../harness/serve.mjs';
import { createEmbed } from '../../assets/lib/embed.mjs';
import { validateRound } from '../../assets/lib/round.mjs';

const q = (extra) => ({ id: 'q1', n: 1, title: 'T', options: [{ id: 'a', label: 'A' }, { id: 'b', label: 'B', recommended: true }], ...extra });
const bad = (data, message) => assert.throws(() => validateRound(data, 'r.json'), { message });

describe('validateRound', () => {
  it('accepts a good round', () => {
    validateRound({ questions: [q(), q({ recommendation: 'B' })] }, 'r.json');
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
  it('allows one recommended option, not two', () => {
    const two = q({ options: [{ id: 'a', recommended: true }, { id: 'b', recommended: true }] });
    bad({ questions: [two] }, 'r.json: questions[0] marks 2 options "recommended"; mark one');
  });
  it('wants a recommended option for a recommendation', () => {
    const none = q({ recommendation: 'A', options: [{ id: 'a' }] });
    bad({ questions: [none] }, 'r.json: questions[0] has "recommendation" but no option with "recommended": true');
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
