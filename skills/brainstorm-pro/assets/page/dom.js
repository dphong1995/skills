export const $ = (s, root = document) => root.querySelector(s);
export const el = (tag, props = {}, ...kids) => {
  const n = document.createElement(tag);
  for (const [k, v] of Object.entries(props)) {
    if (v == null || v === false) continue;
    if (k === 'class') n.className = v;
    else if (k === 'text') n.textContent = v;
    else if (k.startsWith('on')) n.addEventListener(k.slice(2), v);
    else n.setAttribute(k, v === true ? '' : v);
  }
  for (const c of kids.flat()) if (c != null && c !== false) n.append(c);
  return n;
};

// Class sets used in more than one place. daisyUI's color classes take no Tailwind variant, so a variant reads the color variable instead.
export const FOCUS = 'focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[var(--color-primary)]';
export const CODE = 'font-mono text-[.86em] bg-base-200 rounded px-[.3em] py-[.05em] wrap-anywhere';
// daisyUI centers a figure inside a card as a flex box, so each figure sets its own display.
export const FIG = 'm-0 min-w-0 overflow-hidden border border-base-300 rounded-box';
export const CAPTION = 'flex items-center justify-between gap-2 min-h-7 py-0.5 pr-1 pl-2.5 border-b border-base-300 font-mono text-[11px] text-muted';
export const CAP_TEXT = 'min-w-0 wrap-anywhere';
// daisyUI lays an alert's children out as columns; `block` keeps its text flowing as one paragraph.
export const CALLOUT = 'alert alert-soft alert-warning block text-base-content text-[14px] leading-normal px-3 py-1.5';
export const LIST = 'm-0 pl-[1.3em] grid gap-0.5';
export const PC = 'pl-[1.1em] -indent-[1.1em] before:inline-block before:w-[1.1em] before:indent-0 before:font-bold';
export const TEXTAREA = 'textarea w-full min-h-0 resize-y text-[13px] leading-normal px-2.5 py-[5px]';
export const btnCls = (kind, small) => ['btn', small ? 'btn-xs' : 'btn-sm',
  kind === 'primary' ? 'btn-primary' : kind === 'file' ? 'btn-soft btn-primary whitespace-nowrap' : ''].join(' ');
export const KEY_HINT = /Mac|iP/.test(navigator.platform) ? '⌘ Enter' : 'Ctrl Enter';
export const MARK = 'bg-hl text-inherit rounded-[2px] cursor-pointer data-[active=true]:bg-hl-active';
export const HL_BOX = 'grid gap-1.5 border border-base-300 rounded-box bg-base-100 p-2 my-1.5';
export const HL_QUOTE_ROW = 'flex items-start gap-2';
export const HL_QUOTE = 'flex-1 min-w-0 text-[12.5px] text-muted wrap-anywhere';
export const NOTE_BTN = btnCls('', true) + ' btn-ghost text-muted font-normal';
export const THUMB = 'relative m-0 grid justify-items-center gap-px';
export const THUMB_IMG = 'block h-16 w-auto max-w-40 object-contain rounded-md border border-base-300 cursor-zoom-in';
export const THUMB_NUM = 'font-mono text-[11px] text-muted';
export const THUMB_DEL = 'btn btn-circle btn-xs absolute -top-1.5 -right-1.5 size-5 min-h-0 p-0';
