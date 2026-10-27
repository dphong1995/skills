# Artifact Interview

Every interview question goes on ONE Artifact page per brainstorm, one question at a time. The
user reads and answers on the page; you poll the page's database for the answer, then put the
next question on the same page. The chat carries only the link and one-line status updates.

**Covers:** the scope-fixing question (Q0), every Q&A round, and the approach pick (brainstorm-pro
Phase 3 / cook 1D, including a collapsed bug-fix's candidate fixes). **Does not cover:**
approve/changes gates on something already written (per-section design approval, the spec review,
the "just code it" confirmation). Those stay in chat.

Template: `../assets/interview-page.html` (relative to this file). Its page renders whatever is in
the db, so you never edit its markup beyond the `<title>`.

## Setup (once per brainstorm)

0. Load what the publish needs: the `artifact-design` and `artifact-capabilities` skills (the
   `Artifact` tool requires them before a page with `capabilities`), and `ArtifactData` through
   `ToolSearch` when it is listed as deferred.
1. One `Bash`: copy the template to the scratchpad as `interview-<slug>.html` and `sed` its
   `<title>` to a 2-4 word name of the topic.
2. Publish it: `Artifact` with that `file_path`, `capabilities: {db: {}}`, `icon: "question"`,
   and a one-sentence `description` of the topic.
3. Seed it with one `ArtifactData batch`: `meta/state` = `{topic, status: "asking"}` and the
   first question as `questions/q01`. Keep the `version` each write returns; every later write to
   an existing document (`meta/state`) passes it as `if_version`.
4. Functional check, once: `ArtifactData list` of `questions`.
5. Chat: the link plus the first question's title, one line.

## The question document (`questions/qNN`)

Ids `q01`, `q02`, … in the order asked; `n` is the same number. Write content in the user's
language; identifiers, file paths and code stay as they are. Inline text supports `` `code` `` and
`**bold**` only.

| Field | Content |
|---|---|
| `n` | Question number. |
| `stage` | Short tag for the kind of gap: Mục tiêu / Phạm vi / Ràng buộc / Tiêu chí xong / Cách làm / Phản biện. |
| `title` | The decision in one plain sentence. No jargon the user would have to look up. |
| `why` | What changes depending on the answer: what gets built differently, what it rules in or out. 1-2 sentences. |
| `context` | Blocks (see below): how it works today, with scout evidence (`file:line`, existing component or table names); a concrete scenario of the situation the decision is about; every term the options use, explained in plain words. A `table` when options differ along the same axes. |
| `options` | 2-4 entries. Each: `id`, `label` (2-6 words), `means` (what concretely happens / what the user would see), `example` (one concrete case played through this option), `pros`, `cons` (short lists), `effort` (e.g. `thấp`, `1 file FE`), `recommended: true` on exactly one. |
| `multi` | `true` only when several options can hold together. |
| `recommendation` | Why the recommended option wins here, tied to the evidence in `context`. |

Every question always offers a type-your-own answer on top of your options: the page adds a
"Khác (tự nhập)" option with its own text box after them, plus a "Chưa đủ thông tin, giải thích
thêm" button. Never add either to `options`.

Context blocks (`t` is the type):
`{t:"p", text}` · `{t:"list", items:[…], ordered?}` · `{t:"code", text, label?}` (snippet, current
behaviour, or an ASCII flow) · `{t:"table", head:[…], rows:[[…]]}` · `{t:"note", text}` (a caveat
the user must not miss).

**Card test before writing:** someone who has not read the code or this conversation can decide
from the card alone. If deciding needs a fact you have not shown, add it to `context`.

## Waiting for the answer (poll loop)

1. After writing `questions/qNN`, start `Bash` with `run_in_background: true`, command `sleep 20`,
   description `Chờ câu trả lời qNN #1`. End the turn with one status line.
2. When timer `#k` finishes, `ArtifactData get` `answers` / `qNN`.
   - Exists → process it (next section).
   - Missing, k < 26 → start timer `#k+1` and end the turn with no output: `sleep 20` for
     k+1 ≤ 9 (about 3 minutes), `sleep 60` after that.
   - Missing, k = 26 (about 20 minutes) → start no timer; say once in chat:
     "Câu qNN vẫn chờ trên trang: trả lời xong thì nhắn bất kỳ gì ở đây."
3. Keep one timer at a time. A timer whose qNN is already answered ends the turn with no output.
4. A chat message while waiting: if it answers the question, take it as the answer and write it to
   `answers/qNN` with `source: "chat"` so the page's history stays complete; if it says the user
   answered on the page ("xong", "done", "rồi"), read `answers/qNN` now. A chat answer that fits
   none of the options is written as `choice: "other"` with their words in `other`.

## Processing the answer (`answers/qNN`)

`{choice, other, note, answeredAt}`. `choice` is an option `id` (an array when `multi`),
`"other"` (the user typed their own option into `other`; in a `multi` answer it can sit beside
option ids), or `"explain"` (the user needs more information; `note` may say what is unclear).
Read `other` and `note` the way you would read the user's chat reply to that question.

- `"explain"`, or an answer that is a question back ("X làm thế nào?", "check how Y does it") →
  research it, then ask the same decision again as the next `qNN` (same option ids), with the
  missing explanation in `context` and your reading of their note restated in `why`. A re-ask
  does not count toward the round cap.
- Otherwise → mini re-explore if the answer reshaped scope, then write the next question.
- Never edit or delete an answered question; a follow-up is always a new document. `meta/state`
  changes only at setup and at the finish.

## Finishing

When no question is left, update `meta/state` to
`{status: "done", summary: "<the decisions, one line each>"}` (pinned with `if_version`). The page
shows its closing panel. Continue the flow in chat with a one-line count of the rounds.

## Fallback

`Artifact` publish refused or db unavailable → ask in chat instead, still one question per
message, with the same fields in Markdown: title, why, context, options with means / example /
pros / cons / effort, recommendation, and always a last option "Khác (tự nhập)" for the user's own
answer.
