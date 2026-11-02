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

Full detail in **`design-phase.md`** (it runs `~/.claude/skills/brainstorm-pro/references/design-flow.md`) — this is the `brainstorm-pro` flow run in-line, through its
design stage only. Summary:

| Sub-step | What happens | Delegation |
|---|---|---|
| 1A Deep Scout | CLAUDE.md + rules, related dirs/features, cross-repo, docs, user memory. Produce reuse opportunities / conflict risks / open questions. | Orchestrator; `document-specialist`/`Explore` sub-agents optional for parallel reading (conclusions with citations, not file dumps) |
| 1B Best-Practice Research | Conditional — only if no in-repo precedent AND well-trodden externally AND a real design fork. Orienting pass by default. | Orchestrator; `document-specialist` / context7 / WebSearch optional |
| 1C Q&A Loop | ONE round at a time (one question, or up to 4 independent ones) on a local HTML question page (`../brainstorm-pro/references/html-interview.md`): full card, multi-choice + recommendation, answers sent back from the page (or pasted), mini re-explore between rounds, ≥1 brutal-honesty pushback, a playback card ("Thẻ phát lại", no recommendation) before the approaches. **Cap 8 rounds.** | Orchestrator only |
| 1D Approaches | 2-3 approaches with pros/cons/effort, recommendation first — the last question on the interview page. | Orchestrator only |
| 1E Design review | The design written to `<plan-dir>/design.md` (brainstorm-pro's spec structure), self-reviewed + builder check, shown on a review page (`--serve`) — **notes per section, then Duyệt**. | Orchestrator; builder check agent (`../brainstorm-pro/references/builder-check.md`) |

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
three files BEFORE writing anything** and follow them:

- `../ck-plan/references/plan-writing.md` — plan.md and phase-file templates, phase sizing, step
  rules, self-review
- `../ck-plan/references/plan-organization.md` — plan dir naming + layout, file creation order and the
  Phases table, status-update rules
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
- **`plan.md` and phase files:** `plan-writing.md` § plan.md and § Phase file; frontmatter per `output-standards.md` (`branch` = `git branch --show-current`).
- **Writing style:** ck-plan's — bullets, short complete sentences, every block readable on its own.

**Where cook's own artifacts live in that format:**

| Cook concept | Goes into |
|---|---|
| Approved design (`design.md`, already in the plan dir) | `plan.md` links it right after `## Overview` (`Design: [design.md](./design.md)`) — never copy it in |
| Acceptance criteria (`design.md` § Tiêu chí hoàn thành, verbatim) | `plan.md` `## Acceptance Criteria` — Step 5 Section A verifies against it, so it is the authoritative contract |
| Per-phase "Done when" | that phase file's **Success Criteria** section |
| Per-phase file ownership | that phase file's **Related Code Files** section — must be disjoint across phases marked parallel |
| AC traceability (design.md § Truy vết AC, when 1A saved stories) | plan.md `## AC Traceability`: AC, Text and Status copied verbatim, Where = the owning phase file; that phase's **Success Criteria** lists its row ids |
| Open Items | `plan.md` `## Open Items` |

Scope/file-ownership/Success-Criteria blocks are passed **verbatim** into each Step 3 sub-agent
prompt, so write them to be read by an agent with no other context.

**ck-plan rules that do NOT carry over:**

- No `AskUserQuestion` for unresolved questions after the plan is written — cook never stops for the
  user from Step 2 onward (exceptions: `../SKILL.md` § Gates). Genuinely deferred decisions go to `## Open Items`.
- No "create plans only, do not implement" — cook continues into Step 3 in the same run.
- No research / red-team / validate subcommands, and no separate task hydration here
  (Step 3 owns the task list).
- Still no `spec.md`, no `ck-plan` invocation, no `planner` sub-agent.

### Other Step 2 rules

- Size phases per `../ck-plan/references/plan-writing.md` § Sizing phases.
- Plans are local working artifacts — write them, never commit them.

**Plan self-review (orchestrator, no user gate):** run `../ck-plan/references/plan-writing.md` § Self-Review, plus one cook-only check: `plan.md` has `## Acceptance Criteria` matching design.md § Tiêu chí hoàn thành. Fix issues inline (plan files only — never production code). Report its result in the Output line below, not as a separate line.

**Output:** `✓ Step 2: Plan created - [N] phases - ck-plan format - self-review [clean | K fixes]`

Report the plan overview with its phases and continue straight to implementation — do not stop for approval.

## Step 3: Implementation (sub-agent per phase — MANDATORY)

**Task hydration:**
1. `TaskList` first — phase tasks exist only if cook already ran on this plan in this session (ck-plan creates none).
2. If phase tasks exist → pick them up, skip re-creation.
3. If no tasks → **ALWAYS** `TaskCreate` one task per phase file that still has an unchecked `[ ]` box, or has no box at all while its Phases-table Status is not `Complete` (not one per box), with priority order and metadata (`phase`, `planDir`, `phaseFile`). Never proceed to dispatch without a task list.
4. Tasks can be blocked by other tasks via `addBlockedBy`.

Before dispatching phase 1, for every repo the plan touches, record and write down literally (shell variables do not survive between calls): `git rev-parse HEAD` (the Step 3 start commit — Step 5's size check needs it when the work is committed on the base branch itself), `git status --porcelain --untracked-files=all` (the files with changes from before this run), and `git hash-object <file>` for each of those files that exists as a regular file (record a deleted one as `deleted`).

Cook commits on the current branch. If the user's rules forbid committing on it (e.g. `master`), first create the branch their rules name with `git switch -c <branch>` (it changes no file) and record that in `## Claude tự quyết`. Rules with no name pattern → `<ticket>-<slug>` (`<slug>` = the plan dir name without its date and ticket; no ticket → `<slug>` alone). A name that exists locally or on the remote → add `-2` (then `-3`, …). Cook never switches to an existing branch.

**Dispatch rules:**
- **One sub-agent per phase.** Read the `phase-XX-*.md` file, extract scope/files/acceptance criteria, embed verbatim into the sub-agent prompt; paste plan.md's Global Constraints first (§ Report contract, **Global Constraints** paragraph).
- **Every-prompt lines.** Append the two lines from `subagent-patterns.md` § Every prompt (Batch independent reads, Subagent boundary) to every sub-agent prompt cook writes. End every Step 3 prompt with the § Report contract block, verbatim, then those two lines.
- **MANDATORY `TaskUpdate` `in_progress` BEFORE every dispatch.** Mark the phase task `in_progress` immediately before spawning its sub-agent — never dispatch a phase whose task is still `pending`.
- **Parallel when independent.** Phases with disjoint file ownership are dispatched in a single message with multiple `Agent` tool calls.
- **Sequential when dependent.** Wait for prerequisite phase to return before dispatching the next.
- **File ownership boundaries** MUST be in the prompt to prevent concurrent collisions.
- **Frontend phases** → `designer` subagent. **Backend / general phases** → `executor`. Verify both names against the session's agent registry first (see `subagent-patterns.md` § Agent Name Mapping).
- **MANDATORY `TaskUpdate` `complete` once a phase is done.** As soon as a phase's report passes § Report check, status and commit and its commit lands (or nothing is left to commit, § Report check, status and commit), `TaskUpdate` that task to `complete` — in the SAME turn, before dispatching or reporting anything else. Do NOT batch these updates to the end of the workflow.

<HARD-GATE>
The task list MUST be kept in lock-step with reality after EVERY phase: `in_progress` before dispatch, `complete` once its report passed the check and its commit landed (or nothing was left to commit, § Report check, status and commit). A phase awaiting an answer, a re-dispatch or a split stays `in_progress` (once split, the original task is deleted, the **Status handling** list in § Report check, status and commit) and does not block dispatching phases that do not depend on it. If you are about to move to Step 4 and any phase's task is not `complete`, STOP and settle it first. A phase is not "done" until its task status reflects it.
</HARD-GATE>

**Forbidden:** orchestrator using Edit/Write on production files during Step 3. If a sub-agent fails or returns incomplete work, dispatch another sub-agent (`debugger` / `executor`) — do not patch in-line. Commits and the Step 6 push are bookkeeping, not edits. The orchestrator never runs a git command that changes file content (`revert`, `checkout`, `reset`, `stash`); the one exception is in Step 6: the fetch + merge (and `git merge --abort` on a conflict) the user's push rules require before a push.

### Report contract

Paste this block verbatim at the end of every Step 3 prompt, every fix prompt in Steps 4 and 5 (including the revert agent), and the Step 6 `writer` prompt. Templates below refer to it as `<Report contract block, verbatim>`.

**Global Constraints.** Paste plan.md's `## Global Constraints` section verbatim at the top of every
Step 3 prompt, every fix prompt in Steps 4 and 5, and the revert agent's prompt, under the heading
`## Global Constraints (apply to every change you make)`, then the phase file content (or the fix
brief). A plan without the section (older plans) gets nothing pasted.

For a fix batch, revert or docs agent, its prompt stands in for the phase file: the findings (Step 5) or the failing tests (Step 4), the change to undo, or "docs updated, or none needed: <why>" are its Done-when items. It owns every file it lists in `Files changed`, minus pre-run files, the plan root and `<plan-dir>` (the **Commit** rules in § Report check, status and commit) and forbidden paths; an unlisted changed file is not committed and is recorded in `## Claude tự quyết`. Order for every prompt cook writes: plan.md's Global Constraints first (Step 3, fix and revert prompts, per the **Global Constraints** paragraph), then the task, then the Report contract (where used), then the two § Every prompt lines.

```
## Report contract (required)
Do not commit, stage, stash, reset or check out anything: the orchestrator commits your files.
If the phase file and the design do not answer a question that changes what you build, stop and
report NEEDS_CONTEXT instead of guessing. Decide small details yourself and list them under Rulings.
End your reply with this report, in this order:

STATUS: DONE | DONE_WITH_CONCERNS | BLOCKED | NEEDS_CONTEXT
Done when:
- <each "Done when" item, verbatim> — <evidence: file:line, or test name and result; "not done: <why>">
Files changed:
- <every path you created, modified or deleted>
Tests: `<exact command>` → <pass/fail counts; on failure the failing tests and first error lines>
       (or "none — <why no test applies>")
Rulings: <each choice the phase file did not settle: what you chose and why; or "none">
Concerns: <DONE_WITH_CONCERNS only: what may be wrong and where>
Needs: <BLOCKED: what stops you and the split or context you propose;
        NEEDS_CONTEXT: the question, the options you see, the one you would pick>
```

### Report check, status and commit

**Check** — the moment an agent returns, before any commit:
- `STATUS` must be present and every Done-when item must carry evidence. An item with no evidence, marked "not done", or a failing `Tests` line means the phase is not done.
- Exception: a test that already fails on the base. The implementer reports DONE_WITH_CONCERNS with the test name and the reason. Accept it when neither the test file nor the code the test covers is in the phase's diff. Step 4's `test-engineer` confirms it on the base, and the orchestrator records it in `## Claude tự quyết`.
- Re-hash each pre-run file outside the plan root that exists as a regular file (`git hash-object`; a deleted one counts as `deleted`) and compare with the baseline: the hashes recorded at the start of Step 3. After each check the current hashes become the new baseline. Keep the Step 3 start hashes as a separate start record that never moves.
- Compare `Files changed` with `git status --porcelain --untracked-files=all`, after removing the pre-run file list, the plan dir (`<plan-dir>`, wherever the plan root is) and the files other returned phases listed (for a fix batch, revert or docs agent, ownership is as in § Report contract):
  - A file the phase changed without listing it (seen in porcelain, or for a pre-run file by a hash change since the last check) belongs to the phase whose ownership covers it (including owners still running); with no single owner it is not committed and is recorded in `## Claude tự quyết`.
  - A file outside the phase's ownership: if no other phase owns it, commit it with the phase and record it. If another phase owns it, do not commit it; a fresh agent reconciles before Step 4.

**Status handling:** every re-dispatch below is a fresh agent of the same type and model with the original task, what is done, and the new input, as for a `PARTIAL` report (`subagent-patterns.md` § Agent Tool Pattern); never `SendMessage`. Each status names only its new input.
- **DONE:** commit.
- **DONE_WITH_CONCERNS:** a concern about correctness or scope → a fresh agent fixes it before the commit. Any other concern → commit, then pass the concern to the reviewer (Implementer reports) and to `## Claude tự quyết`.
- **BLOCKED:** never retry unchanged. Split the phase, or dispatch a fresh agent with more context. When splitting, write the new phase files and `plan.md` rows the way Step 2 adds a phase, `TaskCreate` the new phases and then `TaskUpdate` the original task to status `deleted` (`TodoWrite`: remove the item), tell each new agent which partial edits already in the tree belong to it, and record the split in `## Claude tự quyết`.
- **NEEDS_CONTEXT:** see below.
- **Not done:** re-dispatch with the gap as the new input.
- Partial edits of an unfinished phase stay uncommitted; the fresh agent continues from them.

**NEEDS_CONTEXT:**
- If the design or plan answers it: re-dispatch with the answer and its source.
- Otherwise, by how hard the decision is to change:
  - **Hard to change** — what the interview card would mark `reversible: false` (`../brainstorm-pro/references/html-interview.md`, card fields: data shape or migration, public API or contract with BE, anything users depend on), and anything hard to undo after a push: ask the user on the interview page. Dispatch every phase that does not depend on the answer BEFORE serving the page; dependent phases wait. After the answer, re-dispatch with the answer. The user's answer is not a `## Claude tự quyết` row.
  - **Everything else:** Claude rules, records it in `## Claude tự quyết`, and dispatches a fresh agent with the ruling.

**Commit** — the orchestrator makes one commit per phase as soon as it passes the check; phases returning together are committed in phase-number order:

```bash
git -C <repo> add -- <file> …
git -C <repo> commit -m "<subject>" -- <file> …
```
- Commit only the paths chosen in the check. Never `git add -A` or `git add .`. Never a path under the plan root (ck-plan SKILL.md § Plan Directory Resolution: `plans/`, else `ai_docs/`, else `docs/plans/`, in the repo being committed) or under `<plan-dir>` itself wherever it sits, a file with changes from before Step 3, or a path the user's rules forbid committing.
- Where the user's rules require a commit identity for a repo, add `-c user.name=… -c user.email=…` accordingly.
- On an `index.lock` error, wait a few seconds and retry. Never delete the lock file.
- Afterwards the phase's files are gone from `git status --porcelain --untracked-files=all`.
- A check that leaves nothing to commit (a phase that edits only pre-run files or files outside git, a check-only phase): the phase is `complete` once the check passes; record the reason in `## Claude tự quyết`.
- A phase that touches several repos gets one commit per repo.
- Subjects: `<ticket> - Phase N: <phase title>` (`Phase N: <phase title>` without a ticket; the ticket comes from the plan frontmatter or the branch name; follow the user's commit-message rules), `<ticket> - Fix test failures`, `<ticket> - Fix review findings (cycle N)`, `<ticket> - Revert <finding summary>`, `<ticket> - Apply review cleanup` (a cleanup-only batch), `<ticket> - Update docs`. The Step 4 fix batch, the Step 5 fix batch, the revert agent's change and the Step 6 docs are committed the same way.
- Cook's commits need no per-commit confirmation; only the push is asked (§ Step 6).

**Output:** `✓ Step 3: Implemented [N] phases via [M] sub-agent dispatches - [X/Y] tasks complete`

## Step 4: Testing

- **MUST** spawn a testing subagent: `Agent(subagent_type="test-engineer", prompt="<test command(s) + verification gate, per the bullets below>", description="Run tests")`. The prompt tells it to run the test command(s) and the project's verification gate (what its CLAUDE.md declares: lint, typecheck, `pnpm verify`, ...), and to report each command verbatim with pass/fail counts and the failing tests.
- It also re-runs on the Step 3 start commit any test an implementer reported as already failing, in a temporary detached worktree (`git worktree add --detach <scratch dir> <commit>`), never by switching the main checkout, and removes the worktree afterwards. Link or install the repo's dependencies there the way its CLAUDE.md or worktree setup says (e.g. symlink `node_modules`), and confirm only when the same assertion fails on the base.
- A test confirmed failing on the Step 3 start commit is exempt from the 100% rule here and in § After applying, and is recorded in `## Claude tự quyết`; if it passes on the base, it is a regression from this run and is treated as a failure (`debugger`).
- If failures: **MUST** spawn `debugger` subagent → fix dispatch → repeat. Fix dispatches (`debugger`/`executor`) end with <Report contract block, verbatim>; the orchestrator commits them (`<ticket> - Fix test failures`).
- This report is the reviewers' evidence (§ Review package).
- Tests cover happy path, edge cases, errors.
- **Forbidden:** fake mocks, commented tests, changed assertions, skipping subagent delegation.

**Output:** `✓ Step 4: Tests [X/X passed] - testing subagent invoked`

Report the test results and dispatch the Step 5 review in the same turn — do not stop for approval. Tests must be 100% green first (a confirmed pre-existing failure excepted); if they are not, fix them before proceeding.

## Step 5: Combined Review — sized by the diff (MANDATORY)

**Size check first.** Base = `git merge-base <base-branch> HEAD` (when the work is committed on the base branch itself, the commit recorded at the start of Step 3). Run `git diff --numstat <base>..HEAD -- . ':(exclude,glob)**/tests/**' ':(exclude,glob)**/__tests__/**' ':(exclude,glob)**/*.test.*' ':(exclude,glob)**/*.spec.*'`; files with changes from before the run are not counted. A binary row (`-  -`) counts as a file with 0 lines. The diff is **small** when added + removed ≤ 80 lines AND ≤ 3 files; otherwise **large**. State the numbers in the Step 5 output line.

| Diff | Dispatch |
|------|----------|
| Small | **One** `code-reviewer` with Dispatch 1/2's prompt, changed in three places: "Return TWO sections — both required, no exceptions" becomes "Return THREE sections — all three required, no exceptions"; a `### Cleanup (non-gating, no severity)` section is added, built from Dispatch 2/2's "Project conventions are binding" paragraph, the four lens blocks (R first, with the `Reuse searched:` line) and the Collapse block; and the prompt says a correctness defect a lens finds goes straight into Section B, never into Cleanup. It does NOT get Dispatch 2/2's role paragraph or its Reviewer evidence block (the single reviewer already has it from Dispatch 1/2's prompt) |
| Large | **Two** fresh `code-reviewer`s **in one message** so they run concurrently, as below |

| # | Agent | Lens | Gating? |
|---|-------|------|---------|
| 1 | `code-reviewer` | Section A plan-compliance + Section B bugs by severity | **YES** |
| 2 | `code-reviewer` | **Reuse + Simplification + Efficiency + Altitude** — one agent, four lenses, Reuse first | no |

Agent 2 runs the four cleanup lenses of the `simplify` skill as a review-only sub-agent. Use `code-reviewer` for every reviewer: it is read-only (no Write/Edit), so no reviewer can violate the Step 3 gate. Fall back to `critic`, then `general-purpose`.

**Model:** dispatch every reviewer with `model: "opus"` — this is the post-implementation verification pass.

**Why size the review:** on a small fix two reviewers cost several times what the executor did. A diff of a few lines gives the cleanup lenses little to find, so one reviewer holding all of it is enough; the second agent pays off only when there is enough code for the lenses to work on.

**Why the four cleanup lenses share one agent:** they work off the same working set — the changed files, the mechanism beneath them, and the shared modules they could have used — and their fixes subsume each other. Copied code is both a Reuse finding ("call the existing helper") and a Simplification finding ("this duplicates X"). The Altitude fix routinely absorbs a Simplification and an Efficiency finding: repair the shared mechanism and the special cases, the duplication and the redundant work all vanish together. One agent holding all four can report "these are one change"; isolated agents cannot, and the orchestrator — which sees only their summaries, not the code — collapses them badly.

**Why Reuse shares the cleanup agent:** a separate Reuse agent searches no more than the cleanup agent, reads many of the same files, and about a third of its findings duplicate Simplification findings the orchestrator must then dedup. Reuse findings are the highest-value ones — they **delete** code — so the merged prompt runs the Reuse pass first and requires a `Reuse searched:` line, which keeps that search from being skipped.

**Why not one agent per lens like `simplify` does:** `simplify` dedups **and applies** the fixes in the same head, so the cross-lens collapse just moves from review time to apply time. Cook forbids that (Step 5's **Forbidden** line below: fixes are always dispatched to a sub-agent), so in cook the collapse would land in the orchestrator working from summaries — the weakest possible place. One cleanup reviewer keeps the synthesis where the code understanding is.

**Why one dispatch instead of two steps:** the reviewers run concurrently, so a large diff costs one reviewer's wall-clock time and one gate. The cost is that gating and non-gating findings arrive together — § Verdict below is what keeps them from turning to mush. Read it before acting on any finding.

**Why quality is a separate lens from bugs:** agent 2 is looking at reuse/complexity/efficiency/depth, NOT correctness. Agent 1 owns bugs. Do not let the cleanup agent argue a correctness defect — if it does, re-file it under Section B severity and treat it as gating.

### Review package

Write the package after Step 4 is green, right before dispatching Step 5. `<base>` is the size check's Base, computed per repo. For each repo:

```bash
mkdir -p <plan-dir>/review
{ echo "## Repo <path>"; git -C <repo> log --oneline <base>..HEAD; git -C <repo> diff --stat <base>..HEAD; git -C <repo> diff -U10 <base>..HEAD -- . ':(exclude,glob)**/package-lock.json' ':(exclude,glob)**/pnpm-lock.yaml' ':(exclude,glob)**/yarn.lock'; } > <plan-dir>/review/review-00.diff   # first repo
{ echo "## Repo <path>"; git -C <repo> log --oneline <base>..HEAD; git -C <repo> diff --stat <base>..HEAD; git -C <repo> diff -U10 <base>..HEAD -- . ':(exclude,glob)**/package-lock.json' ':(exclude,glob)**/pnpm-lock.yaml' ':(exclude,glob)**/yarn.lock'; } >> <plan-dir>/review/review-00.diff   # each further repo
```
- Lockfiles appear only in `--stat`. Snapshots and other generated files stay in the diff.
- A plan that touches several repos goes into one file: write it with `>` for the first repo and `>>` for each further repo, each section starting with `echo "## Repo <path>"` (inside the braces, before the `git log`).
- `review-00.diff` ends with the working-tree diff of every changed file left uncommitted (pre-run files edited by the run, files no phase owns; a re-review package: below): `git -C <repo> diff -U10 HEAD -- <file>`, or `git -C <repo> diff --no-index -U10 /dev/null <file>` for an untracked file (exit code 1 is expected), under a header saying this diff is not committed (and, for a pre-run file, includes edits made before cook ran). A pre-run file outside the plan root counts as edited by the run when its hash differs from the Step 3 start record. For env/secret files or paths the user's rules forbid copying, write only `<path>: edited, not committed, diff omitted (secrets)`.
- A file outside any git repo cannot be committed, so the package copies its full content under the header "no diff: not in a git repo".
- Re-review: before each fix batch that contains a blocking fix (the only batch a re-review follows; a cleanup-only batch needs neither `FIX_BASE` nor a package) record `FIX_BASE` = `git rev-parse HEAD` (written down literally) for every repo the batch can touch, like Base. After re-verify and any revert, write `review-NN.diff` from `FIX_BASE..HEAD`, and append only the uncommitted files the fix batch changed (listed in its report, or attributed to it by the check), not every uncommitted file of the run; NN is the cycle the fix batch opens, i.e. the value after review-cycle.md's `cycle++`: the first review reads `review-00.diff`, the first fix batch is `cycle 1` and its re-review reads `review-01.diff`.

Reviewer evidence block — pasted into Dispatch 1/2 (the small path uses that same prompt), into Dispatch 2/2's role paragraph and into the scoped re-review prompt, replacing every "Diff under review" paragraph. Templates refer to it as `<Reviewer evidence block, verbatim, with review-NN.diff ...>`:

```
## Diff under review
Read <plan-dir>/review/review-NN.diff once. It holds the commit list, the stat and the full diff
with 10 lines of context for <base>..HEAD, and it is your view of the change: do not re-run
git diff, git log or git status. Read a changed file in full only when a hunk you must judge is
cut off, and say so.

## Evidence already gathered: do not repeat it
<Step 4 report: each command verbatim and its result; for a re-review, the re-verify commands
and results>
These ran on this exact HEAD. The implementers' Tests lines in their reports ran earlier, on
their own phase. Do not re-run the test suite, lint, typecheck or diagnostics, even where your
agent definition asks for them. Run one focused test only when the code raises a specific doubt
that no run above answers, and name it in your report.

## Do not trust the reports
Treat the implementer reports as unverified claims. Check each claim, and each Done-when
evidence line, against the package. A stated rationale ("kept it simple", "per YAGNI") never
lowers a finding's severity.

## Outside the diff
Read code outside the diff only to check a risk you can name (the callers of a changed
contract, the other users of a shared helper, an existing helper the diff could reuse), one
focused check per risk. End with `Checked outside the diff: <risk → what you read>` or `none`.
When something cannot be verified from the diff and the evidence, write
`⚠️ CANNOT VERIFY: <what> — <what would settle it>` instead of searching further; the
orchestrator settles it.

You are read-only: do not change the working tree, the index, HEAD or any branch.
```

- For the cleanup reviewer, Lens R's search counts as a named risk. The `Reuse searched:` line stays.
- Give a reviewer only the plan, the package, the implementer reports and the evidence, as they are. Never tell a reviewer which findings are known, what to skip, or how severe something is.
- Settle every `⚠️ CANNOT VERIFY` line yourself, before bucketing, by reading code or running one command. A confirmed problem becomes a finding in the bucket its severity puts it in; anything else goes to `## Claude tự quyết`.

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

  <Reviewer evidence block, verbatim, with review-00.diff>

  ## Return TWO sections — both required, no exceptions

  ### Section A — Plan Compliance
  For every § Acceptance Criteria checkbox in plan.md (or spec.md's § Tiêu chí hoàn thành /
  Acceptance Criteria if present)
  AND every phase "Done when" item:
  AND, when plan.md has a § AC Traceability table, every row whose Status is `in scope`:
  take the AC text from <plan-root>/jira/<KEY>.md (not the plan's wording) and check that
  the code does exactly that, including the UI text, labels and audit wording the AC quotes.
  AND, when plan.md has a § Review Focus with lines (not "None found"), every line: check that the
  code behaves as the line says for that input or condition, and that the test it names exists and
  passed in the Step 4 evidence.
    1. List the criterion verbatim.
    2. Mark ✅ / ❌ MISSING / ❌ WRONG / ⚠ EXTRA.
    3. Cite file:line proving the verdict.
  List table rows with Status `deferred`, `jira-unclear` or `out of scope` with their status and
  no verdict; they do not count against "all ✅".
  This section MUST be exhaustive — every AC, no skips. Also flag work
  that lives outside the file ownership declared in the plan.

  ### Section B — Code Quality (severity-rated)
  Standard severity-rated review of the diff:
    - **Critical:** security, correctness defects, OWASP, data loss, crashes
    - **High:** logic bugs, missed edge cases, regressions
    - **Medium:** maintainability, naming, duplication, SOLID/design concerns, weak tests
    - **Low:** nits, style, comment quality
  For each finding, cite file:line and propose a concrete fix.

  ## Final verdict
  - ✅ APPROVED — Section A all ✅, Section B has no Critical and no High
  - ❌ CHANGES REQUIRED — list specifically what blocks approval and why
  """
)
```

Fill `<plan-root>` with the absolute plan root that holds `jira/` (jira-snapshot.md § File: in a worktree, the main checkout's), here and in § Scoped re-review.

### Dispatch 2/2 — cleanup lenses (non-gating)

Large diff only: same message as dispatch 1, with the same plan files, the same `## Implementer reports` section as Dispatch 1/2 and the same review package, plus the role paragraph, the conventions paragraph and all four lens blocks below. (The small path is defined once, at the dispatch table above.) It returns findings as `file`, `line`, one-line `summary`, and **the concrete cost** — what is duplicated, wasted, or made harder to maintain — plus the `Reuse searched:` line. No severity ratings; these are not bugs.

Role paragraph (agent 2 only):

```
  Quality review of the diff below — you are improving code quality, NOT hunting
  for correctness bugs. Another reviewer owns bugs; do not duplicate that work.
  If you believe you found a real correctness defect (wrong output, crash, data
  loss, security hole, or work that fails outright with realistic data rather
  than merely wasting it), say so explicitly and label it OUT-OF-LENS rather
  than reporting it as a cleanup finding.

  <Reviewer evidence block, verbatim, with review-00.diff>
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
| **Non-blocking** | Section B Medium / Low | Apply what is worth applying; record each skip and why in `## Claude tự quyết` |
| **Non-blocking** | Agent 2, or the Cleanup section of the single reviewer (all four lenses) | Same — apply-or-skip-with-reason. **Never** blocks Step 6, **never** increments the cycle counter |

**Dedup across every report before applying.** The cleanup reviewer has already collapsed its own four lenses; do not undo that by re-splitting its merged findings. Findings pointing at the same line or the same mechanism collapse into one; when a cleanup lens and Section B overlap, keep Section B's (it carries the severity that drives the gate). When you skip a cleanup finding for behaviour change or scope and it carries a `Fallback:`, judge the fallback on its own.

**Apply blocking and non-blocking fixes in the SAME dispatch**, blocking first. Tell the applier that blocking fixes take precedence and to **drop any cleanup finding that the blocking fix already resolved or made stale** — otherwise it patches code that no longer exists.

Applier: `code-simplifier` for a cleanup-only batch (it is purpose-built for this and preserves behaviour), `executor` for anything containing a blocking fix, `debugger` when the blocking fix needs root-cause work first. Dispatch with `model: "sonnet"`. Its prompt carries the findings verbatim and the package path (`<plan-dir>/review/review-NN.diff`), and ends with <Report contract block, verbatim>. Record `FIX_BASE` before dispatching a batch with a blocking fix (§ Review package). Every batch is committed after it returns and passes § Report check, status and commit: its files (`<ticket> - Fix review findings (cycle N)`; N is the cycle the fix batch opens; `<ticket> - Apply review cleanup` for a cleanup-only batch).

### After applying — re-verify (MANDATORY)

Cleanup edits change working code, so the step is NOT done when the fixes land:

1. Re-run Step 4's test command. Must be back to 100%.
2. Run the project's own verification gate if it declares one — the command its CLAUDE.md says must stay green (`pnpm verify` in talos-v2). Lint + typecheck + unit tests passing is **not** the same thing; a repo-level gate catches invariant violations that neither reviewer sees.
3. If either fails, the cleanup caused it. Dispatch a fresh agent (ending with <Report contract block, verbatim>) that undoes only the offending finding's change rather than layering a fix on it; commit it as `<ticket> - Revert <finding summary>`; record the finding as skipped in `## Claude tự quyết`.

Skipping this is how a "quality pass" ships a regression. `simplify` on its own has no verification phase — that gap is closed here, not inherited.

**Cycle (max 3):** as defined in `review-cycle.md`. **Re-dispatch only the gating reviewer** (agent 1, or the single reviewer without its Cleanup section) on every re-review — the cleanup lenses run **once**, on the first pass. Re-running them per cycle repeats the review cost for findings you already hold.

Scoped re-review (both diff sizes) — replaces Dispatch 1/2 on every re-review (cycle > 0):

```
Agent(
  subagent_type="code-reviewer",
  description="Scoped re-review",
  prompt="""
  Scoped re-review of a fix batch. Do not re-review the rest of the change and
  do not re-run Section A over criteria the fix did not touch.

  ## Findings the batch fixed (verbatim)
  <paste each blocking finding; when plan.md has an AC Traceability table, its Section A ❌ rows
  included, with their AC text from <plan-root>/jira/<KEY>.md>

  <Reviewer evidence block, verbatim, with review-NN.diff, <base> = FIX_BASE, and the re-verify evidence>

  ## Return
  - For each finding above: RESOLVED / NOT RESOLVED, with file:line.
  - New issues only in the changed lines, graded on Dispatch 1/2's
    Critical / High / Medium / Low scale, each with file:line and a fix.
  - Final verdict as in Dispatch 1/2: APPROVED (every finding RESOLVED, no new
    Critical or High) or CHANGES REQUIRED.
  """
)
```

**Forbidden:** patching code in-line based on the review. Fixes are always dispatched to a sub-agent. Running the cleanup lenses more than once per cook run. Letting a cleanup finding block Step 6. Telling a reviewer which findings are known, what to skip, or how severe something is.

**Output:** `✓ Step 5: Review ([N] lines / [M] files → [1|2] reviewer[s]) — Section A [✅|❌ N gaps] / Section B [N Critical, N High, N Medium, N Low] / Cleanup [N applied, M skipped] — re-verify [✅|❌] — [Approved|Awaiting fix]`

## Step 6: Finalize

**MANDATORY subagents (NON-NEGOTIABLE):**
1. **MUST** spawn these subagents in parallel:
   - `Agent(subagent_type="executor", prompt="Run full sync-back for [plan-path]: reconcile all completed Claude Tasks with all phase files, backfill stale completed checkboxes across every phase, then update plan.md frontmatter/table progress. Do NOT only mark current phase.", description="Update plan")`
   - `Agent(subagent_type="writer", prompt="Update docs for changes. <Report contract block, verbatim>", description="Update docs")`
2. Sync-back MUST include:

### Status Sync (Finalize)

There is no status CLI — every update is a plain `Edit`. Two places must agree:

| Where | What changes |
|-------|--------------|
| `phase-XX-*.md` checkboxes | `[ ]` → `[x]` for each completed step (Todo List items in older plans) |
| `plan.md` `## Phases` table | that phase's **Status cell only** — `Pending` → `In Progress` → `Complete`. Preserve table structure; never rewrite the whole table |
| `plan.md` frontmatter | `status:` = `in-progress` while any phase is open, `completed` when all are `Complete` |

**Derive status from the checkboxes, not from memory.** A phase is `Complete` only when every checkbox
in its file is ticked (a file with no box: when its task is complete). Re-read the phase file before writing its row.
   - Sweep all `phase-XX-*.md` files in the plan directory.
   - Mark every completed item `[ ] → [x]` based on completed tasks (including earlier phases finished before current phase).
   - Update `plan.md` status/progress (`pending`/`in-progress`/`completed`) from actual checkbox state.
   - Return unresolved mappings if any completed task cannot be matched to a phase file.
3. Use `TaskUpdate` to mark Claude Tasks complete after sync-back confirmation.
4. Onboarding check (API keys, env vars).
5. Commit the files the writer lists under `Files changed` (`<ticket> - Update docs`, § Report check, status and commit), never the plan root or `<plan-dir>` (the **Commit** rules in § Report check, status and commit).
6. Ask on the interview page (`../brainstorm-pro/references/html-interview.md`), not in chat: one question per repo on the same page, "Push nhánh `<branch>` lên `<remote>`?" with options Push / Chưa push; the context of each is its own `git log --oneline <base>..HEAD`. On a branch the user's rules forbid pushing (e.g. `master`), the question says so and recommends Chưa push.
7. If the user chose Push: when the user's push rules require fetching and merging the target branch first, the orchestrator does that (the only git commands that change file content that cook runs: the fetch + merge, and `git merge --abort` on a conflict); the merge commit uses the same commit identity as § Commit. If the merge refuses to start (uncommitted or staged changes in its way, an untracked file it would overwrite), do not stash or clean, and do not retry on your own: do not push; ask on the interview page with git's message, options `Đã dọn xong, thử lại` (run the fetch + merge once more) / `Chưa push`. On a conflict (after `git merge --abort`) ask the same two options. After a merge, dispatch `test-engineer` with the § Step 4 prompt and push only if it is green. If not green, do not push: report "not pushed" in the final report, with the failing tests and whether each also fails on the target branch. Push per the user's push rules. No squash.

**Output:** `✓ Step 6: Finalized - [N] subagents - sync-back done - [K] commits - [pushed | not pushed]`

### Final report

The final report goes in chat, in Vietnamese. It has the `✓ Step N` lines, the commit list, and:

```markdown
## Claude tự quyết
| Bước | Đã quyết | Vì sao | Sai thì sửa gì |
|---|---|---|---|
```

Sources of the rows:
- Choices the plan made that the design did not (Step 2, only when cook wrote the plan).
- The implementers' `Rulings` lines.
- NEEDS_CONTEXT answers Claude ruled on itself.
- Plan changes made during Step 3 (a split phase).
- The outcome of each `⚠️ CANNOT VERIFY` line.
- Skipped findings, with the reason.
- Files that were not committed.
- Blocking findings still open after the 3 cycles.
- Accepted pre-existing failing tests.
- Files committed outside their phase's ownership.
- DONE_WITH_CONCERNS concerns that were not fixed.
- Branch creation (§ Step 3).

With no rows, write "Không có".

## Workflow Summary

Legend: `[R]` = human gate

```
0 → 1A scout → 1B research → 1C Q&A [R per round] → playback card [R] → 1D approaches → 1E design.md → builder check → review page [R, notes per section]
  → 2 (orchestrator writes plan + self-review) → 3 (sub-agent per phase; NEEDS_CONTEXT question [R, only when hard to change]) → 4 (test sub-agent) → 5 (review sized by the diff: 1 or 2 reviewers) → 6 (finalize sub-agents; push question [R], with its merge follow-up)
```

Human gates live in Step 1. Steps 2–6 run straight through (exceptions: `../SKILL.md` § Gates, marked `[R]` above).
If a plan path is provided at Step 0, skip Steps 1–2 and start at Step 3.

## Critical Rules

- Never skip steps without explicit user instruction.
- **STEP 1 IS NOT DELEGATED.** The design conversation (1C Q&A, 1D approaches, 1E design review) is run by the orchestrator. Sub-agents assist with reading in 1A/1B only, plus the builder check agent in 1E. Never write `spec.md`, never invoke `ck-plan` — Step 2 continues in this run.
- **MANDATORY SUBAGENT DELEGATION:** Steps 3, 4, 5, 6 MUST spawn subagents via the `Agent` tool. DO NOT implement, test, or review directly. Resolve every type through `subagent-patterns.md` § Agent Name Mapping first.
  - Step 3: `executor` / `designer` for frontend (one per phase)
  - Step 4: `test-engineer` (and `debugger` if failures)
  - Step 5: `code-reviewer` sized by the diff (§ Step 5) — small: one reviewer returning Section A, Section B and a Cleanup section; large: two in ONE message — agent 1 returns the dual-section gating report (Section A plan-compliance, non-overridable + Section B bugs by severity); agent 2 runs Reuse (first) + Simplification + Efficiency + Altitude and collapses findings that are one change. Cleanup lenses never gate. Re-verify after any fix batch.
  - Step 6: `executor` (sync-back), `writer` (docs); the orchestrator commits and pushes
- **ALWAYS** create a task list — use `TaskCreate` one task per phase file (§ Step 3 Task hydration), with priority order and dependencies (or `TodoWrite` if Task tools unavailable). This is never skipped.
- **ALWAYS** keep the task list updated in lock-step, not just at the end:
  - `TaskUpdate` → `in_progress` immediately BEFORE dispatching each phase's sub-agent.
  - `TaskUpdate` → `complete` once its report passes the check and its commit lands, or nothing is left to commit (§ Report check, status and commit) — in the same turn, one phase at a time. Never batch status updates to the end of the run.
  - (Fall back to `TodoWrite` for these updates only if Task tools error — but the update still happens.)
- All step outputs follow format: `✓ Step [N]: [status] - [metrics]`
- **VALIDATION:** The workflow is INCOMPLETE if (a) `Agent` tool calls = 0 for Step 3, Step 4, OR Step 5, OR (b) any phase finished without a matching `TaskUpdate` → `complete`, OR (c) the task list still shows `pending`/`in_progress` items for work that is actually done.
