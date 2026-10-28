// Text helpers with no DOM, so Node can import them.
export const esc = (s) => s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
export const PATH = /^(.+?)(?::(\d+)(?:-(\d+))?)?$/;
// The same text without the markup, for a label drawn by CSS.
export const plain = (text) => String(text ?? '').replace(/`|\*\*/g, '');
export const short = (q) => {
  q = q.replace(/\s+/g, ' ').trim();
  return q.length <= 100 ? q : q.slice(0, 60).replace(/\s+\S*$/, '') + ' … ' + q.slice(-30).replace(/^\S*\s+/, '');
};
