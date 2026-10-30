import { esc } from './strings.js';
import { el } from './dom.js';

// Code: highlight.js from the CDN (plain text when it did not load), one block per line so lines can be numbered and tinted.
export const LH = 19, PAD = 8;
const AUTO = ['typescript', 'javascript', 'json', 'sql', 'bash', 'python', 'css', 'scss', 'xml', 'yaml'];
const TINT = { mark: 'bg-mark', add: 'bg-add', del: 'bg-del', pad: 'bg-[repeating-linear-gradient(135deg,transparent_0_5px,var(--color-base-300)_5px_6px)]' };
const highlight = (text, lang) => {
  const h = window.hljs;
  if (!h) return null;
  try {
    if (lang && h.getLanguage(lang)) return h.highlight(text, { language: lang, ignoreIllegals: true }).value;
    const auto = h.highlightAuto(text, AUTO);
    return auto.relevance >= 5 ? auto.value : null;
  } catch (e) { return null; }
};
// Splits highlighted HTML into lines, closing the spans still open at each line end and reopening them on the next line.
export const splitLines = (html) => {
  const lines = [];
  let open = [], cur = '';
  for (const part of html.split(/(<span[^>]*>|<\/span>|\n)/)) {
    if (!part) continue;
    if (part === '\n') { lines.push(cur + '</span>'.repeat(open.length)); cur = open.join(''); }
    else if (part.startsWith('<span')) { open.push(part); cur += part; }
    else if (part === '</span>') { open.pop(); cur += part; }
    else cur += part;
  }
  lines.push(cur + '</span>'.repeat(open.length));
  return lines;
};
export const hlLines = (text, lang) => {
  const html = highlight(text, lang);
  return html == null ? text.split('\n').map(esc) : splitLines(html);
};
export const linesView = (lines, nums, cls) => {
  const row = (html, i, px) => '<span class="block min-h-[19px] ' + px + (cls && cls[i] ? ' ' + TINT[cls[i]] : '') + '">' + html + '</span>';
  const PRE = 'm-0 py-2 font-mono text-[12.5px] leading-[19px] [tab-size:2]';
  const src = el('pre', { class: PRE + ' flex-[1_0_auto]' });
  src.innerHTML = '<code class="block">' + lines.map((l, i) => row(l, i, 'px-2.5')).join('') + '</code>';
  let gutter = null;
  if (nums) {
    gutter = el('pre', { class: PRE + ' flex-none sticky left-0 z-1 text-right text-muted bg-base-200 border-r border-base-300 select-none', 'aria-hidden': 'true' });
    gutter.innerHTML = nums.map((n, i) => row(n == null ? '' : String(n), i, 'px-2')).join('');
  }
  return el('div', { class: 'flex items-start overflow-x-auto' }, gutter, src);
};
export const extLang = (file) => (/\.([\w-]+)$/.exec(file || '') || [])[1];

// Longest common subsequence of two string arrays as a walk over both: { t: 'same', i, j }, { t: 'del', i } (only in A) or { t: 'add', j } (only in B).
// null when the arrays are too big for the table (the callers then treat everything as changed).
export const lcs = (A, B) => {
  const n = A.length, m = B.length;
  if (n * m > 4e6) return null;
  const dp = Array.from({ length: n + 1 }, () => new Uint32Array(m + 1));
  for (let i = n - 1; i >= 0; i--) for (let j = m - 1; j >= 0; j--) dp[i][j] = A[i] === B[j] ? dp[i + 1][j + 1] + 1 : Math.max(dp[i + 1][j], dp[i][j + 1]);
  const walk = [];
  let i = 0, j = 0;
  while (i < n || j < m) {
    if (i < n && j < m && A[i] === B[j]) { walk.push({ t: 'same', i, j }); i++; j++; }
    else if (j >= m || (i < n && dp[i + 1][j] >= dp[i][j + 1])) walk.push({ t: 'del', i: i++ });
    else walk.push({ t: 'add', j: j++ });
  }
  return walk;
};

// Before/after: rows line up on the longest common run of lines; removed and added lines are tinted, gaps hatched.
export const alignRows = (a, b) => {
  const n = a.length, m = b.length;
  const A = a.map((s) => s.trimEnd()), B = b.map((s) => s.trimEnd());
  const rows = [];
  const walk = lcs(A, B);
  if (!walk) {
    for (let k = 0; k < Math.max(n, m); k++) rows.push({ a: k < n ? k : null, b: k < m ? k : null, same: A[k] === B[k] });
    return rows;
  }
  let dels = [], adds = [];
  const flush = () => {
    for (let k = 0; k < Math.max(dels.length, adds.length); k++) rows.push({ a: dels[k] ?? null, b: adds[k] ?? null, same: false });
    dels = []; adds = [];
  };
  for (const step of walk) {
    if (step.t === 'del') dels.push(step.i);
    else if (step.t === 'add') adds.push(step.j);
    else { flush(); rows.push({ a: step.i, b: step.j, same: true }); }
  }
  flush();
  return rows;
};
