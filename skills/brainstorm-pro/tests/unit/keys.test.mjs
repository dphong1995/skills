// assets/page/keys.js, the pure keyboard rules, imported straight from the page folder.
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { isTyping, keyName, nextWhere, step } from '../../assets/page/keys.js';

const ev = (o) => ({ code: '', key: '', keyCode: 0, altKey: false, shiftKey: false, metaKey: false, ctrlKey: false, isComposing: false, ...o });

describe('keyName', () => {
  it('reads letters and digits from the physical key, whatever character it types', () => {
    assert.equal(keyName(ev({ code: 'KeyJ', key: 'j' })), 'j');
    assert.equal(keyName(ev({ code: 'KeyJ', key: 'ợ' })), 'j');
    assert.equal(keyName(ev({ code: 'Digit1', key: '1' })), '1');
    assert.equal(keyName(ev({ code: 'Digit1', key: '&' })), '1');
    assert.equal(keyName(ev({ code: 'Numpad7', key: '7' })), '7');
  });
  it('reads ?, Enter and Escape from e.key', () => {
    assert.equal(keyName(ev({ code: 'Slash', key: '?', shiftKey: true })), '?');
    assert.equal(keyName(ev({ code: 'Slash', key: '?' })), '?');
    assert.equal(keyName(ev({ code: 'Enter', key: 'Enter' })), 'Enter');
    assert.equal(keyName(ev({ code: 'NumpadEnter', key: 'Enter' })), 'Enter');
    assert.equal(keyName(ev({ code: 'Escape', key: 'Escape' })), 'Escape');
  });
  it('reads ? from e.key before the physical key, as AZERTY and Canadian layouts type it on a letter or digit key', () => {
    assert.equal(keyName(ev({ code: 'KeyM', key: '?', shiftKey: true })), '?');
    assert.equal(keyName(ev({ code: 'Digit6', key: '?', shiftKey: true })), '?');
    assert.equal(keyName(ev({ code: 'Comma', key: '?', shiftKey: true })), '?');
  });
  it('counts a numpad key as a digit only while it types one (NumLock off sends End, ArrowDown, ...)', () => {
    assert.equal(keyName(ev({ code: 'Numpad1', key: 'End' })), null);
    assert.equal(keyName(ev({ code: 'Numpad2', key: 'ArrowDown' })), null);
    assert.equal(keyName(ev({ code: 'Numpad5', key: 'Clear' })), null);
    assert.equal(keyName(ev({ code: 'Numpad3', key: '3' })), '3');
  });
  it('prefixes mod+ for Cmd or Ctrl, then shift+', () => {
    assert.equal(keyName(ev({ code: 'Enter', key: 'Enter', metaKey: true })), 'mod+Enter');
    assert.equal(keyName(ev({ code: 'Enter', key: 'Enter', ctrlKey: true })), 'mod+Enter');
    assert.equal(keyName(ev({ code: 'Enter', key: 'Enter', metaKey: true, shiftKey: true })), 'mod+shift+Enter');
    assert.equal(keyName(ev({ code: 'Enter', key: 'Enter', shiftKey: true })), 'shift+Enter');
    assert.equal(keyName(ev({ code: 'KeyC', key: 'c', metaKey: true })), 'mod+c');
    assert.equal(keyName(ev({ code: 'Slash', key: '?', ctrlKey: true, shiftKey: true })), 'mod+?');
  });
  it('is null with Alt or Option, so a dead key or a typed symbol never runs a shortcut', () => {
    assert.equal(keyName(ev({ code: 'KeyJ', key: '∆', altKey: true })), null);
    assert.equal(keyName(ev({ code: 'Enter', key: 'Enter', altKey: true, metaKey: true })), null);
    assert.equal(keyName(ev({ code: 'Digit1', key: '¡', altKey: true })), null);
  });
  it('is null for Shift with a letter or a digit, so Shift+arrow-style selecting and capitals pass through', () => {
    assert.equal(keyName(ev({ code: 'KeyJ', key: 'J', shiftKey: true })), null);
    assert.equal(keyName(ev({ code: 'Digit1', key: '!', shiftKey: true })), null);
    assert.equal(keyName(ev({ code: 'KeyJ', key: 'J', shiftKey: true, metaKey: true })), null);
  });
  it('is null while composing and for keyCode 229', () => {
    assert.equal(keyName(ev({ code: 'KeyJ', key: 'j', isComposing: true })), null);
    assert.equal(keyName(ev({ code: 'KeyJ', key: 'Process', keyCode: 229 })), null);
    assert.equal(keyName(ev({ code: 'Enter', key: 'Enter', isComposing: true, metaKey: true })), null);
  });
  it('is null for keys the shortcuts do not use', () => {
    assert.equal(keyName(ev({ code: 'ArrowDown', key: 'ArrowDown' })), null);
    assert.equal(keyName(ev({ code: 'Space', key: ' ' })), null);
    assert.equal(keyName(ev({ code: 'Tab', key: 'Tab' })), null);
    assert.equal(keyName(ev({ code: 'Comma', key: ',' })), null);
    assert.equal(keyName(ev({})), null);
  });
});

