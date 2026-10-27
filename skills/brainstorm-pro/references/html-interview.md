# HTML Interview

Interview questions are shown as a local HTML page in the user's browser, one round at a time. The
user answers there, presses the floating "Copy câu trả lời" button and pastes into the chat; that
message carries every answer of the round. Nothing is published and nothing is polled.

**Covers:** the scope-fixing question (Q0), every Q&A round, and the approach pick (brainstorm-pro
Phase 3 / cook 1D, including a collapsed bug-fix's candidate fixes). **Does not cover:**
approve/changes gates on something already written (per-section design approval, the spec review,
the "just code it" confirmation). Those stay in chat.

Files (brainstorm-pro skill directory, normally `~/.claude/skills/brainstorm-pro`):
`assets/interview-page.html` (the page) and `assets/render-interview.mjs` (puts one round's JSON
into it). Never edit the page by hand.

## What goes in one round

One question, or up to 4 **independent** questions: no answer among them can change another's
options, relevance or recommendation. A question that depends on an answer waits for the next
round. A page counts as one round toward the round cap.

## Asking a round

1. `Write` the round as JSON to `<scratchpad>/interview-<slug>/round-NN.json` (fields below).
2. One `Bash`:
   `node ~/.claude/skills/brainstorm-pro/assets/render-interview.mjs <dir>/round-NN.json <dir>/round-NN.html && open <dir>/round-NN.html`
   (the script rejects malformed JSON, an empty `questions`, or a question missing
   `id`/`n`/`title`/`options`; fix and rerun).
3. End the turn with one chat line: the question titles and the `round-NN.html` path, so the user
   can reopen it. The user's next message is the answer.

Each round opens in its own browser tab; earlier files stay as they were.

## The round JSON (`round-NN.json`)

`{topic, decided, questions: [ … ]}`

- `topic`: the brainstorm topic, the same in every round.
- `decided`: the decisions settled so far, one short line each (`Câu 1: …`). Empty in round 1.
- `questions`: one object per question:

| Field | Content |
|---|---|
| `id`, `n` | `q03` and `3`. Ids run on across rounds in the order asked. |
| `stage` | Short tag for the kind of gap: Mục tiêu / Phạm vi / Ràng buộc / Tiêu chí xong / Cách làm / Phản biện. |
| `title` | The decision in one plain sentence. No jargon the user would have to look up. |
| `why` | What changes depending on the answer: what gets built differently, what it rules in or out. 1-2 sentences. |
| `context` | Blocks (see below): how it works today, with scout evidence (`file:line`, existing component or table names); a concrete scenario of the situation the decision is about; every term the options use, explained in plain words. A `table` when options differ along the same axes. |
| `options` | 2-4 entries. Each: `id`, `label` (2-6 words), `means` (what concretely happens / what the user would see), `example` (one concrete case played through this option), `pros`, `cons` (short lists), `effort` (e.g. `thấp`, `1 file FE`), `recommended: true` on exactly one. |
| `multi` | `true` only when several options can hold together. |
| `recommendation` | Why the recommended option wins here, tied to the evidence in `context`. |

Write content in the user's language; identifiers, file paths and code stay as they are. Inline
text supports `` `code` `` and `**bold**` only.

The page shows the options side by side, lined up part by part (`means`, `example`, `pros`/`cons`),
so give every option the same fields and keep them short and parallel. The recommendation sits
under the options, above the answer strip; nothing is preselected.

The answer strip adds two choices after yours on every question: "Khác (tự nhập)" with its own text
box, and "Cần giải thích thêm". Never add either to `options`. The copy button refuses until every
question on the page has a choice.

Context blocks (`t` is the type):
`{t:"p", text}` · `{t:"list", items:[…], ordered?}` · `{t:"code", …}` (below) ·
`{t:"table", head:[…], rows:[[…]]}` · `{t:"note", text}` (a caveat the user must not miss).

Code blocks are syntax-highlighted (highlight.js from cdnjs; plain text when offline):
- Existing code → `{t:"code", file, lines:[from, to]}` with `file` an absolute path. The script
  cuts the snippet from the file (no `text`), numbers its lines, labels it with the path and range,
  and embeds the whole file (up to 512 KB) behind a "Xem toàn file" button that opens it with the
  range highlighted. The script rejects a relative path or a range outside the file.
- Anything else (a proposed change, a data shape, an ASCII flow) → `{t:"code", text, label, lang}`.
  `label` says what it shows (`job result after the change`). Add `file` (no `lines`) to also
  offer the whole file next to a changed version.
- `lang` is a highlight.js name (`typescript`, `json`, `sql`, `bash`, `plaintext` for ASCII flows
  and pseudo-code); by default it comes from the file extension, else it is guessed.
- Keep a snippet to the lines that matter; the full file is one click away.

**Card test before writing:** someone who has not read the code or this conversation can decide
from the card alone. If deciding needs a fact you have not shown, add it to `context`.

## Reading the answer

The pasted text has one block per question, separated by a blank line:

```
[q03] <title>
Chọn: B. <label>            (several: "A. …; B. …"; own answer: "Khác (tự nhập): <text>")
Ghi chú: <note>             (only when the user wrote one)
```

`Chọn: Cần giải thích thêm` means the user needs more information on that question (`Ghi chú` may
say what is unclear). A reply without `[qNN]` headers is the user answering in their own words:
match it to the open questions, and ask in chat if it is unclear which question it answers.

- Explain request, or an answer that is a question back ("X làm thế nào?", "check how Y does it")
  → research it, then ask the same decision again in the next round under a new id (same option
  ids), with the missing explanation in `context` and your reading of their note restated in
  `why`. A re-ask does not count toward the round cap.
- Otherwise → mini re-explore if the answers reshaped scope, then build the next round.

## Finishing

Nothing to close. Continue the flow in chat with a one-line count of the rounds; the decisions go
into the spec (or cook's Approved Design Block) as usual.

## Fallback

`open` fails (no browser, a remote session) → ask in chat instead, one round per message, with the
same fields in Markdown: title, why, context, options with means / example / pros / cons / effort,
recommendation, and always a last option "Khác (tự nhập)" for the user's own answer.
