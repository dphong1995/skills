# Code Review Cycle

Review-fix cycle used at Step 5. It runs autonomously — no approval stop.

## Reviewer outputs

Step 5 dispatches one review sub-agent (small diff) or two in one message (large diff), sized by the diff per `workflow-steps.md` § Step 5. Their output lands in two buckets:

**Gating — from agent 1 (`code-reviewer`; the single reviewer on a small diff):**
- **Section A — Plan Compliance:** per-acceptance-criterion verdicts (✅ / ❌ MISSING / ❌ WRONG / ⚠ EXTRA) for the plan ACs, plus the `in scope` rows of plan.md's AC Traceability table when it has one. **Non-overridable once CONFIRMED (§ Finding check)** — any ❌ means dispatch a fix sub-agent, not "note it and move on".
- **Section B — Code Quality (bugs):** severity-rated findings (Critical / High / Medium / Low). Critical + High gate; the counts per severity come from this section.

**Gating — from the Runtime check (`verifier`, Dispatch 3; only when a `## Live check` path changed):**
- **Runtime AC:** ✅ / ❌ WRONG / ❌ MISSING / ⚠ NOT VISIBLE. Any ❌ blocks like Section A; `⚠ NOT VISIBLE` does not block ("AC checked by tests only" in `## Claude's calls`).
- **Defects:** BROKEN blocks (graded High); COSMETIC is non-blocking, apply-or-skip.

**Finding check (`verifier`, `workflow-steps.md` § Finding check):** reviewers may park a low-confidence Critical/High under "Open Questions" and write `⚠️ CANNOT VERIFY` lines; both go to the Finding check with every blocking finding. Only CONFIRMED findings gate. A reviewer's **Pre-existing** section (issues already at base in unchanged code) is non-gating and goes to the final report's `## For you to decide`.

**Non-gating — from agent 2, or the Cleanup section of the single reviewer (reuse + simplification + efficiency + altitude, already collapsed across its own lenses):**
- Findings carry `file`, `line`, `summary`, and the concrete cost — **no severity**, because they are not bugs. They are applied or skipped with a stated reason, and they **never** gate Step 6 or increment the cycle counter.

Done means no CONFIRMED blocking finding (Section A, Section B Critical/High, Runtime AC ❌ / BROKEN). Remaining Medium/Low findings and all cleanup findings are applied or explicitly skipped, each skip with its reason recorded in the final report's `## Claude's calls` (Claude tự quyết) (`workflow-steps.md` § Final report) — then re-verified (see `workflow-steps.md` § After applying).

## Cycle (max 3 cycles)

```
cycle = 0
LOOP:
  1. cycle == 0 → small diff: 1 reviewer (gating + Cleanup section)
                  large diff: fan out both agents in one message
                  Runtime check matched → add Dispatch 3 (`verifier`) to the same message
     cycle >  0 → scoped re-review of the gating reviewer ONLY, using the
                  template in `workflow-steps.md` § Scoped re-review, reading
                  the `FIX_BASE..HEAD` package (`workflow-steps.md` § Review
                  package), not the whole change
                  + Dispatch 3 again only when `git diff --name-only FIX_BASE..HEAD`
                  hits a matching Live check path (max once per cycle)

  2. Finding check (`workflow-steps.md` § Finding check): when there is a blocking
     finding, an Open Questions Critical/High or a `⚠️ CANNOT VERIFY` line, one
     `verifier` returns CONFIRMED / REFUTED / UNCLEAR per item; otherwise skip.
     Then dedup across every agent that reported.
     Overlap between a cleanup lens and Section B → keep Section B's copy.

  3. DISPLAY FINDINGS:
     ┌─────────────────────────────────────────┐
     │ Review Results                          │
     ├─────────────────────────────────────────┤
     │ Summary: [what implemented], tests      │
     │ [X/X passed]                            │
     ├─────────────────────────────────────────┤
     │ Section A — Plan Compliance             │
     │  [✅ all N criteria | ❌ N gaps]         │
     │ Runtime — [skipped|✅|❌ N AC, N broken, │
     │  N cosmetic]                            │
     │ Verified [N confirmed, N refuted,       │
     │  N unclear]                             │
     │ Pre-existing ([N]): - [file:line]       │
     │ Blocking — Critical/High ([N])          │
     │  - [issue] at [file:line]               │
     │ Medium/Low ([N]): apply or skip         │
     │  - [issue] at [file:line]               │
     ├─────────────────────────────────────────┤
     │ Cleanup (non-gating)                    │
     │  Reuse ([N]):          - [file:line]    │
     │  Simplification ([N]): - [file:line]    │
     │  Efficiency ([N]):     - [file:line]    │
     │  Altitude ([N]):       - [file:line]    │
     │  Collapsed ([N]): one mechanism change  │
     │   subsuming [M] shallow findings        │
     └─────────────────────────────────────────┘

  4. Decide and act — do NOT ask the user for approval:
     IF cycle < 3 AND a CONFIRMED finding is a Section A or Runtime AC ❌, a Runtime
     BROKEN, or Critical/High (act only on CONFIRMED; REFUTED and UNCLEAR go to
     `## Claude's calls`):
       → if thrash (a re-review marked a finding NOT RESOLVED, or two consecutive
         re-reviews both returned CONFIRMED blocking findings in the same function):
         the applier is `debugger` (root cause, failing reproducing test first),
         recorded in `## Claude's calls`; else the applier is `executor`
       → then dispatch the batch to a fresh applier, blocking fixes FIRST and
         the worthwhile non-blocking ones in the same batch; the
         orchestrator commits the batch (`workflow-steps.md` § Report check,
         status and commit), re-runs tests + the project verification
         gate, cycle++, LOOP
     ELSE:
       → apply the warnings/suggestions/cleanup findings that are genuinely
         worth applying, re-run tests + the project verification gate,
         PROCEED, and record each skipped finding and its reason in
         `## Claude's calls` (behaviour change, out of diff scope, breaks a
         documented invariant, false positive)

  5. Cycle >= 3 with blocking issues left (the ELSE branch of step 4; hard cap, max 3: never open a
     fourth blocking fix batch):
     → stop looping, PROCEED, and report the remaining issues plainly

  6. Re-review audit, before PROCEED to Step 6: every committed "Fix review
     findings (cycle N)" batch must have `review-NN.diff` and a re-review report.
     A missing one → write the package and run the re-review now (it counts
     within the 3 and opens no fourth batch)
```

**Cleanup findings never drive the loop.** They are applied once, in whichever
iteration reaches step 4's ELSE branch. A cook run that loops three times on
Section A still runs the cleanup lenses exactly once.

**Fixes are always dispatched to a sub-agent** (e.g. `executor` or `debugger`). The orchestrator does not patch code in-line.

## Severity

Section B's severity scale is the one in agent 1's prompt (`workflow-steps.md` § Dispatch 1/2). It is the only scale; do not grade against any other list.

**Pre-existing** (an issue already at base in code the diff did not change) has no severity and never gates; it is listed under `## For you to decide`.

A cleanup finding moves to Section B only when the cleanup reviewer labelled it OUT-OF-LENS. Grade it on agent 1's scale; it gates only if Critical or High. Never promote an R/A/B/C finding yourself because it looks serious: duplication, coupling, YAGNI/KISS and wasted work stay cleanup.

## Output Formats
Use the Step 5 output line in `workflow-steps.md` § Step 5 — the single format for both sizes and for re-reviews after a fix.
