import { $, el, TEXTAREA } from './dom.js';

export const CARD = 'card card-border border-base-300 bg-base-100 px-[18px] py-4 max-[520px]:px-3 max-[520px]:py-3.5 grid gap-3 min-w-0 transition-[border-color,box-shadow] duration-200 motion-reduce:transition-none [&.attention]:border-[var(--color-warning)] [&.attention]:[box-shadow:0_0_0_3px_color-mix(in_oklab,var(--color-warning)_22%,transparent)]';
export const main = $('#main');
export const fit = (ta) => {
  ta.style.height = 'auto';
  if (ta.value && ta.offsetParent) ta.style.height = ta.scrollHeight + 2 + 'px';
};

// The floating bar's message, copy fallback and drafts, used by both modes. Served by render-interview.mjs --serve, a page sends its text to Claude itself; opened as a file, it copies the text for pasting.
export const sendMode = !!(D && D.send);
export const fabMsg = el('p', { class: 'alert alert-soft block max-w-[24rem] text-[13px] leading-normal px-2.5 py-1.5 shadow-lg empty:hidden', role: 'status' });
export const out = el('textarea', { id: 'out', class: TEXTAREA + ' w-[min(26rem,calc(100vw-32px))] font-mono text-[12px] max-h-[40vh] bg-base-200', readonly: true, hidden: true, 'aria-label': 'Nội dung để dán vào Claude' });
export const say = (kind, ...parts) => {
  if (!kind && !parts.length && !fabMsg.dataset.kind && !fabMsg.firstChild) return;
  fabMsg.dataset.kind = kind || '';
  fabMsg.classList.toggle('alert-success', kind === 'ok');
  fabMsg.classList.toggle('alert-warning', kind === 'warn');
  fabMsg.replaceChildren(...parts);
};
export const copyText = async (text) => {
  out.value = text;
  let ok = false;
  // writeText can wait forever for a permission the browser never asks for, so it gets two seconds.
  try { await Promise.race([navigator.clipboard.writeText(text), new Promise((_, no) => setTimeout(() => no(new Error('timeout')), 2000))]); ok = true; }
  catch (e) { out.hidden = false; out.focus(); out.select(); try { ok = document.execCommand('copy'); } catch (e2) { ok = false; } }
  out.hidden = ok;
  return ok;
};
export const postText = async (text) => {
  const r = await fetch(D.send, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ text }) });
  if (!r.ok) throw new Error('HTTP ' + r.status);
};
// Drafts are kept in this browser per page file, so a reload or a closed tab loses nothing.
export const storage = (key) => ({
  get: () => { try { return JSON.parse(localStorage.getItem(key) || 'null'); } catch (e) { return null; } },
  set: (v) => { try { if (v) localStorage.setItem(key, JSON.stringify(v)); else localStorage.removeItem(key); } catch (e) { /* storage blocked */ } },
});
