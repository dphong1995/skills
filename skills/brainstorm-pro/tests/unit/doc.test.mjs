// sections and reviewRound, no browser.
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { outDir } from '../harness/serve.mjs';
import { reviewRound, sections } from '../../assets/lib/doc.mjs';

describe('sections', () => {
  it('keys ## by its text and ### by "## › ###", with the text up to the next heading', () => {
    const map = sections('# Title\nintro\n## A\na1\n### B\nb1\nb2\n## C\n');
    assert.deepEqual([...map], [['A', 'a1'], ['A › B', 'b1\nb2'], ['C', '']]);
  });
  it('ignores headings inside code fences, of either kind', () => {
    const md = '## A\n```md\n## not\n```\n~~~\n### nor\n~~~\nafter\n## B';
    assert.deepEqual([...sections(md).keys()], ['A', 'B']);
    assert.ok(sections(md).get('A').includes('after'));
  });
  it('closes a fence only with its own marker character', () => {
    const md = '## A\n```\n~~~\n## in\n```\n## B';
    assert.deepEqual([...sections(md).keys()], ['A', 'B']);
  });
  it('drops markup and a closing #s from the key', () => {
    assert.deepEqual([...sections('## `code` and **bold** ##\n### sub').keys()], ['code and bold', 'code and bold › sub']);
  });
  it('keys a ### before any ## by its own text', () => {
    assert.deepEqual([...sections('### lone\ntext').keys()], ['lone']);
  });
});

describe('reviewRound', () => {
  const dir = outDir('unit-doc');
  const src = join(dir, 'doc.md');
  const out = join(dir, 'review.html');
  const state = join(dir, '.review-state.json');
  const v1 = '## Keep\nsame\n## Edit\nold\n## Drop\ngone\n';
  const v2 = '## Keep\nsame\n## Edit\nnew\n## Add\nfresh\n';

  it('counts the rounds and names what changed, was added and removed', () => {
    const first = reviewRound(out, src, v1);
    assert.deepEqual(first.changes, { round: 1 });
    assert.equal(existsSync(state), false);
    first.save();
    assert.deepEqual(JSON.parse(readFileSync(state, 'utf8')), { [resolve(src)]: { round: 1, md: v1 } });

    const second = reviewRound(out, src, v2);
    assert.deepEqual(second.changes, { round: 2, changed: ['Edit'], added: ['Add'], removed: ['Drop'] });
    second.save();
    assert.equal(reviewRound(out, src, v2).changes.round, 3);
  });
  it('keeps a state per source', () => {
    reviewRound(out, join(dir, 'other.md'), v1).save();
    assert.equal(reviewRound(out, join(dir, 'other.md'), v1).changes.round, 2);
    assert.equal(Object.keys(JSON.parse(readFileSync(state, 'utf8'))).length, 2);
  });
  it('starts again from a state file it cannot read', () => {
    writeFileSync(state, 'not json');
    assert.deepEqual(reviewRound(out, src, v1).changes, { round: 1 });
  });
});
