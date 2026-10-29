// The keyboard rules with no DOM, so Node can import them.
const KEY_CODE = /^(?:Key([A-Z])|Digit(\d)|Numpad(\d))$/;

// 'j', '1', '?', 'Escape', 'mod+Enter', 'mod+shift+Enter'; null for an event the shortcuts must leave alone (Alt, Shift with a letter or digit, composition).
// Letters and digits come from e.code, so the input method does not change them; a numpad digit counts only while it types one (NumLock on). ? comes first: on AZERTY and Canadian layouts it sits on a letter or digit key.
export const keyName = (e) => {
  if (e.isComposing || e.keyCode === 229 || e.altKey) return null;
  const m = KEY_CODE.exec(e.code || '');
  let k;
  if (e.key === '?') k = '?';
  else if (m) {
    if (e.shiftKey || (m[3] != null && !/^\d$/.test(e.key))) return null;
    k = (m[1] || m[2] || m[3]).toLowerCase();
  }
  else if (e.key === 'Enter' || e.key === 'Escape') k = e.key;
  else return null;
  return (e.metaKey || e.ctrlKey ? 'mod+' : '') + (e.shiftKey && k !== '?' ? 'shift+' : '') + k;
};

// True when typing would go into the element; el may be a plain { tagName, type, isContentEditable }. Self-contained: the test harness injects its source into the page.
export const isTyping = (el) => {
  if (!el) return false;
  const tag = String(el.tagName || '').toUpperCase();
  if (tag === 'TEXTAREA' || tag === 'SELECT' || el.isContentEditable) return true;
  return tag === 'INPUT' && !/^(radio|checkbox|button|submit|reset)$/.test(String(el.type || 'text').toLowerCase());
};

// The index d places from i, held inside 0..n-1; -1 when there is nothing.
export const step = (i, n, d) => (n ? Math.min(n - 1, Math.max(0, i + d)) : -1);

// The first index after `from` (wrapping, `from` itself last) whose item passes; -1 when none does.
export const nextWhere = (list, from, pred) => {
  const n = list.length;
  for (let k = 1; k <= n; k++) {
    const i = (((from + k) % n) + n) % n;
    if (pred(list[i], i)) return i;
  }
  return -1;
};
