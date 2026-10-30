# Unified Workflow Steps

Single workflow. Step 1 (Design) is run by the orchestrator; every implementation phase is dispatched to a sub-agent — the orchestrator never edits production code in-line during Step 3.

**ALWAYS create a task list.** A task list MUST always be created for every run — no exceptions. Use `TaskCreate` up front and keep it updated via `TaskUpdate` throughout.

**Task Tool Fallback:** `TaskCreate`/`TaskUpdate`/`TaskGet`/`TaskList` are CLI-only — unavailable in VSCode extension. If they error, still create the task list via `TodoWrite`. The task list is never skipped; only the tool used to hold it changes.

## Step 0: Setup

1. Parse input.
2. If input is a path to `plan.md` or `phase-*.md`, set active plan and skip Steps 1–2.
3. Otherwise continue from Step 1 (Design) — see `design-phase.md`.
4. **ALWAYS** use `TaskCreate` to seed workflow step tasks (with dependencies if complex). This is mandatory — a task list must exist before proceeding. Fall back to `TodoWrite` only if Task tools error.

**Output:** `✓ Step 0: Setup complete - [plan-loaded | full-workflow]`

## Step 1: Design (skip if plan path provided)

Full detail in **`design-phase.md`** — this is the `brainstorm-pro` flow run in-line, through its
design stage only. Summary:

| Sub-step | What happens | Delegation |
|---|---|---|
| 1A Deep Scout | CLAUDE.md + rules, related dirs/features, cross-repo, docs, user memory. Produce reuse opportunities / conflict risks / open questions. | Orchestrator; `document-specialist`/`Explore` sub-agents optional for parallel reading (≤150-line reports) |
| 1B Best-Practice Research | Conditional — only if no in-repo precedent AND well-trodden externally AND a real design fork. Orienting pass by default. | Orchestrator; `document-specialist` / context7 / WebSearch optional |
| 1C Q&A Loop | ONE round at a time (one question, or up to 4 independent ones) on a local HTML question page (`../brainstorm-pro/references/html-interview.md`): full card, multi-choice + recommendation, answers sent back from the page (or pasted), mini re-explore between rounds, ≥1 brutal-honesty pushback. **Cap 8 rounds.** | Orchestrator only |
| 1D Approaches | 2-3 approaches with pros/cons/effort, recommendation first — the last question on the interview page. | Orchestrator only |
| 1E Design review | The design written to `<plan-dir>/design.md` (brainstorm-pro's spec structure), self-reviewed, shown on a review page (`--serve`) — **notes per section, then Duyệt**. | Orchestrator only |

**This step is interactive.** The "no approval stops" rule does NOT apply here — it starts at Step 2.

**Terminal difference vs `brainstorm-pro`:** no `spec.md` is written and the `ck-plan` skill is NOT invoked.
The approved `<plan-dir>/design.md` is the design; Step 2 writes the plan next to it.

**Output:** `✓ Step 1: Design approved - scout [N] actions, Q&A [M] rounds, [K] sections, [J] acceptance criteria`

## Step 2: Planning (orchestrator-run — skip if plan path provided)

<HARD-GATE>
The orchestrator writes the plan ITSELF using Write. Do NOT dispatch a `planner` (or any other)
sub-agent for this step. The orchestrator just held the design conversation — it has the scout
findings, the Q&A answers, and the design review's notes in context. A sub-agent has none of that
and would re-derive (and distort) requirements the user already settled. Sub-agent delegation begins
at Step 3.
</HARD-GATE>

### Format: reuse `ck-plan`'s (do NOT invent one)

Cook does not delegate planning, but it does not define its own plan format either. **Load these
two files BEFORE writing anything** and follow them:

- `../ck-plan/references/plan-organization.md` — plan dir naming + layout, `plan.md` skeleton,
  phase-file section set, status-update rules
- `../ck-plan/references/output-standards.md` — frontmatter schema + auto-population rules, tag
  vocabulary, writing style

Concretely that means:

- **Plan dir:** resolve it per ck-plan's SKILL.md § "Plan Directory Resolution" — an explicit path
  in the arguments wins, otherwise `{plan-root}/{YYYY-MM-DD}-{kebab-slug}/` where `{plan-root}` is
  the project's existing plan directory (`plans/`, else `ai_docs/`, else `docs/plans/`). Never in
  the user's home directory — always the current project.
- **Write the files yourself** with `Write`. There is no scaffolding CLI. The orchestrator owns the
  `## Phases` table end to end: one row per `phase-XX-*.md`, same order, human-readable link text,
  `Status` starting at `Pending`. Adding a phase = write the file, insert the row.
- **`plan.md`:** required YAML frontmatter (`title`, `description`, `status: pending`, `priority`,
  `effort`, `issue`, `branch` = `git branch --show-current`, `tags`, `created`), then `## Overview`,
  `## Phases` table with **human-readable link text** (`[Setup Environment](./phase-01-setup.md)`,
  not the filename), `## Dependencies`. Keep the generic part under 80 lines.
- **`phase-XX-*.md`:** ck-plan's section set — Context Links, Overview, Key Insights, Requirements,
  Architecture, Related Code Files, Implementation Steps, Todo List, Success Criteria, Risk
  Assessment, Security Considerations, Next Steps. Omit a section only when it is genuinely empty
  for that phase; never pad it with filler.
- **Writing style:** ck-plan's — bullets, short sentences, concision over grammar.

**Where cook's own artifacts live in that format:**

| Cook concept | Goes into |
|---|---|
| Approved design (`design.md`, already in the plan dir) | `plan.md` links it right after `## Overview` (`Design: [design.md](./design.md)`) — never copy it in |
| Acceptance criteria (`design.md` § Tiêu chí hoàn thành, verbatim) | `plan.md` `## Acceptance Criteria` — Step 5 Section A verifies against it, so it is the authoritative contract |
| Per-phase "Done when" | that phase file's **Success Criteria** section |
| Per-phase file ownership | that phase file's **Related Code Files** section — must be disjoint across phases marked parallel |
| Open Items | `plan.md` `## Open Items` |

Scope/file-ownership/Success-Criteria blocks are passed **verbatim** into each Step 3 sub-agent
prompt, so write them to be read by an agent with no other context.

**ck-plan rules that do NOT carry over:**

- No `AskUserQuestion` for unresolved questions after the plan is written — cook never stops for the
  user from Step 2 onward. Genuinely deferred decisions go to `## Open Items`.
- No "create plans only, do not implement" — cook continues into Step 3 in the same run.
- No research / red-team / validate subcommands, and no separate task hydration here
  (Step 3 owns the task list).
- Still no `spec.md`, no `ck-plan` invocation, no `planner` sub-agent.

### Other Step 2 rules

- Keep the phase count honest to the work: a small, single-file-cluster change is ONE phase. Do not
  inflate phases to look thorough.
- Plans are local working artifacts — write them, never commit them.

**Plan self-review (5 checks, orchestrator, no user gate):** after writing the plan, re-read it
once —
1. **Placeholder scan** — any `TBD`/`TODO`/vague step? Fix inline or move to Open Items.
2. **Coverage** — every acceptance criterion from the design maps to at least one phase.
3. **Internal consistency** — phase file ownership disjoint where phases are marked parallel;
   dependencies declared.
4. **Ambiguity** — any phase step interpretable two ways? Pick one, make it explicit.
5. **Format** — `plan.md` has complete frontmatter, a Phases table with human-readable links, and
   `## Acceptance Criteria`; every phase file has Success Criteria + Related Code Files.

Fix issues inline (plan files only — never production code). No re-review loop.

**Output:** `✓ Step 2: Plan created - [N] phases - ck-plan format - self-review [clean | K fixes]`

Report the plan overview with its phases and continue straight to implementation — do not stop for approval.

## Step 3: Implementation (sub-agent per phase — MANDATORY)

**Task hydration:**
1. `TaskList` first — check for existing tasks (hydrated by planning skill in same session).
2. If tasks exist → pick them up, skip re-creation.
3. If no tasks → **ALWAYS** `TaskCreate` for each unchecked `[ ]` item (read plan phases) with priority order and metadata (`phase`, `planDir`, `phaseFile`). Never proceed to dispatch without a task list.
4. Tasks can be blocked by other tasks via `addBlockedBy`.

Before dispatching phase 1, record `git rev-parse HEAD` and write the printed commit id down literally (shell variables do not survive between calls) — Step 5's size check needs it when the work is committed on the base branch itself.

**Dispatch rules:**
- **One sub-agent per phase.** Read the `phase-XX-*.md` file, extract scope/files/acceptance criteria, embed verbatim into the sub-agent prompt.
- **MANDATORY `TaskUpdate` `in_progress` BEFORE every dispatch.** Mark the phase task `in_progress` immediately before spawning its sub-agent — never dispatch a phase whose task is still `pending`.
- **Parallel when independent.** Phases with disjoint file ownership are dispatched in a single message with multiple `Agent` tool calls.
- **Sequential when dependent.** Wait for prerequisite phase to return before dispatching the next.
- **File ownership boundaries** MUST be in the prompt to prevent concurrent collisions.
- **Frontend phases** → `designer` subagent. **Backend / general phases** → `executor`. Verify both names against the session's agent registry first (see `subagent-patterns.md` § Agent Name Mapping).
- **MANDATORY `TaskUpdate` `complete` AFTER every phase returns.** The moment a phase sub-agent returns and its type-check/lint pass, `TaskUpdate` that task to `complete` — in the SAME turn, before dispatching or reporting anything else. Do NOT batch these updates to the end of the workflow.

<HARD-GATE>
The task list MUST be kept in lock-step with reality after EVERY phase: `in_progress` before dispatch, `complete` right after return. If you are about to dispatch the next phase (or move to Step 4) and the previous phase's task is not yet marked `complete`, STOP and update it first. A phase is not "done" until its task status reflects it.
</HARD-GATE>

**Forbidden:** orchestrator using Edit/Write on production files during Step 3. If a sub-agent fails or returns incomplete work, dispatch another sub-agent (`debugger` / `executor`) — do not patch in-line.

**Output:** `✓ Step 3: Implemented [N] phases via [M] sub-agent dispatches - [X/Y] tasks complete`

## Step 4: Testing

- **MUST** spawn a testing subagent: `Agent(subagent_type="test-engineer", prompt="Run test suite", description="Run tests")`.
- If failures: **MUST** spawn `debugger` subagent → fix dispatch → repeat.
- Tests cover happy path, edge cases, errors.
- **Forbidden:** fake mocks, commented tests, changed assertions, skipping subagent delegation.

**Output:** `✓ Step 4: Tests [X/X passed] - testing subagent invoked`

Report the test results and dispatch the Step 5 review in the same turn — do not stop for approval. Tests must be 100% green first; if they are not, fix them before proceeding.

## Step 5: Combined Review — sized by the diff (MANDATORY)

**Size check first.** Base = `git merge-base <base-branch> HEAD` (when the work is committed on the base branch itself, the commit recorded before Step 3). Run `git diff --numstat <base> -- . ':(exclude,glob)**/tests/**' ':(exclude,glob)**/__tests__/**' ':(exclude,glob)**/*.test.*' ':(exclude,glob)**/*.spec.*'` and add the line count of each untracked file from `git ls-files -o --exclude-standard` with the same excludes. A binary row (`-  -`) counts as a file with 0 lines. The diff is **small** when added + removed ≤ 80 lines AND ≤ 3 files; otherwise **large**. State the numbers in the Step 5 output line.

| Diff | Dispatch |
|------|----------|
| Small | **One** `code-reviewer` with Dispatch 1/2's prompt, changed in three places: "Return TWO sections — both required, no exceptions" becomes "Return THREE sections — all three required, no exceptions"; a `### Cleanup (non-gating, no severity)` section is added, built from Dispatch 2/2's "Project conventions are binding" paragraph, the four lens blocks (R first, with the `Reuse searched:` line) and the Collapse block; and the prompt says a correctness defect a lens finds goes straight into Section B, never into Cleanup. It does NOT get Dispatch 2/2's role paragraph or its second "Diff under review" block |
| Large | **Two** fresh `code-reviewer`s **in one message** so they run concurrently, as below |

| # | Agent | Lens | Gating? |
|---|-------|------|---------|
| 1 | `code-reviewer` | Section A plan-compliance + Section B bugs by severity | **YES** |
| 2 | `code-reviewer` | **Reuse + Simplification + Efficiency + Altitude** — one agent, four lenses, Reuse first | no |

Agent 2 runs the four cleanup lenses of the `simplify` skill as a review-only sub-agent. Use `code-reviewer` for every reviewer: it is read-only (no Write/Edit), so no reviewer can violate the Step 3 gate. Fall back to `critic`, then `general-purpose`.

**Model:** dispatch every reviewer with `model: "opus"` — this is the post-implementation verification pass.

**Why size the review:** on small bug fixes the two reviewers cost $6-17 against $1-4 for the executor, and every re-review repeated the whole change. A diff of a few lines gives the cleanup lenses little to find, so one reviewer holding all of it is enough; the second agent pays off only when there is enough code for the lenses to work on.

**Why the four cleanup lenses share one agent:** they work off the same working set — the changed files, the mechanism beneath them, and the shared modules they could have used — and their fixes subsume each other. Copied code is both a Reuse finding ("call the existing helper") and a Simplification finding ("this duplicates X"). The Altitude fix routinely absorbs a Simplification and an Efficiency finding: repair the shared mechanism and the special cases, the duplication and the redundant work all vanish together. One agent holding all four can report "these are one change"; isolated agents cannot, and the orchestrator — which sees only their summaries, not the code — collapses them badly.

**Why Reuse no longer gets its own agent:** it was split out as the search-heavy lens, but on past cook runs the separate Simplification+Efficiency+Altitude agent made just as many tool calls (median 40 each) and read many of the same files, and about a third of the Reuse agent's findings came back as Simplification findings for the orchestrator to dedup. Reuse findings are still the highest-value ones — they **delete** code — so the merged prompt runs the Reuse pass first and requires a `Reuse searched:` line, which keeps that search from being skipped.

**Why not one agent per lens like `simplify` does:** `simplify` dedups **and applies** the fixes in the same head, so the cross-lens collapse just moves from review time to apply time. Cook forbids that (§ Forbidden: fixes always go to a sub-agent), so in cook the collapse would land in the orchestrator working from summaries — the weakest possible place. One cleanup reviewer keeps the synthesis where the code understanding is.

**Why one dispatch instead of two steps:** same wall-clock as the old single reviewer, and one gate for the user instead of two. The cost is that gating and non-gating findings arrive together — § Verdict below is what keeps them from turning to mush. Read it before acting on any finding.

**Why quality is a separate lens from bugs:** agent 2 is looking at reuse/complexity/efficiency/depth, NOT correctness. Agent 1 owns bugs. Do not let the cleanup agent argue a correctness defect — if it does, re-file it under Section B severity and treat it as gating.

### Dispatch 1/2 — compliance + bugs (gating)

```
Agent(
  subagent_type="code-reviewer",
  description="Combined compliance + quality review",
  prompt="""
  Dual-section review of the implementation against the plan.

  ## Plan files (authoritative)
  - <plan-dir>/plan.md  (carries the approved design's Acceptance Criteria)
  - <plan-dir>/design.md  (the approved design, when cook wrote it)
  - <plan-dir>/spec.md  (only if the plan was authored outside cook and one exists)
  - <plan-dir>/phase-01-*.md, phase-02-*.md, ... (list ALL phase files verbatim)

  ## Implementer reports
  <paste each phase sub-agent's final report>

  ## Diff under review
  Diff against the same base as the size check (the merge-base with
  <base-branch>, or the commit recorded at the start of Step 3 when the work
  is on the base branch itself), plus uncommitted and untracked changes
  (`git status`, `git ls-files -o --exclude-standard`). List the changed
  files and read them.

  ## CRITICAL: do not trust the implementer reports
  Read the actual code at HEAD. Phase sub-agents may have skipped,
  misinterpreted, or over-built relative to the plan.

  ## Return TWO sections — both required, no exceptions

  ### Section A — Plan Compliance
  For every § Acceptance Criteria checkbox in plan.md (or spec.md's § Tiêu chí hoàn thành /
  Acceptance Criteria if present)
  AND every phase "Done when" item:
    1. List the criterion verbatim.
    2. Mark ✅ / ❌ MISSING / ❌ WRONG / ⚠ EXTRA.
    3. Cite file:line proving the verdict.
  This section MUST be exhaustive — every AC, no skips. Also flag work
  that lives outside the file ownership declared in the plan.

  ### Section B — Code Quality (severity-rated)
  Standard severity-rated review of the diff:
    - **Critical:** security, correctness defects, OWASP, data loss, crashes
    - **High:** logic bugs, missed edge cases, SOLID violations, regressions
    - **Medium:** maintainability, naming, duplication, weak tests
    - **Low:** nits, style, comment quality
  For each finding, cite file:line and propose a concrete fix.

  ## Final verdict
  - ✅ APPROVED — Section A all ✅, Section B has no Critical and no High
  - ❌ CHANGES REQUIRED — list specifically what blocks approval and why
  """
)
```

### Dispatch 2/2 — cleanup lenses (non-gating)

Large diff only: same message as dispatch 1, with the same plan files + diff scope, plus the role paragraph, the conventions paragraph and all four lens blocks below. (The small path is defined once, at the dispatch table above.) It returns findings as `file`, `line`, one-line `summary`, and **the concrete cost** — what is duplicated, wasted, or made harder to maintain — plus the `Reuse searched:` line. No severity ratings; these are not bugs.

Role paragraph (agent 2 only):

```
  Quality review of the diff below — you are improving code quality, NOT hunting
  for correctness bugs. Another reviewer owns bugs; do not duplicate that work.
  If you believe you found a real correctness defect (wrong output, crash, data
  loss, security hole, or work that fails outright with realistic data rather
  than merely wasting it), say so explicitly and label it OUT-OF-LENS rather
  than reporting it as a cleanup finding.

  ## Diff under review
  Diff against the same base as the size check (the merge-base with
  <base-branch>, or the commit recorded at the start of Step 3 when the work
  is on the base branch itself), plus uncommitted and untracked changes
  (`git status`, `git ls-files -o --exclude-standard`). Read the actual files,
  not just the hunks.
```

Conventions paragraph (shared with the small path's Cleanup section):

```
  ## Project conventions are binding
  Read the repo's CLAUDE.md / AGENTS.md FIRST. A "simplification" that breaks a
  documented invariant is not a finding — it is a defect you would be
  introducing. Some invariants look redundant on purpose and say so in prose.
  When a convention and your lens disagree, the convention wins; report the
  tension instead of a fix.

  Return findings only. Do NOT edit any file.
```

Lens blocks (all four, in this order):
```
  ## Lens R: Reuse — do this pass first
  Flag new code that re-implements something the codebase already has. Grep the
  shared/utility modules and the files adjacent to the change, and NAME the
  existing helper to call instead — a finding without a named replacement is not
  actionable, so drop it.

  Search before you start the other lenses: the shared libs, the sibling
  modules, and the barrel/index files the changed code could have imported from.
  Reading the diff alone is not a Reuse pass. Your report MUST contain a line
  `Reuse searched: <modules and directories you checked>`, whether or not you
  found anything.

  ## Lens A: Simplification
  Flag unnecessary complexity the diff adds: redundant or derivable state,
  copy-paste with slight variation, deep nesting, dead code left behind. Name
  the simpler form that does the same job.

  ## Lens B: Efficiency
  Flag wasted work the diff introduces: redundant computation or repeated I/O,
  independent operations run sequentially, blocking work added to startup or hot
  paths. Also flag long-lived objects built from closures or captured
  environments — they keep the entire enclosing scope alive for the object's
  lifetime, a leak when that scope holds large values; prefer a class/struct
  that copies only the fields it needs. Name the cheaper alternative.

  ## Lens C: Altitude
  Check that each change fixes the root cause at the right depth rather than
  patching a symptom with a fragile bandaid. Special cases layered on shared
  infrastructure are a sign the fix is not deep enough — prefer the simpler,
  more general change to the underlying mechanism over adding special cases, and
  name that change.

  ## Collapse before you report — this is why you hold all four lenses
  These lenses overlap on purpose. Copied code is both a Reuse and a
  Simplification finding; an Altitude fix often removes a Simplification and an
  Efficiency finding. Before returning, merge findings that are ONE change and
  say so explicitly: "Lens A #2 and Lens B #1 are the same fix as Lens C #1 —
  repairing <mechanism> removes both." Report the change once, list what it
  subsumes, and do NOT also report the subsumed findings separately.

  Reuse and Altitude outrank Simplification and Efficiency. If a Simplification
  or Efficiency fix would polish code that Reuse says to replace with an
  existing helper, or that Altitude says should be deleted or generalised away,
  drop it — applying it is wasted work and makes the code look intentional,
  which is harder to delete later.

  If that higher fix changes behaviour or reaches beyond the diff, the
  orchestrator may skip it. Then put the smallest in-diff fix for the same
  problem under it as `Fallback:` instead of dropping it.

  Tag every finding with its lens (R / A / B / C) so the orchestrator can bucket it.
```

### Verdict — gating vs non-gating

Wait for every reviewer, then split the findings. This split is the whole point of the step; do not blend them into one score.

| Bucket | Source | Effect |
|--------|--------|--------|
| **Blocking** | Section A any ❌ | **Non-overridable.** Dispatch a fix sub-agent or abort. Nothing else. |
| **Blocking** | Section B Critical or High | Dispatch a fix sub-agent, cycle++ |
| **Non-blocking** | Section B Medium / Low | Apply what is worth applying; state each skip and why |
| **Non-blocking** | Agent 2, or the Cleanup section of the single reviewer (all four lenses) | Same — apply-or-skip-with-reason. **Never** blocks Step 6, **never** increments the cycle counter |

**Dedup across every report before applying.** The cleanup reviewer has already collapsed its own four lenses; do not undo that by re-splitting its merged findings. Findings pointing at the same line or the same mechanism collapse into one; when a cleanup lens and Section B overlap, keep Section B's (it carries the severity that drives the gate). When you skip a cleanup finding for behaviour change or scope and it carries a `Fallback:`, judge the fallback on its own.

**Apply blocking and non-blocking fixes in the SAME dispatch**, blocking first. Tell the applier that blocking fixes take precedence and to **drop any cleanup finding that the blocking fix already resolved or made stale** — otherwise it patches code that no longer exists.

Applier: `code-simplifier` for a cleanup-only batch (it is purpose-built for this and preserves behaviour), `executor` for anything containing a blocking fix, `debugger` when the blocking fix needs root-cause work first. Dispatch with `model: "sonnet"`.

### After applying — re-verify (MANDATORY)

Cleanup edits change working code, so the step is NOT done when the fixes land:

1. Re-run Step 4's test command. Must be back to 100%.
2. Run the project's own verification gate if it declares one — the command its CLAUDE.md says must stay green (`pnpm verify` in talos-v2). Lint + typecheck + unit tests passing is **not** the same thing; a repo-level gate catches invariant violations that neither reviewer sees.
3. If either fails, the cleanup caused it. Revert the offending finding rather than layering a fix on it, and record it as skipped.

Skipping this is how a "quality pass" ships a regression. `simplify` on its own has no verification phase — that gap is closed here, not inherited.

**Cycle (max 3):** as defined in `review-cycle.md`. **Re-dispatch only the gating reviewer** (agent 1, or the single reviewer without its Cleanup section) on every re-review — the cleanup lenses run **once**, on the first pass. Re-running them per cycle repeats the review cost for findings you already hold.

**Scoped re-review (every re-review, both diff sizes).** Take a tree snapshot before the fix batch, and a second one after re-verify and any revert (§ After applying, step 3):

```
IDX=$(mktemp -u); G=$(git rev-parse --git-path index); [ -f "$G" ] && cp "$G" "$IDX"; GIT_INDEX_FILE=$IDX git add -A; GIT_INDEX_FILE=$IDX git write-tree; rm -f "$IDX"
```

It does not touch the real index and covers tracked edits, untracked files and commits. Write down the two printed tree ids literally — shell variables do not survive between Bash calls. The delta is `git diff <before-tree> <after-tree>`.

Scoped re-review — replaces Dispatch 1/2 on every re-review (cycle > 0):

```
Agent(
  subagent_type="code-reviewer",
  description="Scoped re-review",
  prompt="""
  Scoped re-review of a fix batch. Do not re-review the rest of the change and
  do not re-run Section A over criteria the fix did not touch.

  ## Findings the batch fixed (verbatim)
  <paste each blocking finding>

  ## Delta
  `git diff <before-tree> <after-tree>`

  ## Return
  - For each finding above: RESOLVED / NOT RESOLVED, with file:line.
  - New issues only in the changed lines, graded on Dispatch 1/2's
    Critical / High / Medium / Low scale, each with file:line and a fix.
  - Final verdict as in Dispatch 1/2: APPROVED (every finding RESOLVED, no new
    Critical or High) or CHANGES REQUIRED.
  """
)
```

**Forbidden:** patching code in-line based on the review. Fixes are always dispatched to a sub-agent. Running the cleanup lenses more than once per cook run. Letting a cleanup finding block Step 6.

**Output:** `✓ Step 5: Review ([N] lines / [M] files → [1|2] reviewer[s]) — Section A [✅|❌ N gaps] / Section B [score]/10 / Cleanup [N applied, M skipped] — re-verify [✅|❌] — [Approved|Awaiting fix]`

## Step 6: Finalize

**MANDATORY subagents (NON-NEGOTIABLE):**
1. **MUST** spawn these subagents in parallel:
   - `Agent(subagent_type="executor", prompt="Run full sync-back for [plan-path]: reconcile all completed Claude Tasks with all phase files, backfill stale completed checkboxes across every phase, then update plan.md frontmatter/table progress. Do NOT only mark current phase.", description="Update plan")`
   - `Agent(subagent_type="writer", prompt="Update docs for changes.", description="Update docs")`
2. Sync-back MUST include:

### Status Sync (Finalize)

There is no status CLI — every update is a plain `Edit`. Two places must agree:

| Where | What changes |
|-------|--------------|
| `phase-XX-*.md` todo list | `[ ]` → `[x]` for each completed item |
| `plan.md` `## Phases` table | that phase's **Status cell only** — `Pending` → `In Progress` → `Complete`. Preserve table structure; never rewrite the whole table |
| `plan.md` frontmatter | `status:` = `in-progress` while any phase is open, `completed` when all are `Complete` |

**Derive status from the checkboxes, not from memory.** A phase is `Complete` only when every todo
box in its file is ticked. Re-read the phase file before writing its row.
   - Sweep all `phase-XX-*.md` files in the plan directory.
   - Mark every completed item `[ ] → [x]` based on completed tasks (including earlier phases finished before current phase).
   - Update `plan.md` status/progress (`pending`/`in-progress`/`completed`) from actual checkbox state.
   - Return unresolved mappings if any completed task cannot be matched to a phase file.
3. Use `TaskUpdate` to mark Claude Tasks complete after sync-back confirmation.
4. Onboarding check (API keys, env vars).
5. Ask whether to commit (and push) on the interview page (`../brainstorm-pro/references/html-interview.md`), not in chat; if yes, **MUST** spawn: `Agent(subagent_type="git-master", prompt="Stage and commit changes; push if the user chose it", description="Commit")`.

**CRITICAL:** Step 6 is INCOMPLETE without spawning all required subagents. DO NOT skip subagent delegation.

**Output:** `✓ Step 6: Finalized - [N] subagents invoked - Full-plan sync-back completed - Committed`

## Workflow Summary

Legend: `[R]` = human gate

```
0 → 1A scout → 1B research → 1C Q&A [R per round] → 1D approaches → 1E design.md review page [R, notes per section]
  → 2 (orchestrator writes plan + self-review) → 3 (sub-agent per phase) → 4 (test sub-agent) → 5 (review sized by the diff: 1 or 2 reviewers) → 6 (finalize sub-agents)
```

All human gates live in Step 1. Steps 2–6 run straight through; only the final commit is confirmed.
If a plan path is provided at Step 0, skip Steps 1–2 and start at Step 3.

## Critical Rules

- Never skip steps without explicit user instruction.
- **STEP 1 IS NOT DELEGATED.** The design conversation (1C Q&A, 1D approaches, 1E design review) is run by the orchestrator. Sub-agents assist with reading in 1A/1B only. Never write `spec.md`, never invoke `ck-plan` — Step 2 continues in this run.
- **MANDATORY SUBAGENT DELEGATION:** Steps 3, 4, 5, 6 MUST spawn subagents via the `Agent` tool. DO NOT implement, test, or review directly. Resolve every type through `subagent-patterns.md` § Agent Name Mapping first.
  - Step 3: `executor` / `designer` for frontend (one per phase)
  - Step 4: `test-engineer` (and `debugger` if failures)
  - Step 5: `code-reviewer` sized by the diff (§ Step 5) — small: one reviewer returning Section A, Section B and a Cleanup section; large: two in ONE message — agent 1 returns the dual-section gating report (Section A plan-compliance, non-overridable + Section B bugs by severity); agent 2 runs Reuse (first) + Simplification + Efficiency + Altitude and collapses findings that are one change. Cleanup lenses never gate. Re-verify after any fix batch.
  - Step 6: `executor` (sync-back), `writer` (docs), `git-master` (commit)
- **ALWAYS** create a task list — use `TaskCreate` for each unchecked item with priority order and dependencies (or `TodoWrite` if Task tools unavailable). This is never skipped.
- **ALWAYS** keep the task list updated in lock-step, not just at the end:
  - `TaskUpdate` → `in_progress` immediately BEFORE dispatching each phase's sub-agent.
  - `TaskUpdate` → `complete` immediately AFTER that sub-agent returns and verification passes — in the same turn, one phase at a time. Never batch status updates to the end of the run.
  - (Fall back to `TodoWrite` for these updates only if Task tools error — but the update still happens.)
- All step outputs follow format: `✓ Step [N]: [status] - [metrics]`
- **VALIDATION:** The workflow is INCOMPLETE if (a) `Agent` tool calls = 0 for Step 3, Step 4, OR Step 5, OR (b) any phase finished without a matching `TaskUpdate` → `complete`, OR (c) the task list still shows `pending`/`in_progress` items for work that is actually done.
