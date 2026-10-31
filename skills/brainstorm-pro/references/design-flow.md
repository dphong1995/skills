# Design Flow — shared by brainstorm-pro and cook

Used by two callers: the `brainstorm-pro` skill (Phases 1–6, then Phase 7) and cook's Step 1 when no plan exists (1A–1E, then Step 2). The caller gives the path of the document to write.

- *The document* is the spec for brainstorm-pro (`{plan-root}/{YYYY-MM-DD}-{kebab-slug}/spec.md`) or `<plan-dir>/design.md` for cook.
- *The next step* is brainstorm-pro Phase 7 or cook Step 2.

Run Phases 1–6 below as written. Do nothing the next step does (invoking a skill, planning, writing code) before Phase 6 passes.

## Core Principles

- **YAGNI / KISS / DRY** — every design decision must honour these.
- **Reuse over rewrite** — scout first; if a similar feature exists, design ON TOP of it, not next to it.
- **One round at a time** — never batch questions that depend on each other; follow-ups depend on prior answers.
- **Decide from the card** — each question goes on the interview page with the context, evidence, concrete examples and trade-offs the user needs to decide without reading the code or the chat.
- **Brutal honesty** — challenge the premise, the user's preferred option, and any unexamined assumptions.
- **Evidence over assumption** — never ask the user what the code already reveals.

## Anti-Rationalization

| Thought | Reality |
|---------|---------|
| "This is too simple to need a design" | Simple projects = most wasted work from unexamined assumptions. |
| "I already know the solution" | Then writing it down takes 30 seconds. Do it. |
| "Let me explore the code first" | Phase 1 IS exploration. Don't skip the structured scout. |
| "I'll just batch the questions" | Batching dependent questions breaks adaptive follow-up. Group only independent ones (up to 4 on one page). |
| "This one is quick, I'll ask it in chat" | Every interview question goes on the HTML question page. Chat carries the file path; the answers come back from the page (or pasted). |
| "A short question is easier to answer" | A short question makes the user reconstruct the context. Fill the card: why, evidence, example per option, pros/cons, recommendation. |
| "Skip scout, the ticket is detailed" | Tickets often miss existing infra. Scout anyway, briefly. |
| "User wants action, not talk" | Bad action wastes more time than good design. |
| "Just propose one approach" | Always 2-3. Lead with rec, but show alternatives. |
| "Research the best practice first, then scout" | Scout first — the repo may already solve it (reuse beats research) and you'd research the wrong stack. Best-practice research is Phase 1.5, AFTER scout. |
| "Deep-dive the pattern now, before Q&A" | If WHAT is still fuzzy you may deep-dive a direction the user descopes. Orient lightly; deep-dive only a real, still-open fork. |
| "The design is all in the document now, so the review can be skipped" | The document IS what the user approves, section by section on the review page (Phase 6). Writing it is not approving it. |
| "Duyệt came with notes, they can wait for the plan" | Notes sent with Duyệt are small fixes: apply them to the document before the next step. |
| "No open questions left, so nothing to wait for" | Having no *questions* is why you write the document. It is not permission to hand it off. The user reviews the artifact, not your confidence in it. |
| "Self-review found nothing, so the builder check adds nothing" | You wrote it from the conversation; you cannot see what it leaves out. The builder can. |
| "The picks are on the page, so the user knows what I understood" | Picks are answers, not your understanding of why. Play it back once before Phase 3. |

## Process Flow (Authoritative)

