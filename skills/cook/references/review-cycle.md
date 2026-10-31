# Code Review Cycle

Review-fix cycle used at Step 5. It runs autonomously — no approval stop.

## Report contract

Step 5 dispatches one review sub-agent (small diff) or two in one message (large diff), sized by the diff per `workflow-steps.md` § Step 5. Their output lands in two buckets:

**Gating — from agent 1 (`code-reviewer`; the single reviewer on a small diff):**
- **Section A — Plan Compliance:** per-acceptance-criterion verdicts (plan ACs, plus the `in scope` rows of plan.md's AC Traceability table when it has one) (✅ / ❌ MISSING / ❌ WRONG / ⚠ EXTRA). **Non-overridable** — any ❌ means dispatch a fix sub-agent, not "note it and move on".
- **Section B — Code Quality (bugs):** severity-rated findings (Critical / High / Medium / Low). Critical + High gate; the score / critical_count / warnings / suggestions tally is built from this section.

**Non-gating — from agent 2, or the Cleanup section of the single reviewer (reuse + simplification + efficiency + altitude, already collapsed across its own lenses):**
- Findings carry `file`, `line`, `summary`, and the concrete cost — **no severity**, because they are not bugs. They are applied or skipped with a stated reason, and they **never** gate Step 6 or increment the cycle counter.

Done means Section A = all ✅ **and** Section B = no Critical / no High. Remaining Medium/Low findings and all cleanup findings are applied or explicitly skipped with a stated reason in the final report — then re-verified (see `workflow-steps.md` § After applying).

## Cycle (max 3 cycles)

```
cycle = 0
LOOP:
  1. cycle == 0 → small diff: 1 reviewer (gating + Cleanup section)
                  large diff: fan out both agents in one message
     cycle >  0 → scoped re-review of the gating reviewer ONLY, using the
                  template in `workflow-steps.md` § Scoped re-review: the
                  fixed findings + the before/after tree delta, not the
                  whole change

  2. Dedup across every agent that reported.
     Overlap between a cleanup lens and Section B → keep Section B's copy.

  3. DISPLAY FINDINGS:
     ┌─────────────────────────────────────────┐
     │ Review Results: [score]/10              │
     ├─────────────────────────────────────────┤
     │ Summary: [what implemented], tests      │
     │ [X/X passed]                            │
     ├─────────────────────────────────────────┤
     │ Section A — Plan Compliance             │
     │  [✅ all N criteria | ❌ N gaps]         │
     │ Critical Issues ([N]): MUST FIX         │
     │  - [issue] at [file:line]               │
     │ Warnings ([N]): SHOULD FIX              │
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
     IF Section A has any ❌ OR critical_count > 0 OR high_count > 0:
       → dispatch fix sub-agent with the blocking fixes FIRST and the
         worthwhile non-blocking ones in the same batch, re-run tests +
         the project verification gate, cycle++, LOOP
     ELSE:
       → apply the warnings/suggestions/cleanup findings that are genuinely
         worth applying, re-run tests + the project verification gate,
         PROCEED, and state in the report which findings you skipped and why
         (behaviour change, out of diff scope, breaks a documented invariant,
         false positive)

  5. IF cycle >= 3 AND blocking issues remain:
     → stop looping, PROCEED, and report the remaining issues plainly
```

**Cleanup findings never drive the loop.** They are applied once, in whichever
iteration reaches step 4's ELSE branch. A cook run that loops three times on
Section A still runs the cleanup lenses exactly once.

**Fixes are always dispatched to a sub-agent** (e.g. `executor` or `debugger`). The orchestrator does not patch code in-line.

## Severity

Section B's severity scale is the one in agent 1's prompt (`workflow-steps.md` § Dispatch 1/2). It is the only scale; do not grade against any other list.

A cleanup finding moves to Section B only when the cleanup reviewer labelled it OUT-OF-LENS. Grade it on agent 1's scale; it gates only if Critical or High. Never promote an R/A/B/C finding yourself because it looks serious: duplication, coupling, YAGNI/KISS and wasted work stay cleanup.

## Output Formats
Use the Step 5 output line in `workflow-steps.md` § Step 5 — the single format for both sizes and for re-reviews after a fix.
