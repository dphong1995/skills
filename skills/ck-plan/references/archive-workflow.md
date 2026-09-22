# Archive Workflow

## Your mission
Read and analyze the plans, then archive specific plans or all plans in the `plans` directory.

## Plan Resolution
1. If a path was provided in the arguments → use that path
2. Else read every plan in the project's plan root (`plans/`, else `ai_docs/`, else `docs/plans/`)

## Workflow

### Step 1: Read Plan Files

Read the plan directory:
- `plan.md` - Overview and phases list
- `phase-*.md` - 20 first lines of each phase file to understand the progress and status

### Step 2: Ask user to confirm the action before archiving these plans
Use `AskUserQuestion` tool to ask if user wants to proceed with archiving these plans, select specific plans to archive or all completed plans only.
Use `AskUserQuestion` tool to ask if user wants to delete permanently or move to the `./plans/archive` directory.

### Step 3: Archive the plans
Start archiving the plans based on the user's choice:
- Move the plans to the `./plans/archive` directory.
- Delete the plans permanently: `rm -rf ./plans/<plan-1> ./plans/<plan-2> ...`

### Step 4: Ask if user wants to commit the changes
Use `AskUserQuestion` tool to ask if user wants to commit the changes with these options:
- Stage and commit the changes (delegate to the `git-master` agent, or run `git` yourself)
- Commit and push the changes (same, plus `git push`)
- Nah, I'll do it later

**Before offering these:** check whether the plan root is gitignored (`git check-ignore -v plans`).
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
