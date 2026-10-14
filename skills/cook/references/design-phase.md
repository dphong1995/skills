# Step 1: Design (brainstorm-pro flow, in-line)

Runs ONLY when no plan path was provided. This is the `brainstorm-pro` flow verbatim through
its design stage — deep scout → best-practice research → one-question-at-a-time Q&A →
2-3 approaches → sectioned design with per-section approval.

**Terminal difference vs `brainstorm-pro`:** where brainstorm-pro writes `spec.md` and hands
off to `ck-plan`, cook does NOT write a spec. Once the last design section is approved, the
approved design is carried verbatim into Step 2, where the **orchestrator itself** writes the
plan files (no `planner` sub-agent), and execution continues in the same run.

**The orchestrator runs this step itself** — scout/Q&A/design are conversational and must not
be delegated. Sub-agents are allowed only as parallel research helpers inside 1A/1B.

## Core Principles

- **YAGNI / KISS / DRY** — every design decision must honour these.
- **Reuse over rewrite** — scout first; if a similar feature exists, design ON TOP of it.
- **One question at a time** — never batch; follow-ups depend on prior answers.
- **Brutal honesty** — challenge the premise, the user's preferred option, unexamined assumptions.
- **Evidence over assumption** — never ask the user what the code already reveals.

## Anti-Rationalization (design step)

| Thought | Reality |
|---------|---------|
| "This is too simple to need a design" | Simple tasks = most wasted work from unexamined assumptions. |
| "I already know the solution" | Then writing it down takes 30 seconds. Do it. |
| "Cook is supposed to run without stopping" | The no-stop rule starts at Step 2. Step 1 is interactive by design. |
| "I'll just batch the questions" | Batching breaks adaptive follow-up. One at a time. |
| "Skip scout, the ticket is detailed" | Tickets often miss existing infra. Scout anyway, briefly. |
| "Just propose one approach" | Always 2-3. Lead with rec, but show alternatives. |
| "Research the best practice first, then scout" | Scout first — the repo may already solve it. Research is 1B, AFTER scout. |
| "Deep-dive the pattern now, before Q&A" | If WHAT is fuzzy you may deep-dive a direction the user descopes. |
| "Design approved, let me start editing files" | No. Write the plan yourself first (Step 2), then dispatch sub-agents. |
| "Design approved, hand it to `planner`" | No `planner` in cook. You held the conversation, so you write the plan. |

## Flow

```mermaid
flowchart TD
    A[1A: Deep Scout] --> W{Know WHAT to build, or only HOW?}
    W -->|WHAT fuzzy| Q0[Ask ONE scope-fixing question] --> R
    W -->|WHAT clear| R[1B: Best-Practice Research<br/>orienting; deep only on a real fork]
    R --> B[1C: Q&A Loop]
    B --> C{Round cap or scope clear?}
    C -->|need more| B
    C -->|new fork surfaced| R
    C -->|clear| D[1D: Propose 2-3 Approaches]
    D --> E[1E: Sectioned Design]
    E --> F{All sections approved?}
    F -->|no| E
    F -->|yes| G[Step 2: Plan]
```

---

## 1A: Deep Scout (BEFORE asking anything)

The single highest-leverage step. Skipping or shortening this is the #1 mistake.

**Required actions:**
1. **Read `CLAUDE.md`** + any `.claude/rules/*.md` to internalize project conventions.
2. **Identify related areas** in the codebase: `ls`/`grep` related directories, find existing
   similar features (search keywords from the topic), read relevant existing
   components / hooks / services.
3. **Cross-repo scan** if the project has additional working directories (e.g. backend repo).
4. **Read related docs** in `docs/` if any.
5. **List user memory** if available — past feedback informs design.

**Scout output (keep in working memory, carry into Step 2):**
- "Existing infrastructure" — what already exists and could be reused
- "Reuse opportunities" — specific files/functions/types to build on
- "Conflict risks" — existing patterns this feature must respect
- "Open questions for user" — gaps the code does NOT answer

**Cite scout evidence in later questions.** Never ask "what database does the project use?"
if scout already showed it.

**Budget:** 5-15 actions typical. `document-specialist` / `Explore` sub-agents may be dispatched in
parallel for distinct areas (reports ≤150 lines); the orchestrator still owns synthesis.

**Output:** `✓ Step 1A: Scout complete - [N] actions, [M] reuse opportunities`

---

## 1B: Best-Practice Research (conditional, right after scout)

Two DIFFERENT kinds of research — don't conflate them:
- **Reuse research (internal)** = 1A scout itself. Always first.
- **Best-practice research (external)** = how the wider world solves this. THIS step.

**Default position: immediately after scout, before Q&A.** Knowing the solution space up front
makes your questions sharper.

**The one exception — defer when intent is fuzzy.** After scout, run the discriminator:

> **"Do I already know WHAT I'm building, or only HOW?"**

- **WHAT is clear** (only "how" is open) → research now.
- **WHAT is fuzzy** (scope/intent unsettled) → ask ONE scope-fixing question FIRST, then research.

**Calibrate depth:**
- **Orienting pass (default):** just enough to know which approaches exist and which is canonical.
- **Deep-dive:** exact API semantics, pattern comparison. Reserve for a fork that genuinely
  affects the design AND is still open. May also come LATER as a top-up if Q&A opens a new fork.