```mermaid
flowchart TD
    A[Phase 1: Deep Scout] --> W{Know WHAT to build, or only HOW?}
    W -->|WHAT fuzzy| Q0[Ask ONE scope-fixing question<br/>on the interview page] --> R
    W -->|WHAT clear| R[Phase 1.5: Best-Practice Research<br/>orienting; deep only on a real fork]
    R --> B[Phase 2: Q&A Loop<br/>HTML question page, answers sent back]
    B --> C{Round cap or scope clear?}
    C -->|need more| B
    C -->|new fork surfaced| R
    C -->|clear| P[Thẻ phát lại<br/>one card, before Phase 3]
    P -->|Đúng| D[Phase 3: Propose 2-3 Approaches]
    P -->|Sai hoặc thiếu| FX[Fix the understanding<br/>mini re-explore; ask more on a new gap] --> FC{Thing to build changed?}
    FC -->|yes| P
    FC -->|no| D
    D --> G[Phase 4: Write the Document<br/>the design, in sections]
    G --> H[Phase 5: Document Self-Review<br/>4 checks]
    H --> I{Issues found?}
    I -->|yes| G
    I -->|no| BN{Builder check not run yet,<br/>or the pick line in Cách làm changed?}
    BN -->|yes| BC[Builder check<br/>fresh agent reads the document] --> BT[Triage; write already-answered<br/>and small items into the document] --> BQ{Questions for the user?}
    BN -->|no| J
    BQ -->|yes| BR[One interview round<br/>update the document] --> BF[4 checks on the changed parts] --> J
    BQ -->|no| BF
    J[Phase 6: User Review Gate<br/>document on a review page, notes per section] --> K{User approves the document?}
    K -->|Sửa| G
    K -->|approved| L[The next step]
```

If prose conflicts with this diagram, follow the diagram.

---

## Phase 1: Deep Scout (BEFORE asking anything)

The single highest-leverage step. Skipping or shortening this is the #1 mistake.

**Required actions:**
1. **Read `CLAUDE.md`** + any `.claude/rules/*.md` to internalize project conventions
2. **Save the Jira stories** when the topic names Jira issues: run `~/.claude/skills/brainstorm-pro/references/jira-snapshot.md` (Sonnet agents in the background; it keeps Story/Epic only). Read their one-line reports, then the saved files of the stories you design.
3. **Identify related areas** in the codebase:
   - `ls` and `grep` related directories (`src/pages/`, `src/components/`, etc.)
   - Find existing similar features (search for keywords from the topic)
   - Read any relevant existing components / hooks / services
4. **Cross-repo scan** if the project has additional working directories (e.g. backend repo) — check there too
5. **Read related docs** in `docs/` if any
6. **List user memory** if available — past feedback informs design

**Scout output (keep in working memory):**
- "Existing infrastructure" summary: what already exists and could be reused
- "Reuse opportunities": specific files/functions/types to build on
- "Conflict risks": existing patterns this feature must respect
- "Open questions for user": gaps the code does NOT answer

**Cite scout evidence in later questions.** Never ask "what database does the project use?" if scout already showed it.

**Budget:** 5-15 actions typical. Stop when you have a clear picture of reuse landscape.

---

## Phase 1.5: Best-Practice Research (conditional, right after scout)

Two DIFFERENT kinds of research — don't conflate them:
- **Reuse research (internal)** = Phase 1 scout itself. Always first.
- **Best-practice research (external)** = how the wider world solves this (canonical libs, patterns, trade-offs). THIS phase.

**Default position: immediately after scout, before Q&A.** Knowing the solution space up front makes your questions sharper — you can only ask "time-based vs context-based coalescing?" if you already know both exist.

**The one exception — defer when intent is fuzzy.** After scout, run the discriminator:

> **"Do I already know WHAT I'm building, or only HOW?"**

- **WHAT is clear** (only "how" is open) → research now.
- **WHAT is fuzzy** (scope/intent unsettled) → ask ONE scope-fixing question FIRST, then research. Researching a moving target wastes the work — you may deep-dive a direction the user then descopes.

**Calibrate depth:**
- **Orienting pass (default):** just enough to know which approaches exist and which is canonical. Light.
- **Deep-dive:** exact API semantics, pattern comparison (e.g. command vs snapshot). Reserve for a fork that genuinely affects the design AND is still open. A deep-dive may also come LATER as a top-up if Q&A opens a new fork the orienting pass didn't settle (the Q&A↔Phase-3 hinge — see the diagram's loop back to 1.5).

