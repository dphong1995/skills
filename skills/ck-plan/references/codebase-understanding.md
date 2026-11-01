# Codebase Understanding Phase

**When to skip:** If provided with exploration reports, skip this phase.

## Core Activities

### Parallel Codebase Exploration
- For a wide, multi-area search, dispatch `Explore` agents (fallback: `explore`, then
  `general-purpose`) via the `Agent` tool, one per area, in a single message so they run
  concurrently; make a few targeted reads yourself
- Give each a narrow question and a breadth hint ("medium" / "very thorough") — they return
  conclusions and file paths, not file dumps
- Wait for all of them to report before analysis
- Explore agents cannot write files: save each returned report to
  `{plan-dir}/reports/explore-{NN}-{aspect}.md` yourself, so later steps and reviewers can read it by path

### Essential Documentation Review

Read the project's own docs first — **discover them, don't assume filenames.** They are the highest
signal available and they override this skill's defaults.

1. **Conventions / rules** — `CLAUDE.md` at the repo root, then any nested `CLAUDE.md` or
   `AGENTS.md` in the packages you will touch. These carry the enforced invariants, boundary rules,
   and testing contract.
2. **Architecture / overview** — whatever exists under `docs/`. Find it with `ls docs/` rather than
   guessing; different repos use `docs/01-overview.md`, `docs/architecture/`,
   `docs/codebase-summary.md`, or nothing at all.
3. **Design system / UI guidelines** — if the task touches frontend, look for a design-system doc or
   a Storybook config in the UI package.
4. **Existing plans** — `ls {plan-root}` for prior plans covering adjacent work; they record
   decisions already made.

If a doc named here does not exist, note it and move on — absence is not a blocker. Never cite a
doc you did not actually read.
