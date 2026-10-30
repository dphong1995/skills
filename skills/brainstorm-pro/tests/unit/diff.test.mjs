// wordDiff, no browser.
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { wordDiff } from '../../assets/lib/diff.mjs';

const join = (parts, ...types) => parts.filter((p) => types.includes(p.t)).map((p) => p.s).join('');

describe('wordDiff', () => {
  it('gives one same part for identical texts', () => {
    assert.deepEqual(wordDiff('a b\nc', 'a b\nc'), [{ t: 'same', s: 'a b\nc' }]);
  });
  it('marks a word changed mid-line and merges neighbours of one type', () => {
    assert.deepEqual(wordDiff('the old cat sat', 'the new cat sat'), [
      { t: 'same', s: 'the ' }, { t: 'del', s: 'old' }, { t: 'add', s: 'new' }, { t: 'same', s: ' cat sat' }]);
  });
  it('marks a word added at the end', () => {
    assert.deepEqual(wordDiff('one two', 'one two three'), [{ t: 'same', s: 'one two' }, { t: 'add', s: ' three' }]);
  });
  it('marks an inserted line and a deleted line whole, keeping the newline', () => {
    assert.deepEqual(wordDiff('a\nc', 'a\nb\nc'), [{ t: 'same', s: 'a\n' }, { t: 'add', s: 'b\n' }, { t: 'same', s: 'c' }]);
    assert.deepEqual(wordDiff('a\nb\nc', 'a\nc'), [{ t: 'same', s: 'a\n' }, { t: 'del', s: 'b\n' }, { t: 'same', s: 'c' }]);
  });
  it('keeps Vietnamese words with diacritics whole', () => {
    const parts = wordDiff('Người dùng chọn phương án', 'Người dùng chọn phương án khác');
    assert.deepEqual(parts, [{ t: 'same', s: 'Người dùng chọn phương án' }, { t: 'add', s: ' khác' }]);
    const w = wordDiff('đã duyệt xong', 'đã duyệt lại');
    assert.deepEqual(w.filter((p) => p.t !== 'same'), [{ t: 'del', s: 'xong' }, { t: 'add', s: 'lại' }]);
  });
  it('rebuilds both texts from its parts', () => {
    const pairs = [['', ''], ['', 'x\ny'], ['x\ny', ''], ['a  b \n c', 'a b\n c d'], ['one\ntwo\nthree\n', 'one\nthree\nfour'],
      ['Tiếng Việt có dấu, đủ cả.\nHai', 'Tiếng Việt có dấu.\nHai\nBa'], ['trailing \nx', 'trailing\nx'], ['a\n\n\nb', 'a\nb']];
    for (const [a, b] of pairs) {
      const parts = wordDiff(a, b);
      assert.equal(join(parts, 'same', 'del'), a, JSON.stringify([a, b]));
      assert.equal(join(parts, 'same', 'add'), b, JSON.stringify([a, b]));
      for (let i = 1; i < parts.length; i++) assert.notEqual(parts[i].t, parts[i - 1].t, 'adjacent parts merged');
    }
  });
  it('falls back to whole-line parts for a line pair too long for the word LCS', () => {
    const a = Array.from({ length: 2100 }, (_, i) => 'w' + i).join(' ');
    const b = a.replace('w5 ', 'x5 ');
    assert.deepEqual(wordDiff(a, b), [{ t: 'del', s: a }, { t: 'add', s: b }]);
  });
});