**Only research at all when ALL three hold** (else skip):
1. Scout found NO in-repo precedent to copy.
2. The problem is well-trodden externally (undo/redo, caching, auth, retry, rate-limiting…).
3. The choice materially affects the design — a real fork, not a cosmetic detail.

Anchor findings to the design and cite them in the 1D approaches. Prefer the codebase's own
conventions + large-system precedents over novel invention.

**Output:** `✓ Step 1B: Research [done - N sources | skipped - <reason>]`

---

## 1C: Q&A Loop (one question at a time)

**Rules:**
- ONE question per message — never batch.
- Multi-choice options preferred (3-5 options + free-text), with **your recommendation** marked.
- Each question targets the **biggest current gap** in your understanding.
- After each answer, decide: do I need to re-explore the code? If yes, mini-explore (1-3 actions)
  BEFORE the next question.
- Brutal honesty injection: at least once, challenge the user's framing or preferred direction
  with a "what if X were not true?" question.

**Question targeting (biggest gap first):**
| Gap | Sample Question |
|---|---|
| Goal unclear | "When you say 'audit log', what does an admin DO with it — verify integrity, debug user issues, satisfy compliance?" |
| Constraints unclear | "Volume: ~100 events/day or ~10k? Affects pagination strategy." |
| Success criteria unclear | "If I showed you the finished page, what would make you say 'yes, exactly'?" |
| Scope drift risk | "You mentioned X. Is that v1 or v2?" |

**Round cap: 8.** If still unclear at round 8, present what you have and ask the user to fill
blanks directly. Do NOT spin past the cap.

**Scope check (continuous):** if the request describes 3+ independent subsystems, STOP and help
decompose into sub-projects. Cook only runs the first one.

**Output:** `✓ Step 1C: Q&A complete - [N] rounds`

---

## 1D: Propose 2-3 Approaches

```
**Approach A — [name]** (RECOMMENDED)
- Pros: ...
- Cons: ...
- Effort: low/medium/high

**Approach B — [name]**
- Pros: ...
- Cons: ...

**Approach C — [name]** (only if it adds a new dimension)
- Pros: ...
- Cons: ...
```

Lead with your recommendation and the reason. If the user prefers a non-recommended option,
**brutally honest pushback** — but accept the user's call if they reconfirm with reason.

**Output:** `✓ Step 1D: Approach selected - [name]`

---

## 1E: Sectioned Design (per-section approval)

Present the design in sections, get **explicit approval after each section** before moving on.

Standard sections (scale each to its complexity — a few sentences if simple, 200-300 words if nuanced):
1. **Architecture** — high-level components and how they fit
2. **Components / Files** — concrete files to create/modify, REUSING existing infra from scout
3. **Data flow / Types** — request/response shapes, type contracts
4. **Error handling**
5. **Testing & rollout** — what tests, what feature flag (if any), rollback strategy

Between sections, ask: *"Does this section look right? Any changes before we move to the next?"*

**On final approval, assemble the Approved Design Block** (kept in working memory, written
verbatim into the Step 2 plan files by the orchestrator — this replaces `spec.md`; the plan files
themselves follow `ck-plan`'s format, see `workflow-steps.md` § Step 2):

```markdown
## Context
Problem statement + scout findings (what existing infra is being reused).

## Goal
One-sentence objective.

## Constraints
- ...

## Non-Goals
- ...

## Approach
Selected approach from 1D, with rationale (cite 1B research if used).

## Architecture / Components & Files / Data Flow & Types / Error Handling / Testing & Rollout
(The five approved sections, verbatim.)

## Acceptance Criteria
- [ ] Concrete, testable items

## Open Items
- Genuinely deferred decisions.
```

**Output:** `✓ Step 1E: Design approved - [N] sections, [M] acceptance criteria`

---

## Escalation

- **User says "just code it" / "skip planning"** → respect it. Collapse Step 1 to: minimal scout
  (≤3 actions) + one confirmation question + a 5-line design block, then go to Step 2.
- **Bug fix handed off by `routing-bug-fixes-to-cook`** (root cause confirmed) → same collapse, with
  the investigation as the scout: Q&A only for a genuinely open question, the candidate fixes with a
  recommendation + a 5-line design block, one approval, then Step 2.
- **Topic too large** → decompose into sub-projects; cook only the first.
- **Scope keeps drifting** → freeze scope explicitly; document drifted items as Open Items.
- **No clear recommendation possible** → present approaches with trade-offs, ask the user to
  pick, do NOT push.

## Design Step Checklist

- [ ] 1A scout actions logged (≥5 actions; cited in later questions)
- [ ] 1B research done when warranted (all 3 conditions), or consciously skipped with reason
- [ ] Q&A respected one-per-round rule, max 8 rounds
- [ ] Mid-stream re-explore happened at least once if an answer reshaped scope
- [ ] At least one brutal honesty pushback on premise/assumption
- [ ] 2-3 approaches presented with trade-offs + recommendation
- [ ] Each design section approved before the next
- [ ] Approved Design Block assembled (with Acceptance Criteria)
- [ ] No implementation code written during Step 1
- [ ] No `spec.md` written and no handoff to `ck-plan` — Step 2 continues in this run
