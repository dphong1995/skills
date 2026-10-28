import { $, btnCls, el, FOCUS, KEY_HINT, TEXTAREA, PC } from './dom.js';
import { plain } from './strings.js';
import { inline } from './text.js';
import { block, codeBlock, mockBlock } from './blocks.js';
import { CARD, copyText, fabMsg, fit, main, out, postText, say, sendMode, storage } from './fab.js';
import { imageInput, images, imageStrip, imgLines } from './images.js';

export const interviewPage = () => {
  const KEYS = 'ABCDEFGHIJ';
  const keyOf = (i) => KEYS[i] || String(i + 1);
  const OTHER = { id: 'other', label: 'Khác (tự nhập)', means: 'Không phương án nào ở trên đúng ý: gõ phương án của bạn.' };
  const EXPLAIN = { id: 'explain', label: 'Cần giải thích thêm', means: 'Claude giải thích rõ hơn rồi hỏi lại câu này. Ghi điều bạn chưa rõ vào ô ghi chú.' };
  const Q = D.questions;
  const nums = Q.map((q) => q.n);
  const range = nums.length > 1 ? 'Câu ' + Math.min(...nums) + '–' + Math.max(...nums) : 'Câu ' + (nums[0] ?? '');

  document.title = range + ' · ' + (D.topic || 'Phỏng vấn thiết kế');
  $('#topic').textContent = D.topic || 'Phỏng vấn thiết kế';
  const navLinks = Q.map((q) => el('a', { href: '#card-' + q.id, class: 'btn btn-xs btn-primary btn-outline font-mono font-medium' }));
  $('#qnav').append(...navLinks);

  // One card per question: what to read first, the answer strip last. The card's DOM is the answer state.
  const questionCard = (q) => {
    const multi = !!q.multi;
    const own = q.options || [];
    const options = [...own, OTHER, EXPLAIN];
    const state = el('span', { class: 'badge badge-sm ml-auto normal-case tracking-normal' });
    const inputs = [];
    const optCards = new Map();
    const otherBox = el('textarea', { id: q.id + '-other', rows: '1', hidden: true, class: TEXTAREA + ' flex-[1_1_18rem]', placeholder: 'Phương án của bạn…', 'aria-label': 'Phương án tự nhập' });
    const note = el('textarea', { id: q.id + '-note', rows: '1', class: TEXTAREA + ' flex-[1_1_18rem]', placeholder: 'Ghi chú (không bắt buộc): lý do, điều kiện kèm theo, hoặc điều bạn chưa rõ…', 'aria-label': 'Ghi chú cho câu ' + (q.n ?? '') });
    otherBox.addEventListener('input', () => fit(otherBox));
    note.addEventListener('input', () => fit(note));
    const strip = imageStrip([otherBox, note]);
    imageInput(otherBox, strip);
    imageInput(note, strip);

    // Option cards sit side by side and share rows (subgrid), so each part lines up across options.
    const parts = own.map((o, i) => [
      el('div', { class: 'flex flex-wrap items-baseline gap-x-2 gap-y-0.5' },
        el('span', { class: 'font-mono font-semibold text-primary', text: keyOf(i) + '.' }),
        el('span', { class: 'font-semibold' }, inline(o.label)),
        o.recommended ? el('span', { class: 'badge badge-xs badge-primary font-mono tracking-[.06em] uppercase', text: 'Đề xuất' }) : null,
        o.effort ? el('span', { class: 'ml-auto text-[12px] text-muted' }, 'Công sức: ', inline(o.effort)) : null),
      o.means ? el('p', {}, inline(o.means)) : null,
      o.mockup ? mockBlock(o.mockup) : null,
      o.code ? codeBlock(o.code) : null,
      o.example ? el('p', { class: 'text-[13px] bg-base-200 rounded-md px-2 py-1 whitespace-pre-line' }, el('strong', { text: 'Ví dụ: ' }), inline(o.example)) : null,
      (o.pros && o.pros.length) || (o.cons && o.cons.length) ? el('ul', { class: 'grid gap-px content-start text-[13px]' },
        (o.pros || []).map((p) => el('li', { class: PC + " before:content-['+'] before:text-[var(--color-success)]" }, inline(p))),
        (o.cons || []).map((c) => el('li', { class: PC + " before:content-['–'] before:text-[var(--color-error)]" }, inline(c)))) : null,
    ]);
    const used = [0, 1, 2, 3, 4, 5].filter((s) => parts.some((p) => p[s]));
    // A click on the card picks the option like its button in the answer strip, unless it lands on a control or ends a text selection.
    const optCard = (o, i) => {
      const box = el('div', { class: 'card border px-3 py-2.5 grid grid-rows-subgrid min-w-0 cursor-pointer transition-[border-color,background-color] duration-150 motion-reduce:transition-none hover:border-[var(--color-primary)] data-[chosen=true]:bg-tint data-[chosen=true]:border-[var(--color-primary)] data-[chosen=true]:[box-shadow:inset_0_0_0_1px_var(--color-primary)] '
        + (o.recommended ? 'border-primary' : 'border-base-300'), style: 'grid-row: span ' + used.length,
        onclick: (e) => {
          if (e.target.closest('a, button, input, textarea, iframe') || String(getSelection())) return;
          const input = inputs.find((x) => x.value === o.id);
          input.checked = multi ? !input.checked : true;
          input.dispatchEvent(new Event('change', { bubbles: true }));
        } },
        used.map((s) => parts[i][s] || el('div')));
      optCards.set(o.id, box);
      return box;
    };

    // Each choice is a daisyUI button that is itself the radio (or checkbox); its label is drawn from aria-label.
    const choice = (o, i) => {
      const builtin = o === OTHER || o === EXPLAIN;
      const input = el('input', { type: multi ? 'checkbox' : 'radio', name: 'choice-' + q.id, id: q.id + '-opt-' + o.id, value: o.id,
        'aria-label': (builtin ? '' : keyOf(i) + '. ') + plain(o.label) + (o.recommended ? ' ★' : ''),
        title: builtin ? o.means : o.recommended ? 'Claude đề xuất' : null,
        class: 'btn btn-sm font-medium checked:font-semibold' + (builtin ? ' btn-dash checked:bg-tint checked:text-[var(--color-primary)] checked:border-[var(--color-primary)]' : '') });
      inputs.push(input);
      input.addEventListener('change', () => {
        if (multi && input.checked) {
          for (const other of inputs) if (other !== input && (o === EXPLAIN || other.value === 'explain')) other.checked = false;
        }
        refresh();
        if (input.checked && o === OTHER) otherBox.focus();
        if (input.checked && o === EXPLAIN) note.focus();
      });
      return input;
    };

    const recI = own.findIndex((o) => o.recommended);
    const hasUndo = q.reversible != null;
    const undoText = q.reversible ? 'Dễ đổi về sau' : 'Khó đổi về sau';
    const card = el('article', { class: CARD, id: 'card-' + q.id },
      el('header', { class: 'grid gap-1' },
        el('div', { class: 'flex flex-wrap items-center gap-1.5 font-mono tracking-[.06em] uppercase' },
          el('span', { class: 'badge badge-sm badge-primary badge-outline', text: 'Câu ' + (q.n ?? '') }),
          q.stage ? el('span', { class: 'badge badge-sm badge-ghost text-muted', text: q.stage }) : null,
          hasUndo ? el('span', { class: 'badge badge-sm normal-case tracking-normal ' + (q.reversible ? 'badge-success badge-outline' : 'badge-warning badge-soft font-semibold'),
            title: q.reversibleWhy || null, text: undoText }) : null,
          state),
        el('h2', { class: 'text-[18px] leading-[1.35] font-bold text-balance' }, inline(q.title))),
      q.why || (hasUndo && q.reversibleWhy) ? el('div', { class: 'alert alert-soft alert-info block text-base-content text-[14px] leading-normal px-3 py-1.5 [&_strong]:text-[var(--color-info)]' },
        q.why ? el('p', {}, el('strong', { text: 'Vì sao cần chốt: ' }), inline(q.why)) : null,
        hasUndo && q.reversibleWhy ? el('p', {}, el('strong', { text: undoText + ': ' }), inline(q.reversibleWhy)) : null) : null,
      q.context && q.context.length ? el('div', { class: 'grid gap-2 min-w-0' }, q.context.map(block)) : null,
      own.length ? el('div', { class: 'grid gap-x-2 gap-y-1.5 grid-cols-[repeat(auto-fit,minmax(min(100%,15rem),1fr))]'
        + (own.length === 4 ? ' max-[1060px]:grid-cols-[repeat(auto-fit,minmax(min(100%,22rem),1fr))]' : '') }, own.map(optCard)) : null,
      q.recommendation ? el('p', { class: 'border-l-[3px] border-primary px-2.5 py-0.5' },
        el('strong', { class: 'text-primary' }, 'Claude đề xuất', recI >= 0 ? [' ' + keyOf(recI) + '. ', inline(own[recI].label)] : null, ': '),
        inline(q.recommendation)) : null,
      el('div', { class: 'border-t border-dashed border-base-300 pt-2.5 grid gap-2' },
        el('div', { class: 'flex items-baseline gap-2' },
          el('span', { class: 'font-mono text-[11px] font-medium tracking-[.06em] uppercase text-muted', text: 'Trả lời' }),
          el('span', { class: 'text-[12px] text-muted', text: multi ? 'chọn một hoặc nhiều' : 'chọn một' })),
        el('div', { class: 'flex flex-wrap gap-1.5' }, options.map(choice)),
        el('div', { class: 'flex flex-wrap gap-2' }, otherBox, note),
        strip));
    const otherInput = inputs[own.length];

    function refresh() {
      const done = inputs.some((i) => i.checked);
      state.textContent = done ? 'Đã chọn' : 'Chưa chọn';
      state.classList.toggle('badge-success', done);
      state.classList.toggle('badge-soft', done);
      state.classList.toggle('badge-ghost', !done);
      for (const i of inputs) if (optCards.has(i.value)) optCards.get(i.value).dataset.chosen = String(i.checked);
      otherBox.hidden = !otherInput.checked;
      strip.refresh();
      card.classList.remove('attention');
      updateFab();
    }

    const labelOf = (input) => {
      if (input.value === 'other') return 'Khác (tự nhập): ' + otherBox.value.trim();
      if (input.value === 'explain') return 'Cần giải thích thêm';
      const i = options.findIndex((o) => o.id === input.value);
      return keyOf(i) + '. ' + options[i].label;
    };

    refresh();
    return {
      q, card,
      rec: recI >= 0 ? own[recI].id : null,
      // null when complete, else what is missing.
      problem: () => {
        if (!inputs.some((i) => i.checked)) return 'chưa chọn phương án';
        if (otherInput.checked && !otherBox.value.trim()) return 'chọn "Khác" nhưng chưa gõ phương án';
        return null;
      },
      done: () => inputs.some((i) => i.checked),
      text: () => {
        const lines = ['[' + q.id + '] ' + q.title, 'Chọn: ' + inputs.filter((i) => i.checked).map(labelOf).join('; ')];
        if (note.value.trim()) lines.push('Ghi chú: ' + note.value.trim());
        lines.push(...imgLines((otherInput.checked ? otherBox.value : '') + '\n' + note.value));
        return lines.join('\n');
      },
      pick: (id) => {
        const input = inputs.find((i) => i.value === id);
        if (input) { input.checked = true; refresh(); }
      },
      get: () => ({ c: inputs.filter((i) => i.checked).map((i) => i.value), o: otherBox.value, n: note.value }),
      set: (s) => {
        for (const i of inputs) i.checked = !!s && (s.c || []).includes(i.value);
        otherBox.value = (s && s.o) || '';
        note.value = (s && s.n) || '';
        refresh();
        fit(otherBox);
        fit(note);
      },
    };
  };

  const fabCount = el('span', { class: 'font-mono text-[12px] text-muted whitespace-nowrap' });
  let cards = [];

  const store = storage('interview-draft:' + location.pathname);
  const save = () => store.set({ ...Object.fromEntries(cards.map((c) => [c.q.id, c.get()])), _img: Object.fromEntries(images) });

  const acceptBtn = el('button', { type: 'button', class: btnCls(), title: 'Chọn phương án Claude đề xuất cho các câu chưa chọn', onclick: () => {
    const todo = cards.filter((c) => !c.done() && c.rec);
    for (const c of todo) c.pick(c.rec);
    save();
    say('ok', 'Đã chọn theo đề xuất cho ' + todo.length + ' câu. Xem lại rồi ' + (sendMode ? 'gửi.' : 'copy.'));
  } }, 'Theo đề xuất');

  function updateFab() {
    fabCount.textContent = cards.filter((c) => c.done()).length + '/' + Q.length + ' câu';
    acceptBtn.hidden = !cards.some((c) => !c.done() && c.rec);
    cards.forEach((c, k) => {
      const done = c.done();
      navLinks[k].classList.toggle('btn-outline', !done);
      navLinks[k].textContent = 'Câu ' + (c.q.n ?? '') + (done ? ' ✓' : '');
    });
    say('');
  }

  if (D.decided && D.decided.length) {
    main.append(el('details', { class: 'collapse collapse-arrow bg-base-100 border border-base-300 text-[13px]' },
      el('summary', { class: 'collapse-title min-h-0 py-1.5 ps-3 font-semibold ' + FOCUS, text: 'Đã chốt trước đó (' + D.decided.length + ')' }),
      el('div', { class: 'collapse-content px-3 pb-2' }, el('ul', { class: 'm-0 pl-[2em] grid gap-0.5 list-disc' }, D.decided.map((d) => el('li', {}, inline(d)))))));
  }
  cards = Q.map(questionCard);
  for (const c of cards) main.append(c.card);
  main.addEventListener('input', save);
  main.addEventListener('change', save);

  // The answers of every card, or null after pointing at the first card that is not complete.
  const collect = () => {
    const missing = cards.find((c) => c.problem());
    if (missing) {
      missing.card.classList.add('attention');
      missing.card.scrollIntoView({ behavior: 'smooth', block: 'start' });
      say('warn', 'Câu ' + (missing.q.n ?? '') + ' ' + missing.problem() + '.');
      return null;
    }
    return cards.map((c) => c.text()).join('\n\n');
  };
  const copyAll = async () => {
    const text = collect();
    if (text == null) return;
    if (await copyText(text)) {
      say('ok', 'Đã copy ' + cards.length + ' câu trả lời. Dán vào terminal của Claude rồi Enter.');
      setTimeout(() => { if (fabMsg.dataset.kind === 'ok') say(''); }, 4000);
    }
    else say('warn', 'Không copy tự động được: chọn đoạn bên dưới, copy rồi dán vào terminal của Claude.');
  };
  const sendBtn = el('button', { type: 'button', class: btnCls('primary'), onclick: () => sendAll() }, 'Gửi câu trả lời', el('kbd', { class: 'kbd kbd-xs', text: KEY_HINT }));
  const sendAll = async () => {
    const text = collect();
    if (text == null) return;
    sendBtn.disabled = true;
    try {
      await postText(text);
      store.set(null);
      sendBtn.textContent = 'Đã gửi';
      say('ok', 'Đã gửi cho Claude. Có thể đóng tab này.');
    } catch (e) {
      sendBtn.disabled = false;
      say('warn', await copyText(text)
        ? 'Không gửi được (Claude không còn chờ trang này). Đã copy câu trả lời: dán vào terminal của Claude.'
        : 'Không gửi được và không copy tự động được: chọn đoạn bên dưới, copy rồi dán vào terminal của Claude.');
    }
  };
  const submit = () => (sendMode ? (sendBtn.disabled ? null : sendAll()) : copyAll());

  document.body.append(el('aside', { class: 'toast toast-end z-10 items-end', 'aria-label': 'Gửi câu trả lời' }, fabMsg, out,
    el('div', { class: 'flex items-center gap-1.5 bg-base-100 border border-base-300 rounded-box py-1 pr-1 pl-3 shadow-lg' }, fabCount, acceptBtn,
      sendMode ? [el('button', { type: 'button', class: btnCls(), title: 'Copy để dán vào terminal', onclick: copyAll }, 'Copy'), sendBtn]
        : el('button', { type: 'button', class: btnCls('primary'), onclick: copyAll }, 'Copy câu trả lời', el('kbd', { class: 'kbd kbd-xs', text: KEY_HINT })))));
  document.addEventListener('keydown', (e) => { if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) { e.preventDefault(); submit(); } });
  updateFab();

  const draft = store.get();
  if (draft && cards.some((c) => { const s = draft[c.q.id]; return s && ((s.c || []).length || s.o || s.n); })) {
    for (const [k, v] of Object.entries(draft._img || {})) images.set(+k, v);
    for (const c of cards) if (draft[c.q.id]) c.set(draft[c.q.id]);
    say('', 'Đã khôi phục câu trả lời đã lưu.', el('button', { type: 'button', class: btnCls('', true) + ' ml-2', onclick: () => {
      images.clear();
      for (const c of cards) c.set(null);
      store.set(null);
      say('');
    } }, 'Xóa nháp'));
  }
};
