# Codebase Understanding Phase

**When to skip:** If provided with exploration reports, skip this phase.

## Core Activities

### Parallel Codebase Exploration
- Dispatch `Explore` agents (fallback: `explore`, then `general-purpose`) via the `Agent` tool, one
  per aspect of the task, in a single message so they run concurrently
- Give each a narrow question and a breadth hint ("medium" / "very thorough") — they return
  conclusions and file paths, not file dumps
- Wait for all of them to report before analysis
- Write each report to `{plan-dir}/reports/explore-{NN}-{aspect}.md` so the planner subagent can
  read them by path instead of receiving them inline

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

### Environment Analysis
- Review development environment setup
- Analyze dotenv files and configuration
- Identify required dependencies
- Understand build and deployment processes

### Pattern Recognition
- Study existing patterns in codebase
- Identify conventions and architectural decisions
- Note consistency in implementation approaches
- Understand error handling patterns

### Integration Planning
- Identify how new features integrate with existing architecture
- Map dependencies between components
- Understand data flow and state management
- Consider backward compatibility

## Best Practices

- Start with documentation before diving into code
- Use Explore agents for targeted file discovery
- Document patterns found for consistency
- Note any inconsistencies or technical debt
- Consider impact on existing features
