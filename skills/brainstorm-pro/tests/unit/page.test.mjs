// The pure page modules, imported straight from assets/page/.
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { esc, plain, short } from '../../assets/page/strings.js';
import { alignRows, extLang, splitLines } from '../../assets/page/code.js';

describe('strings.js', () => {
  it('esc escapes & < >', () => assert.equal(esc('a<b>&c'), 'a&lt;b&gt;&amp;c'));
  it('plain drops backticks and **', () => {
    assert.equal(plain('a `b` **c**'), 'a b c');
    assert.equal(plain(null), '');
    assert.equal(plain(7), '7');
  });
  describe('short', () => {
    it('collapses white space and keeps up to 100 characters', () => {
      assert.equal(short('  a \n b\t c '), 'a b c');
      const hundred = 'x'.repeat(100);
      assert.equal(short(hundred), hundred);
    });
    it('cuts a longer text to its start and its end at a word boundary', () => {
      const words = Array.from({ length: 40 }, (_, i) => 'word' + i).join(' ');
      const s = short(words);
      assert.ok(s.length < 100);
      assert.ok(s.startsWith('word0 word1') && s.includes(' … ') && s.endsWith('word39'));
      assert.ok(!s.startsWith(' ') && !s.endsWith(' '));
    });
  });
});

describe('code.js', () => {
  describe('extLang', () => {
    it('reads the extension', () => {
      assert.equal(extLang('a/b/c.ts'), 'ts');
      assert.equal(extLang('x.test.mjs'), 'mjs');
      assert.equal(extLang('Makefile'), undefined);
      assert.equal(extLang(undefined), undefined);
    });
  });

  describe('splitLines', () => {
    it('gives a plain text one entry per line', () => assert.deepEqual(splitLines('a\nb\n'), ['a', 'b', '']));
    it('closes the spans open at a line end and reopens them on the next line', () => {
      assert.deepEqual(splitLines('<span class="a">x\ny</span>z'), ['<span class="a">x</span>', '<span class="a">y</span>z']);
    });
    it('keeps nested spans in order across several lines', () => {
      assert.deepEqual(splitLines('<span class="a">1<span class="b">2\n3\n4</span>5</span>'), [
        '<span class="a">1<span class="b">2</span></span>',
        '<span class="a"><span class="b">3</span></span>',
        '<span class="a"><span class="b">4</span>5</span>',
      ]);
    });
  });

  describe('alignRows', () => {
    it('pairs equal lines, ignoring trailing white space', () => {
      assert.deepEqual(alignRows(['a', 'b '], ['a ', 'b']), [{ a: 0, b: 0, same: true }, { a: 1, b: 1, same: true }]);
    });
    it('leaves a gap on the old side for an inserted line', () => {
      assert.deepEqual(alignRows(['a', 'c'], ['a', 'b', 'c']), [
        { a: 0, b: 0, same: true }, { a: null, b: 1, same: false }, { a: 1, b: 2, same: true }]);
    });
    it('leaves a gap on the new side for a deleted line', () => {
      assert.deepEqual(alignRows(['a', 'b', 'c'], ['a', 'c']), [
        { a: 0, b: 0, same: true }, { a: 1, b: null, same: false }, { a: 2, b: 1, same: true }]);
    });
    it('puts a replaced line side by side', () => {
      assert.deepEqual(alignRows(['a', 'x', 'c'], ['a', 'y', 'c']), [
        { a: 0, b: 0, same: true }, { a: 1, b: 1, same: false }, { a: 2, b: 2, same: true }]);
    });
    it('handles one side empty', () => {
      assert.deepEqual(alignRows([], ['a']), [{ a: null, b: 0, same: false }]);
      assert.deepEqual(alignRows(['a'], []), [{ a: 0, b: null, same: false }]);
    });
    it('falls back to row by row when the inputs are too large to align', () => {
      const a = Array.from({ length: 2001 }, (_, i) => 'l' + i);
      const b = ['new', ...a.slice(0, 2000)];
      const rows = alignRows(a, b);
      assert.equal(rows.length, 2001);
      assert.deepEqual(rows[0], { a: 0, b: 0, same: false });
      assert.deepEqual(rows[2000], { a: 2000, b: 2000, same: false });
      const short2 = alignRows(a.slice(0, 3), b.slice(0, 3));
      assert.equal(short2.length, 4);
    });
  });
});
