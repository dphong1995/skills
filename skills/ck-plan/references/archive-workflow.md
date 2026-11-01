# Archive Workflow

## Your mission
Read and analyze the plans, then archive specific plans or all plans in the plan root.

## Plan Resolution
1. If a path was provided in the arguments → use that path
2. Else read every plan in the project's plan root (`plans/`, else `ai_docs/`, else `docs/plans/`)

## Workflow

### Step 1: Read Plan Files

Read the plan directory:
- `plan.md` - Overview and phases list
- `phase-*.md` - 20 first lines of each phase file to understand the progress and status

### Step 2: Ask user to confirm the action before archiving these plans
Ask on the interview page (`~/.claude/skills/brainstorm-pro/references/html-interview.md`) whether to archive these plans, only specific ones, or all completed plans only.
On the same page, ask whether to delete them permanently or move them to the archive directory (Step 3).

### Step 3: Archive the plans
Start archiving the plans based on the user's choice:
- Move the plans to `{plan-root}/archive`.
- Delete the plans permanently: `rm -rf {plan-root}/<plan-1> {plan-root}/<plan-2> ...`

### Step 4: Ask if user wants to commit the changes
Ask on the interview page whether to commit the changes, with these options:
- Stage and commit the changes (delegate to the `git-master` agent, or run `git` yourself)
- Commit and push the changes (same, plus `git push`)
- Nah, I'll do it later

**Before offering these:** check whether the plan root is gitignored (`git check-ignore -v {plan-root}`).
If it is, there is nothing to commit — say so and skip this question entirely.

## Output
After archiving the plans, provide summary:
- Number of plans archived
- Number of plans deleted permanently
- Table of plans that are archived or deleted (title, status, created date, LOC)

## Important Notes
- Only ask questions about genuine decision points
- Sacrifice grammar for concision
- List any unresolved questions at the end
- Ensure token efficiency while maintaining high quality