describe('isTyping', () => {
  const input = (type) => ({ tagName: 'INPUT', type });
  it('is true for text boxes, selects and editable content', () => {
    assert.equal(isTyping({ tagName: 'TEXTAREA' }), true);
    assert.equal(isTyping({ tagName: 'SELECT' }), true);
    assert.equal(isTyping({ tagName: 'DIV', isContentEditable: true }), true);
    assert.equal(isTyping({ tagName: 'textarea' }), true);
  });
  it('is true for every input type that takes text', () => {
    for (const t of ['text', 'search', 'email', 'url', 'tel', 'password', 'number', 'date', 'time', 'datetime-local', 'month', 'week', 'file', 'color', 'range', 'image']) {
      assert.equal(isTyping(input(t)), true, t);
    }
    assert.equal(isTyping({ tagName: 'INPUT' }), true, 'no type attribute means text');
    assert.equal(isTyping(input('TEXT')), true);
  });
  it('is false for radio, checkbox, button, submit and reset inputs', () => {
    for (const t of ['radio', 'checkbox', 'button', 'submit', 'reset', 'RADIO']) assert.equal(isTyping(input(t)), false, t);
  });
  it('is false for other elements and for nothing', () => {
    assert.equal(isTyping({ tagName: 'BUTTON' }), false);
    assert.equal(isTyping({ tagName: 'A' }), false);
    assert.equal(isTyping({ tagName: 'BODY', isContentEditable: false }), false);
    assert.equal(isTyping(null), false);
    assert.equal(isTyping(undefined), false);
  });
});

describe('step', () => {
  it('moves by d and stops at both ends', () => {
    assert.equal(step(1, 5, 1), 2);
    assert.equal(step(1, 5, -1), 0);
    assert.equal(step(0, 5, -1), 0);
    assert.equal(step(4, 5, 1), 4);
    assert.equal(step(-1, 5, 1), 0);
  });
  it('is -1 for an empty list', () => assert.equal(step(0, 0, 1), -1));
});

describe('nextWhere', () => {
  const list = ['a', 'b', 'c', 'd'];
  it('finds the next match after `from`', () => {
    assert.equal(nextWhere(list, 0, (x) => x === 'c' || x === 'd'), 2);
    assert.equal(nextWhere(list, 2, (x) => x === 'c' || x === 'd'), 3);
  });
  it('wraps to the start, and ends on `from` itself', () => {
    assert.equal(nextWhere(list, 3, (x) => x === 'b'), 1);
    assert.equal(nextWhere(list, 1, (x) => x === 'b'), 1);
  });
  it('starts at the first item from -1', () => assert.equal(nextWhere(list, -1, () => true), 0));
  it('is -1 when nothing matches or the list is empty', () => {
    assert.equal(nextWhere(list, 0, () => false), -1);
    assert.equal(nextWhere([], 0, () => true), -1);
  });
  it('passes the index to the predicate', () => assert.equal(nextWhere(list, 0, (_, i) => i === 3), 3));
});
