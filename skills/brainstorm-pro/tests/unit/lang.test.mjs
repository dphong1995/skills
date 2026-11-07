import { afterEach, describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { pageLang } from '../../assets/lib/lang.mjs';
import { STRINGS, setLang, t } from '../../assets/page/i18n.js';

const homes = [];
const home = (settings) => {
  const dir = mkdtempSync(join(tmpdir(), 'bp-lang-'));
  homes.push(dir);
  if (settings !== undefined) writeFileSync(join(dir, 'settings.json'), typeof settings === 'string' ? settings : JSON.stringify(settings));
  return { CLAUDE_HOME: dir };
};
afterEach(() => { for (const d of homes.splice(0)) rmSync(d, { recursive: true, force: true }); });

describe('pageLang', () => {
  it('reads Vietnamese from the language setting, with or without case and diacritics', () => {
    for (const language of ['vietnamese', 'Vietnamese', 'vi', 'tiếng Việt', 'Tiếng Việt']) {
      assert.equal(pageLang({ env: home({ language }) }), 'vi', language);
    }
  });
  it('gives English for another language, no key, invalid JSON or no file', () => {
    for (const settings of [{ language: 'english' }, { language: 'japanese' }, { theme: 'dark' }, { language: 7 }, '{ not json', undefined]) {
      assert.equal(pageLang({ env: home(settings) }), 'en', JSON.stringify(settings));
    }
  });
  it('lets --lang win over the file', () => {
    assert.equal(pageLang({ flag: 'en', env: home({ language: 'vietnamese' }) }), 'en');
    assert.equal(pageLang({ flag: 'vi', env: home() }), 'vi');
  });
});

describe('page strings', () => {
  it('have the same keys in both languages', () => {
    assert.deepEqual(Object.keys(STRINGS.vi).sort(), Object.keys(STRINGS.en).sort());
  });
  it('t() picks the language, calls function values and throws on an unknown key', () => {
    setLang('en');
    assert.equal(t('close'), 'Close');
    assert.equal(t('noteCount', 2), '2 notes');
    setLang('vi');
    assert.equal(t('close'), 'Đóng');
    assert.equal(t('noteCount', 2), '2 góp ý');
    assert.throws(() => t('nope'), /unknown string key/);
    setLang(undefined);
  });
});
