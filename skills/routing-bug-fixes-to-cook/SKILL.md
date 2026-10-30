---
name: routing-bug-fixes-to-cook
description: Use when a bug's root cause is confirmed (superpowers:systematic-debugging Phase 3 done, or the cause is otherwise known) and you are about to write the fix, or when a fix in progress grows to more files or lines than planned
---

# Routing Bug Fixes to Cook

## Overview

systematic-debugging finds the root cause. It does not decide who implements the fix. A large fix goes through the `cook` skill (plan → executor sub-agents → tests → review). Only a small fix continues with systematic-debugging Phase 4 in-line.

**Violating the letter of this rule is violating its spirit.**

## The Gate

Run it after the root cause is confirmed and BEFORE the first test or fix edit. List the files the fix touches and estimate the lines it changes. Count production code only — tests, snapshots, fixtures, lockfiles and generated files don't count. The fix is **LARGE** if ANY of these is true:

1. It touches **≥ 3 production files** in total, counted across all repositories
2. It changes **> 50 production lines** (added + removed), counted across all repositories
3. There is **more than one reasonable way** to fix it

| Result | Do |
|---|---|
| LARGE | Invoke `cook` now (Handoff below). Write no test and no fix yourself first. |
| SMALL | Continue systematic-debugging Phase 4. |
| SMALL, then a 3rd production file appears or the fix passes 50 production lines | STOP editing. Invoke `cook` with the findings and the edits made so far. |

While fixing in-line, after every fix edit run `git diff --stat` in each repository you edited and add up the fix's production lines; a new file counts all its lines (untracked files are not in `git diff`). When the in-line fix is done, ask on the interview page (`~/.claude/skills/brainstorm-pro/references/html-interview.md`), not in chat, whether to commit (and push).

Only the user's explicit words skip the gate ("sửa luôn, không cần cook", "skip cook"). Urgency is not permission.

## Handoff

Invoke the Skill tool with `skill: "cook"` and these args:

```
Bug fix from routing-bug-fixes-to-cook — root cause confirmed. Step 1 runs collapsed.
Symptom: <what the user sees>
Root cause: <file:line — what is wrong + the evidence>
Files to change: <list, per repository>
Candidate fixes: <1-3, recommended first>
Failing test: <the test that reproduces it>
Edits so far: <files already changed and their state, or "none">
```

Collapsed Step 1 (cook `references/design-phase.md` § Escalation): the investigation is the scout; ask Q&A only for a genuinely open question; the candidate fixes (recommended first) and the 5-line design block are one interview-page question, whose pick is the only approval (none in chat). From Step 2 on, cook runs exactly as written — executor sub-agents do every edit, the first phase writes the failing test, and the review still runs.

## Rationalizations

| Excuse | Reality |
|---|---|
| "I have the whole fix in my head — cook would re-plan work I already planned" | Collapsed Step 1 is one approval. Your findings become the plan. |
| "It's urgent, QA is waiting" | Urgency is not permission. Only the user's explicit words skip the gate. |
| "I'm already in Phase 4; debugging says don't skip process under pressure" | Phase 4 in-line is for SMALL fixes only. This gate decides that. |
| "I'll invoke cook but do the edits inline, it's faster" | Cook with inline edits is not cook. Executors do every edit. |
| "I'll keep cook light / cut it down if it sprawls" | The collapse is defined above. Nothing else gets cut. |
| "Fix the BE file now, cook the FE later" | Splitting is the same skip. The whole fix goes to cook. |
| "Only 2 FE files + one line in BE" | That is 3 production files. Count across repositories. |
| "Only 2 files, so it's small" | Over 50 production lines is LARGE at any file count. |
| "My estimate was under 50, so I'll finish" | The `git diff --stat` count decides, not the estimate. |
| "Ask the user whether to use cook" | The user already made this rule. Invoke cook. |

## Red Flags — STOP and invoke cook

- About to Edit/Write a 3rd production file for a bug fix outside cook
- `git diff --stat` shows over 50 production lines for an in-line fix
- "Just this one file first"
- Writing the failing test yourself before the gate said SMALL
