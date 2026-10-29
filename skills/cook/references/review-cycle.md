# Code Review Cycle

Review-fix cycle used at Step 5 (2-agent fan-out: compliance + bugs, and Reuse+Simplification+Efficiency+Altitude). It runs autonomously — no approval stop.

## Report contract

Step 5 fans out two review sub-agents in one message. Their output lands in two buckets:

**Gating — from agent 1 (`code-reviewer`):**
- **Section A — Plan Compliance:** per-acceptance-criterion verdicts (✅ / ❌ MISSING / ❌ WRONG / ⚠ EXTRA). **Non-overridable** — any ❌ means dispatch a fix sub-agent, not "note it and move on".
- **Section B — Code Quality (bugs):** severity-rated findings (Critical / High / Medium / Low). Critical + High gate; the score / critical_count / warnings / suggestions tally is built from this section.

**Non-gating — from agent 2 (reuse + simplification + efficiency + altitude, already collapsed across its own lenses):**
- Findings carry `file`, `line`, `summary`, and the concrete cost — **no severity**, because they are not bugs. They are applied or skipped with a stated reason, and they **never** gate Step 6 or increment the cycle counter.

Done means Section A = all ✅ **and** Section B = no Critical / no High. Remaining Medium/Low findings and all cleanup findings are applied or explicitly skipped with a stated reason in the final report — then re-verified (see `workflow-steps.md` § After applying).

## Cycle (max 3 cycles)

```
cycle = 0
LOOP:
  1. cycle == 0 → fan out BOTH agents in one message
     cycle >  0 → re-dispatch agent 1 ONLY (agent 2 already reported)

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
Section A still fans out agent 2 exactly once.

**Fixes are always dispatched to a sub-agent** (e.g. `executor` or `debugger`). The orchestrator does not patch code in-line.

## Severity

Section B's severity scale is the one in agent 1's prompt (`workflow-steps.md` § Dispatch 1/2). It is the only scale; do not grade against any other list.

A cleanup finding moves to Section B only when agent 2 labelled it OUT-OF-LENS. Grade it on agent 1's scale; it gates only if Critical or High. Never promote an R/A/B/C finding yourself because it looks serious: duplication, coupling, YAGNI/KISS and wasted work stay cleanup.

## Output Formats
- After fix: `✓ Step 5: [old]/10 → Fixed [N] issues → [new]/10 — cleanup [N applied, M skipped] — re-verify ✅`
- Clean: `✓ Step 5: Reviewed - [score]/10 - [N] findings applied, [M] skipped - cleanup [N applied, M skipped] - re-verify ✅`
