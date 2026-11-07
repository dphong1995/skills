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
  it('reads CRLF like LF, so a CRLF round after an LF round changes nothing', () => {
    const lf = '## A\na1\n### B\nb1\n';
    assert.deepEqual([...sections(lf.replace(/\n/g, '\r\n'))], [...sections(lf)]);
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
    assert.deepEqual(second.changes, { round: 2, changed: ['Edit'], added: ['Add'], removed: ['Drop'], notes: [], diffs: { Edit: [{ t: 'del', s: 'old' }, { t: 'add', s: 'new' }] } });
    second.save();
    assert.equal(reviewRound(out, src, v2).changes.round, 3);
  });
  it('gives round 2 the saved notes and a word diff of each changed section only', () => {
    const d = outDir('unit-doc-notes');
    const o = join(d, 'review.html'), f = join(d, '.review-state.json'), s = join(d, 'doc.md');
    const notes = [{ kind: 'section', key: 'Edit', text: 'why old?' }, { kind: 'general', key: 'General', text: 'ok' }];
    const first = reviewRound(o, s, v1);
    assert.equal(first.changes.notes, undefined);
    first.save();
    first.saveNotes(notes);
    assert.deepEqual(JSON.parse(readFileSync(f, 'utf8'))[resolve(s)], { round: 1, md: v1, notes });
    const second = reviewRound(o, s, v2);
    assert.deepEqual(second.changes.notes, notes);
    assert.deepEqual(Object.keys(second.changes.diffs), ['Edit']);
    assert.deepEqual(second.changes.diffs.Edit, [{ t: 'del', s: 'old' }, { t: 'add', s: 'new' }]);
    second.save();
    assert.equal(JSON.parse(readFileSync(f, 'utf8'))[resolve(s)].notes, undefined, 'round 2 starts without notes');
    assert.deepEqual(reviewRound(o, s, v2).changes.notes, []);
  });
  it('round 2 reads legacy Chung and dòng state', () => {
    const d = outDir('unit-doc-legacy');
    const o = join(d, 'review.html'), s = join(d, 'doc.md');
    const first = reviewRound(o, s, v1);
    first.save();
    first.saveNotes([{ kind: 'general', key: 'Chung', text: 'x' }, { kind: 'passage', key: 'Mục', lines: 'dòng 3-5', quote: 'q', text: 't' }, { kind: 'passage', key: 'Mục', lines: 'dòng 7', quote: 'q', text: 't' }, { kind: 'passage', key: 'Mở đầu', lines: 'dòng 1', quote: 'q', text: 't' }]);
    const notes = reviewRound(o, s, v2).changes.notes;
    assert.deepEqual(notes.map((n) => n.key), ['General', 'Mục', 'Mục', 'Intro']);
    assert.deepEqual(notes.map((n) => n.lines), [undefined, 'lines 3-5', 'lines 7', 'lines 1']);
  });
  it('saveNotes creates a missing state file and keeps the other sources', () => {
    const d = outDir('unit-doc-notes2');
    const o = join(d, 'review.html'), f = join(d, '.review-state.json');
    reviewRound(o, join(d, 'other.md'), v1).save();
    const r = reviewRound(o, join(d, 'a.md'), v1);
    r.saveNotes([{ kind: 'general', key: 'General', text: 'x' }]);
    const all = JSON.parse(readFileSync(f, 'utf8'));
    assert.deepEqual(all[resolve(d, 'a.md')], { round: 1, md: v1, notes: [{ kind: 'general', key: 'General', text: 'x' }] });
    assert.equal(all[resolve(d, 'other.md')].round, 1);
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