**Only research at all when ALL three hold** (else skip — reuse or bespoke beats research):
1. Scout found NO in-repo precedent to copy (if there is one, reuse it — don't research).
2. The problem is well-trodden externally (undo/redo, caching, auth, retry, rate-limiting…) with canonical patterns.
3. The choice materially affects the design — a real fork, not a cosmetic detail.

Anchor findings to the design and cite them in the Phase 3 approaches. Prefer the codebase's own conventions + large-system precedents over novel invention.

---

## Phase 2: Q&A Loop (one round at a time, on the interview page)

**REQUIRED:** run the interview through `~/.claude/skills/brainstorm-pro/references/html-interview.md` — a local HTML page per
round opened in the browser, a full card per question, the answers sent back from the page (or pasted into chat).
The scope-fixing question (Q0) and the Phase 3 approach pick are asked the same way.

**Rules:**
- ONE round per page — one question, or up to 4 independent ones (no answer can change another's options or relevance). Open it, wait for the answers, then open the next. A question that depends on an answer waits for the next round
- 2-4 suggested options per question with **your recommendation** clearly marked (the playback card excepted), plus a **"Khác (tự nhập)" option on every question** for the user's own answer (the page adds it; in the chat fallback, list it yourself)
- Each question targets the **biggest current gap** in your understanding
- After each answer, decide: do I need to re-explore the code? If yes, mini-explore (1-3 actions) BEFORE the next question
- Brutal honesty injection: at least once during the loop, challenge the user's framing or preferred direction with a "what if X were not true?" question

**Question targeting (biggest gap first):**
| Gap | Sample Question |
|---|---|
| Goal unclear | "When you say 'audit log', what does an admin DO with it — verify integrity, debug user issues, satisfy compliance?" |
| Constraints unclear | "Volume: ~100 events/day or ~10k? Affects pagination strategy." |
| Success criteria unclear | "If I showed you the finished page, what would make you say 'yes, exactly'?" |
| Scope drift risk | "You mentioned X. Is that v1 or v2?" |

**Round cap: 8.** If still unclear at round 8, present what you have and ask the user to fill blanks directly. Do NOT spin past the cap. Not counted: re-asks (`html-interview.md` § Reading the answer), the playback card, the builder check's rounds.

**Scope check (continuous):** if the request describes 3+ independent subsystems (e.g. "platform with chat, billing, analytics"), STOP and help decompose into sub-projects. Each sub-project gets its own brainstorm cycle.

### Thẻ phát lại (playback card)

- **When:** the goal and scope are clear, right before Phase 3. A round holding only this card.
- **Card:** `stage: "Phát lại"`, `title`: "Tôi đang hiểu việc cần làm như dưới đây. Đúng chưa?". `context`: 200-300 words — what is to be built, for whom, why, and what is in and out of scope (ideas dropped included). Only what the user said is attributed to the user; every guess is marked **(đoán)**. Options `A. Đúng` / `B. Sai hoặc thiếu` (the user notes where it is wrong). Fields: `id`, `n`, `stage`, `title`, `context`, `options` (`id`, `label` only); no `why`, `reversible` or `recommendation`.
- **Reading the answer:** A → Phase 3. B → fix your understanding (a mini re-explore if needed); ask more if the note opens a new gap; play it back again only if the thing to build changed; then Phase 3.
- **Skipped** on shortened branches ("just code it", cook's bug-fix shortcut): they do not run Phase 2.

---

## Phase 3: Propose 2-3 Approaches

After Q&A, present 2-3 viable approaches as the last question on the interview page: one option
per approach (`means`, `example`, `pros`, `cons`, `effort`), the pick and its
reason in `recommendation: { option, why }`. In content terms:

```
**Approach A — [name]** (RECOMMENDED)
- Pros: ...
- Cons: ...
- Effort: low/medium/high

**Approach B — [name]**
- Pros: ...
- Cons: ...

**Approach C — [name]** (only if it adds new dimension)
- Pros: ...
- Cons: ...
```

Lead with your recommendation and the reason. If user prefers a non-recommended option, **brutally honest pushback** as the next question on the page (stage `Phản biện`: the risk you see, keep vs. switch) — but accept the user's call if they reconfirm with reason. Once the approach is settled, go on to Phase 4.

---

## Phase 4: Write the Document

Write the document at the path the caller gives.

The design goes straight into the file; it is not walked through in chat. Scale each section to its complexity — a few sentences if simple, 200-300 words if nuanced.

**Document structure** (a Google-style design doc with the bottom line first; the reader decides from the
first two sections whether to skim or read closely):
```markdown
# Spec: <chủ đề>

## Tóm tắt
- **Thay đổi:** what the user or system gets, in one line
- **Cách làm:** the chosen approach, in one line
- **Công sức:** phases, repos, size
- **Còn cần bạn quyết:** what needs the user, or "không"

## Cần bạn xem kỹ
1-3 places that most need the reader's judgement: hard-to-reverse choices, risks, assumptions.

## Bối cảnh
Today, with `file:line` evidence and the scout's reuse findings; no solution yet.

## Mục tiêu / Không làm

## Quyết định
The interview's picks and the calls made without asking, one row each:

| Câu | Chọn | Nguồn |
|---|---|---|
| question | the pick | bạn / Claude |

## Cách làm
The chosen approach and why, then a "Phương án đã loại" table (each alternative, including
"không làm", with one reason it lost).

## Thiết kế
### Kiến trúc              (a Mermaid diagram when there are 3+ parts)
### Thành phần / File      (create/modify, what changes, what it reuses)
### Luồng dữ liệu / Kiểu   (shapes, not full code; cite file:line)
### Truy vết AC         (only for Story/Epic work: the table of `~/.claude/skills/brainstorm-pro/references/jira-snapshot.md` § Traceability table; AC text English, verbatim)

## Rủi ro & xử lý lỗi

## Kiểm thử & triển khai

## Tiêu chí hoàn thành
- [ ] Concrete, testable items

## Còn mở
- Genuinely deferred decisions (with options to revisit)

## Phụ lục: File thay đổi
FE: ...
BE: ...
```

**Nguồn column:** `bạn` = the user picked it, on the page or in chat, including with the "Theo đề xuất" button. `Claude` = you decided without asking. A `Claude` row that is hard to change later also goes in `## Cần bạn xem kỹ`.

Write the document in Vietnamese: the user reads it on the review page. Code, identifiers, file paths,
commands and text meant for Jira or a PR stay as they are. Each section opens with its conclusion.
Keep code short and cite `file:line`: the review page opens the file.

---

## Phase 5: Document Self-Review (4 checks)

After writing, re-read the document with fresh eyes:

1. **Placeholder scan** — any `TBD`, `TODO`, vague requirement? Fix inline or move to Open Items.
2. **Internal consistency** — do sections contradict each other? Architecture matches Components?
3. **Scope check** — focused enough for ONE implementation plan? Or needs decomposition?
4. **Ambiguity check** — any requirement interpretable two ways? Pick one, make explicit.

Fix issues inline. No re-review loop — fix and move on.

**Builder check:** follow `~/.claude/skills/brainstorm-pro/references/builder-check.md`.

---

## Phase 6: User Review Gate

**This gate is not optional and cannot be waived by inference.**

After self-review, show the document on its review page (`~/.claude/skills/brainstorm-pro/references/html-doc.md`), served from a
foreground `Bash` call with `timeout: 600000`:

```
node ~/.claude/skills/brainstorm-pro/assets/render-interview.mjs <document path> <scratchpad>/review-{slug}/review-NN.html --serve
```

- `Quyết định: Duyệt` → apply any notes to the document, then the next step in the same turn.
- `Quyết định: Sửa` → edit the document as the notes ask, re-run Phase 5's four checks, show the next round
  (`review-NN+1.html`). The builder check re-runs only if the round changed the chosen approach (the pick line in `## Cách làm`); wording edits or added rows in the rejected-alternatives table do not.
- Moved to the background after 10 minutes → end the turn with one line; the answer arrives as the
  task's notification. A reply in chat counts the same (stop the serve command).
- No browser (exit 4) → ask in chat: "Document written to `<document path>`. Please review
  and let me know if you want any changes before we move to the implementation plan." and wait.

### What does NOT satisfy this gate

The only things that open it are the user's **Duyệt** on the document's review page, or their approving
reply in chat, in this conversation, after the document file exists. None of the following count — each is a real way this gate has been
skipped by mistake:

- **"No need to walk me through it"** → there is no walk-through: the document goes straight to its review
  page, so this is never a reason to skip that page.
- **The builder check reported nothing important left** — it read the document; the user did not.
- **A question answered on the interview page** — that settled one decision, not the document.
- **"No open questions left"** — that is the precondition for writing the document, not for shipping it.
- **A standing preference to auto-run `cook`** — that governs what happens after this gate, never this gate.
- **The document looking obviously right to you.** Self-review is Phase 5 and it already happened. Phase
  6 exists because the author is the worst reviewer of their own scope calls.

If the user has genuinely pre-authorized skipping this ("just build it", "don't wait for me"), quote
the words they used when you proceed, so the assumption is visible and correctable.

---

## Tool Usage

- `Read` / `Glob` / `Grep` for scout (Phase 1)
- `Bash` for `ls`, `find`, `git log` etc. during scout
- `context7` (library docs), `WebSearch`, or a `document-specialist` sub-agent for Phase 1.5 best-practice research
- `Write` (question JSON) + `Bash` (`render-interview.mjs --serve` in the foreground, 10-minute timeout, same message) for every interview question (Q0, Phase 2, Phase 3) — `~/.claude/skills/brainstorm-pro/references/html-interview.md`; chat only as its fallback
- Sub-agents OK for parallel research IF Phase 1 reveals multiple unknown areas — but keep it focused
- `Write` to save the document in Phase 4; `Bash` (`render-interview.mjs <document> … --serve`, foreground, 10-minute timeout) to show it in Phase 6 — `~/.claude/skills/brainstorm-pro/references/html-doc.md`
- `Agent` for the builder check — settings and prompt in `~/.claude/skills/brainstorm-pro/references/builder-check.md` (its round goes on the interview page like any other)

## Examples

### Good — scout citation
```
[Phase 1 found existing PatientAudit entity + AuditLogService gateway]

Round 2: "I found PatientAudit BE entity and AuditLogService FE gateway in
src/gateways/AuditLogService.ts. The new feature can extend the existing
search criteria. Should we reuse the existing gateway or create a new
admin-scoped one?"
```

### Good — brutal honesty pushback
```
User: "I want a separate audit log table for this feature."
Response: "Pushback — there's already a PatientAudit table with the schema
you'd need. Adding a parallel table violates DRY and creates two sources
of truth for the same concept. Strong recommendation: extend PatientAudit
with the missing fields. Are there specific reasons the existing table
won't work?"
```

### Bad — batching
```
"What's the audience? Where should it appear? What events? What retention?"
```
Why bad: 4 questions whose answers shape each other (the audience decides where it appears and which events matter) break adaptive follow-up.

### Bad — asking what scout could answer
```
"What database does the project use?"
```
Why bad: scout would reveal `TypeORM` instantly. Wastes a round.

## Escalation

- **Topic too large** → decompose into sub-projects; work only the first.
- **Scope keeps drifting** → freeze scope explicitly; document drifted items as v2/Open Items.
- **No clear recommendation possible** → present approaches with trade-offs, ask user to pick, do NOT push.

## Final Checklist

- [ ] Phase 1 scout actions logged (≥5 actions; cited in later questions)
- [ ] Phase 1.5 best-practice research done right after scout when warranted (all 3 conditions), or consciously skipped (reuse/bespoke/no real fork); deep-dive reserved for a real open fork
- [ ] Q&A respected one-per-round rule, max 8 rounds
- [ ] Every interview question (Q0, Q&A, approach pick) went on the interview page as a full card (why, context with evidence, options with example/pros/cons, recommendation; the playback card excepted: its own field set); each answer taken from the page's reply (or the user's pasted one) before the next question
- [ ] Mid-stream re-explore happened at least once if answer reshaped scope
- [ ] At least one brutal honesty pushback on premise/assumption
- [ ] 2-3 approaches presented with trade-offs + recommendation
- [ ] Document written to the path the caller gave
- [ ] Document self-review completed (4 checks)
- [ ] Document shown on its review page (`--serve`); notes applied; the user approved it — you can quote the Duyệt or the reply
- [ ] Playback card asked before Phase 3
- [ ] Builder check ran and was triaged
- [ ] `## Quyết định` has the Nguồn column and the hard-to-change `Claude` rows are in `## Cần bạn xem kỹ`
