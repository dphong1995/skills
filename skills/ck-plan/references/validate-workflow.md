# Validate Workflow

Interview the user with critical questions to validate assumptions, confirm decisions, and surface potential issues in an implementation plan before coding begins.

## Plan Resolution

1. If a path was provided in the arguments → use it
2. Else list the project's plan root (`plans/`, else `ai_docs/`, else `docs/plans/`) and pick the
   most recent directory whose `plan.md` frontmatter is `status: pending | in-progress`; if more
   than one qualifies, ask via `AskUserQuestion`
3. If no plan found → ask the user for the path, or tell them to create a plan first

## Configuration

Nothing injects validation settings. Use these defaults, overridden only by what the user says in
this conversation:

- **Question count:** 3-8. Fewer than the minimum is fine for a simple plan.
- **Run at all:** ask once before starting; skip if the user already declined validation, or said
  "no questions" / "just plan it".

## Workflow

### Step 1: Read Plan Files
- `plan.md` - Overview and phases list
- `phase-*.md` - All phase files
- Look for decision points, assumptions, risks, tradeoffs

### Step 2: Extract Question Topics
Load: `references/validate-question-framework.md`

### Step 3: Generate Questions
For each detected topic, formulate a concrete question with 2-4 options.
Mark recommended option with "(Recommended)" suffix.

### Step 4: Interview User
Use `AskUserQuestion` tool.
- Target 3-8 questions total (see Configuration)
- Group related questions (max 4 per tool call)
- Focus on: assumptions, risks, tradeoffs, architecture

### Step 5: Document Answers
Add or append `## Validation Log` section in `plan.md`.
Load: `references/validate-question-framework.md` for recording format.

### Step 6: Propagate Changes to Phases
Auto-propagate validation decisions to affected phase files.
Add marker: `<!-- Updated: Validation Session N - {change} -->`

## Output
- Number of questions asked
- Key decisions confirmed
- Phase propagation results
- Recommendation: proceed or revise

## Next Steps (MANDATORY)

**In a full planning run:** validation is the last phase before the handoff. Continue straight into
it — invoke the `cook` skill with `--auto {ABSOLUTE_PATH_TO_PLAN_DIR}/plan.md`. `--auto` because the
plan was already validated, so cook's own review gates are redundant. Do not stop to ask.

**When invoked standalone as the `validate` subcommand:** stop and print the next step with the
absolute path:
> **Best Practice:** Run `/clear` before implementing to start with fresh context.
> Then run:
> ```
> /cook --auto {ABSOLUTE_PATH_TO_PLAN_DIR}/plan.md
> ```
> **Why absolute path?** After `/clear`, the new session loses previous context.

## Important Notes
- Only ask about genuine decision points
- If plan is simple, fewer than min questions is okay
- Prioritize questions that could change implementation significantly
