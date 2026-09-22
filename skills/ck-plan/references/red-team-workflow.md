# Red Team Review

Adversarially review an implementation plan by spawning parallel reviewer subagents that try to tear it apart. Each reviewer adopts a different hostile lens. You then adjudicate findings, and the user decides which to apply.

**Mindset:** Like hiring someone who hates the implementer to destroy their work.

## Plan Resolution

1. If a path was provided in the arguments → use it
2. Else list the project's plan root (`plans/`, else `ai_docs/`, else `docs/plans/`) and pick the
   most recent directory whose `plan.md` frontmatter is `status: pending | in-progress`; if more
   than one qualifies, ask via `AskUserQuestion`
3. If no plan found → ask the user for the path, or tell them to create a plan first

## Workflow

### Step 1: Read Plan Files
Read the plan directory:
- `plan.md` — Overview, phases, dependencies
- `phase-*.md` — All phase files (full content)

### Step 2: Scale Reviewer Count

| Phase Count | Reviewers | Lenses Selected |
|-------------|-----------|-----------------|
| 1-2 phases | 2 | Security Adversary + Assumption Destroyer |
| 3-5 phases | 3 | + Failure Mode Analyst |
| 6+ phases | 4 | + Scope & Complexity Critic (all lenses) |

### Step 3: Define Adversarial Lenses
Load: `references/red-team-personas.md`

### Step 4: Spawn Reviewers
Launch reviewers simultaneously via the `Agent` tool with `subagent_type: "code-reviewer"`
(fallbacks, in order: `critic`, `general-purpose`) — all in one message so they run concurrently.
Each reviewer prompt MUST include override, persona, plan file paths, and hostile instructions.
Load: `references/red-team-personas.md` for reviewer prompt template.

### Step 5: Collect, Deduplicate & Cap
1. Collect all findings
2. Deduplicate overlapping findings
3. Sort by severity: Critical → High → Medium
4. Cap at 15 findings

### Step 6: Adjudicate
For each finding, evaluate and propose: **Accept** or **Reject**.

### Step 7: User Review
Present via `AskUserQuestion`:
- "Looks good, apply accepted findings"
- "Let me review each one"
- "Reject all, plan is fine"

**If "Let me review each one":**
For each finding marked Accept, ask via `AskUserQuestion`:
- Options: "Yes, apply" | "No, reject" | "Modify suggestion"

**If "Modify suggestion":**
Ask via `AskUserQuestion`: "Describe your modification to this finding's suggested fix:"
(user provides free text via "Other" option)
Record the modified suggestion. Set disposition to "Accept (modified)" in the Red Team Review table.

### Step 8: Apply to Plan
For accepted findings, edit target phase files inline with marker.
Add `## Red Team Review` section to `plan.md`.

## Output
- Total findings by severity
- Accepted vs rejected count
- Files modified
- Key risks addressed

## Next Steps (MANDATORY)

Red team is a phase, not a terminus. When run as part of a full planning run, continue straight into
post-plan validation and then the cook handoff (see `workflow-modes.md`).

When invoked standalone as the `red-team` subcommand, stop here and tell the user their next step:
run validation, then `/cook {absolute-plan-path}/plan.md`.

## Important Notes
- Reviewers must be HOSTILE, not helpful
- Deduplicate aggressively
- Adjudication must be evidence-based
- Reviewers read plan files directly
