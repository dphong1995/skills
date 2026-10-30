// Word diff of two texts for the review page's "Xem chỗ đổi" box: [{ t: 'same' | 'del' | 'add', s }].
// Same + del parts rebuild `a`, same + add parts rebuild `b`.
import { alignRows, lcs } from '../page/code.js';

const TOKEN = /[\p{L}\p{N}_]+|\s+|[^\s\p{L}\p{N}_]/gu;

// Lines first (alignRows), then the words of each changed line pair.
export const wordDiff = (a, b) => {
  const parts = [];
  const push = (t, s) => {
    if (!s) return;
    const last = parts[parts.length - 1];
    if (last && last.t === t) last.s += s;
    else parts.push({ t, s });
  };
  const A = a.split('\n'), B = b.split('\n');
  const withNl = (lines, i) => lines[i] + (i < lines.length - 1 ? '\n' : '');
  for (const row of alignRows(A, B)) {
    const x = row.a == null ? null : withNl(A, row.a);
    const y = row.b == null ? null : withNl(B, row.b);
    if (x === y) push('same', x);
    else if (x == null) push('add', y);
    else if (y == null) push('del', x);
    else wordParts(x, y, push);
  }
  return parts;
};

const wordParts = (x, y, push) => {
  const p = x.match(TOKEN) || [], q = y.match(TOKEN) || [];
  const walk = lcs(p, q);
  if (!walk) {
    push('del', x);
    push('add', y);
    return;
  }
  for (const step of walk) {
    if (step.t === 'same') push('same', p[step.i]);
    else if (step.t === 'del') push('del', p[step.i]);
    else push('add', q[step.j]);
  }
};
