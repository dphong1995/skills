# Plan Writing

What `plan.md` and the phase files contain, how to write a step, and the self-review every plan
gets. Read by ck-plan and by cook Step 2. Where the files go and how status is tracked:
`plan-organization.md`. Frontmatter and writing style: `output-standards.md`.

## Who reads the plan
Each phase file is pasted into the prompt of an executor that has not seen the spec, the
conversation or the other phases; plan.md's Global Constraints are pasted with it. The executor
writes idiomatic code once it knows the exact interface and the exact test, and makes a reasonable
choice wherever the plan leaves one open. What it cannot know is what was decided: which files,
which names and signatures, which values the spec pins, which tests prove the phase. Write those.
A plan is the set of decisions the executor cannot make alone.

## plan.md
Frontmatter (`output-standards.md`), then in this order:
- `# <title>`
- `## Overview`: 2-5 lines, then the requirements link: `Spec: [spec.md](./spec.md)` (cook:
  `Design: [design.md](./design.md)`). With neither, one line saying where the requirements came from.
- `## Global Constraints`: the rules every phase must follow, one per line. Exact values copied
  verbatim from the spec or design (names, UI copy, limits, formats, versions), and the project rules
  this change is most likely to break ("the study comes from the registry-id header", "run only the
  touched test files"). Not a copy of CLAUDE.md: executors read it already. `None.` when there are none.
- `## Review Focus`: up to 5 inputs or failure conditions the spec implies but does not spell out
  and no other test in the plan covers, most likely to hurt a user first, one per line:
  `<input or condition> → <behaviour a reasonable user expects> — test: <test name> (phase N)`.
  Each line's test is a step in the owning phase. With none: `None found (checked: <what you considered>)`.
- `## Phases`: the table (`plan-organization.md` § Plan Creation).
- `## Dependencies`
- `## AC Traceability`: only when the spec or design has "Truy vết AC". Copy AC, Text and Status
  verbatim and fill Where with the owning phase file; format:
  `../brainstorm-pro/references/jira-snapshot.md` § Traceability table.
- Cook adds `## Acceptance Criteria` and `## Open Items` (`cook/references/workflow-steps.md` § Step 2).

Keep plan.md under 80 lines, not counting Global Constraints, Review Focus and AC Traceability.

## Phase file
`phase-XX-<deliverable>.md`, sections in this order. Omit an optional section when it has nothing
to say; never pad one.
1. `## Overview`: 1-3 lines: what the phase delivers, priority, status, the phases it depends on.
2. `## Context Links`: spec sections, reference files, the conventions file to read first.
3. `## Key Insights`: scout facts the executor cannot see from the files it owns: a caller
   elsewhere, a trap, a precedent to copy.
4. `## Requirements` (optional): only what this phase must do that Global Constraints and the
   linked spec sections do not already say.
5. `## Architecture` (optional): only when Interfaces and the steps do not show the design.
6. `## Interfaces`: `Consumes:` what the phase uses from other phases, or from code another phase
   changes, with exact names, parameters, return types, routes and payloads. `Produces:` what later
   or parallel phases rely on, in the same detail. `None.` when it shares nothing.
7. `## Related Code Files`: exact paths under Create / Modify (with the line range when known) /
   Delete / Test. This is the phase's file ownership: disjoint from every phase that runs in
   parallel with it. Never "grep for the rest".
8. `## Implementation Steps`: checkbox steps (§ Steps). They are the phase's todo list: cook
   creates a task per phase file with an unchecked box, and sync-back ticks them.
9. `## Success Criteria`: the "Done when" items cook checks the executor's report against, each one
   checkable; the AC Traceability row ids the phase owns.
10. `## Risk Assessment`: what could break and how the phase guards against it; security when the
    phase touches auth, permissions, data exposure or input handling.

No Todo List (the steps are the checkboxes) and no Next Steps (the Phases table and Dependencies
say it).

## Sizing phases
- A phase is the smallest unit with its own verification that a reviewer could reject while
  approving its neighbour. A small change in one cluster of files is one phase.
- Fold setup, configuration, tests and docs into the phase whose deliverable needs them. No "write
  tests" phase, no "setup" phase, and no "verify" phase that re-runs what cook Step 4 runs (the
  tests and the project's verification gate).
- A separate phase only for work no single phase owns: a measurement across phases or repos, a
  live check of the finished flow, a scenario check of rewritten skill text.
- Phases marked parallel own disjoint files (`workflow-modes.md` § Parallel Mode).

## Steps
Each step is one action with a checkable result, and lets the executor write exactly one
reasonable thing: unambiguous, not complete.

- **Behaviour an automated test can see** (logic, a bug fix, an API, a reducer, a component's
  rendered output): test first.
  - [ ] Write the failing test `<name>` in `<file>`: <its assertions, with the spec's exact values>
  - [ ] Run `<command>` → FAIL: <the reason it must fail>
  - [ ] Change `<file>`: `<signature>`; <the values the spec pins; one line on the approach when the
        signature and the test leave a choice>
  - [ ] Run `<command>` → PASS
- **Behaviour no automated test can see**: the step names the check instead, with its command or
  procedure and the result that means it passed:
  - a refactor guarded by snapshot or guard tests: run them before and after → unchanged;
  - a visual or layout change: a live check: what to open, what to measure, the expected value;
  - skill, docs or config text: `grep` for what must and must not be there, or a scenario check.
- **A verification step** is the exact command and the output that means it passed. "Run the
  tests" is not a step.
- **Code in a step** is the signature, the file and the values the spec pins. A body only for an
  algorithm the signature and the tests do not determine, or for exact copy the spec fixes. A plan
  longer than the code it describes has written the code instead.
- **Lines that decide nothing** are gaps: "TBD", "handle edge cases", "add appropriate validation",
  "update touched tests", "modify the files above", a name no phase defines.
- **Another phase's code**: point at its Interfaces block; do not repeat it.

## Self-Review
Run after writing the plan, in every mode (fast included), before red team, validation or the cook
handoff. Re-read the plan with the spec (or design) beside it. Fix what you find in the plan files;
no second pass. A spec requirement with no phase gets a phase or a step.

1. **Coverage**: every requirement and every Tiêu chí hoàn thành / Acceptance Criteria item of the
   spec or design maps to a phase; every `in scope` AC Traceability row names a phase and is listed
   in that phase's Success Criteria.
2. **Step scan**: every step lets the executor write exactly one reasonable thing: no line that
   decides nothing, no body the signature and the tests already determine. Every phase has at least
   one verification step with its command and expected result.
3. **Interface consistency**: names, signatures, types, routes and payloads match between each
   phase's Produces and every Consumes that uses them, and agree with Global Constraints.
4. **Review Focus**: for each input or failure condition the spec implies, a phase's test exercises
   it; up to 5 of those most likely to hurt a user that no test covered are now in Review Focus,
   each with its test step in the owning phase.
5. **Proportion**: `wc -l` of the plan files against the spec or design. Over 2× → find sections that
   restate the spec (Requirements, Key Insights, Architecture) and replace them with links. Still
   over 2× is fine when the steps carry it. No spec or design → skip.
6. **Format**: frontmatter complete; the Phases table has one row per phase file on disk, in order,
   with human-readable link text, and every row resolves (`ls {plan-dir}`); Related Code Files are
   disjoint between phases marked parallel; dependencies are declared.

Output: `✓ Self-review: [clean | K fixes]`
