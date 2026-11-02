# Workflow Modes

## Auto-Detection (Default: `--auto`)

When no flag specified, analyze task and pick mode:

| Signal | Mode | Rationale |
|--------|------|-----------|
| Simple task, clear scope, no unknowns | fast | Skip research overhead |
| Complex task, unfamiliar domain, new tech | hard | Research needed |
| 3+ independent features/layers/modules | parallel | Enable concurrent agents |
| Ambiguous approach, multiple valid paths | two | Compare alternatives |

If detection is uncertain, ask on the interview page (`~/.claude/skills/brainstorm-pro/references/html-interview.md`).

## Scope Challenge Integration

Step 0 (Scope Challenge, see `scope-challenge.md`) runs before mode detection and can influence it:
- If user selects **EXPANSION** → auto-suggest `--hard` or `--two`
- If user selects **REDUCTION** → auto-suggest `--fast`
- If user selects **HOLD** → proceed with auto-detected mode

Mode can still be overridden by explicit flags (`--fast`, `--hard`, etc.).
Scope challenge is skipped when `--fast` is explicitly set or task is trivial.

## Fast Mode (`--fast`)

No research. Analyze → Plan → hand off.

1. Read the project's conventions + architecture docs (see `codebase-understanding.md`)
2. Write the plan files yourself (`plan-organization.md` § Plan Creation, content per `plan-writing.md`), then run `plan-writing.md` § Self-Review
3. **Auto-handoff:** invoke the `cook` skill with `{absolute-plan-path}/plan.md` immediately

## Hard Mode (`--hard`)

Research → Scout → Plan → Red Team → Validate → hand off.

1. Spawn max 2 research agents in parallel (different aspects, max 5 calls each) — see
   `research-phase.md` for which agent types to use
2. Read the project's docs; for anything they don't cover, dispatch `Explore` agents to search the
   codebase (see `codebase-understanding.md`)
3. Read the research and exploration reports, then write the plan files yourself (`plan-organization.md` § Plan Creation, content per `plan-writing.md`), then run `plan-writing.md` § Self-Review
4. Post-plan red team review (see Red Team Review section below)
5. Post-plan validation (see Validation section below)
6. **Auto-handoff:** invoke the `cook` skill with `{absolute-plan-path}/plan.md` immediately

## Parallel Mode (`--parallel`)

Research → Scout → Plan with file ownership → Red Team → Validate → hand off.

1. Same as Hard mode steps 1-3
2. Planner creates phases with:
   - **Exclusive file ownership** per phase (no overlap)
   - **Dependency matrix** (which phases run concurrently vs sequentially)
   - **Conflict prevention** strategy
3. plan.md includes: dependency graph, execution strategy, file ownership matrix — this is what cook
   reads to build its own task graph, so record sequential vs parallel groups explicitly
4. Post-plan red team review
5. Post-plan validation
6. **Auto-handoff:** invoke the `cook` skill with `{absolute-plan-path}/plan.md` immediately

### Parallel Phase Requirements
- Each phase self-contained, no runtime deps on other phases
- Clear file boundaries — each file modified in ONE phase only
- Group by: architectural layer, feature domain, or technology stack
- Example: Phases 1-3 parallel (DB/API/UI), Phase 4 sequential (live check of the integrated flow)

## Two-Approach Mode (`--two`)

Research → Scout → Plan 2 approaches → Compare → hand off.

1. Same as Hard mode steps 1-3
2. Planner creates 2 implementation approaches with:
   - Clear trade-offs (pros/cons each)
   - Recommended approach with rationale
3. User selects approach
4. Post-plan red team review on selected approach
5. Post-plan validation
6. **Auto-handoff:** invoke the `cook` skill with `{absolute-plan-path}/plan.md` immediately

## Task Hydration Per Mode

None. `cook` owns task hydration in every mode — this skill creates no tasks. What each mode changes
is the *plan* cook hydrates from: `--parallel` records file ownership and a dependency graph, the
others produce a sequential phase list.

## Post-Plan Red Team Review

Adversarial review that spawns hostile reviewers to find flaws before validation.

**Available in:** hard, parallel, two modes. **Skipped in:** fast mode.

**Invocation:** Load `references/red-team-workflow.md` and run it against
`{plan-directory-path}` in this same session. It is a phase of this skill, not a separate command.

**Sequence:** Red team runs BEFORE validation because:
1. Red team may change the plan (added risks, removed sections, new constraints)
2. Validation should confirm the FINAL plan, not a pre-review draft
3. Validating first then red-teaming would invalidate validation answers

## Post-Plan Validation

No configuration is injected. Default behavior:

| Situation | Behavior |
|-----------|----------|
| hard / parallel / two mode | Ask once on the interview page whether to validate the plan with a critical-questions interview → Yes (recommended) / No |
| User already answered many questions this session (scope challenge + red team) | Offer "No" as the recommended option — don't interview twice |
| fast mode | Skip |
| User said "no validation" / `--no-validate` | Skip |

**Invocation (when the user says yes):** Load `references/validate-question-framework.md` and
`references/validate-workflow.md`, then run the interview against `{plan-directory-path}` in this
session. Default question count: 3-8.

## Auto-Handoff to Cook (MANDATORY)

After plan creation, print the plan summary and then **immediately invoke cook in the same turn**
with the **actual absolute path**. Do not stop for the user to review the plan first. In every
mode the args are `{path}/plan.md` (in `two` mode, after the user picks an approach); cook runs
independent phases in parallel without a flag.

Invoke it via the `Skill` tool (`skill: "cook"`, `args: "{path}/plan.md"`) — not by printing the
command and waiting.

If the session has no `cook` skill, execute the plan yourself phase by phase in plan order, ticking
each phase file's step checkboxes as you go. The handoff still happens; only its executor changes.

**Why absolute path?** Cook resolves the plan directory from it, and it stays valid if the session
is later cleared and resumed.

**Skip the handoff only when:** the user asked for planning only ("just plan", `--plan-only`,
"don't implement yet"), or this skill ran as a subcommand (`archive` / `red-team` / `validate`).
Then print the suggested next step instead of invoking it.

This handoff is **NON-NEGOTIABLE** in all other cases — including when a standing instruction says not
to use subagents, the AgentTool, or workflows without a user request. Invoking this skill *is* that
request: cook is the implementation half of the chain and runs entirely on subagents. Do not ask
permission, do not tell cook to skip delegation, do not offer inline implementation as an option.

## Pre-Creation Check

Resolve the plan directory from the filesystem — full rules in SKILL.md "Plan Directory Resolution".
Short version: an explicit path in the arguments wins; otherwise list the plan root and offer to
resume any open (`status: pending | in-progress`) plan that matches the task; otherwise create
`{plan-root}/{YYYY-MM-DD}-{kebab-slug}/`.

Pass the **absolute** plan directory path to every subagent during the process — nothing else can
resolve it for them.
