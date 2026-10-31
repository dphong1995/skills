# Subagent Patterns

Standard patterns for spawning and using subagents in cook workflows.

## Agent Tool Pattern
```
Agent(subagent_type="[type]", prompt="[task description]", description="[brief]")
```
The tool is named `Agent` here and `Task` in some harnesses — use whichever this session exposes.
A report starting with `PARTIAL` (from the context guard hook) is re-dispatched to a fresh agent, see the "Subagent models" rule in CLAUDE.md.

## Every prompt
Every prompt cook writes for a sub-agent (any type, including `general-purpose` and `designer`, whose definitions do not carry it) ends with this line:

> Batch independent reads: when you need several files, line ranges, diffs or searches that do not depend on each other, request them in one turn (parallel tool calls, or one Bash command that prints several ranges). Go one call at a time only when the next read depends on the previous one.

## Agent Name Mapping (CHECK FIRST)

Agent types are per-session. **Verify every `subagent_type` against the session's agent registry
before dispatching**, and fall back rightward. A missing name is never a reason to skip a step or to
do the work inline.

| Role in this skill | Type, first available wins |
|--------------------|----------------------------|
| Implementation (backend / general) | `executor` → `general-purpose` |
| Implementation (frontend / UI) | `designer` → `executor` → `general-purpose` |
| Codebase file discovery | `Explore` → `explore` → `general-purpose` |
| External docs / prior art | `document-specialist` → `general-purpose` |
| Requirements analysis | `analyst` → `general-purpose` |
| Testing | `test-engineer` → `executor` → `general-purpose` |
| Debugging / root cause | `debugger` → `tracer` → `general-purpose` |
| Code review | `code-reviewer` → `critic` → `general-purpose` |
| Plan sync-back (mechanical file edits) | `executor` → `general-purpose` |
| Docs update | `writer` → `general-purpose` |
| Git / commit | `git-master` → `general-purpose` |

Older names that no longer exist anywhere — `fullstack-developer`, `ui-ux-designer`, `tester`,
`project-manager`, `docs-manager`, `git-manager`, `researcher`, `scout` — map through the table
above. Do not dispatch them directly.

## Step 1A/1B Helpers (OPTIONAL — Step 1 itself is orchestrator-run)
```
Agent(subagent_type="document-specialist", prompt="Research [topic]. Report ≤150 lines.", description="Research [topic]")
Agent(subagent_type="Explore", prompt="Find files related to [feature] in codebase. Breadth: medium.", description="Explore [feature]")
```
- Only for parallel reading during deep scout (1A) / best-practice research (1B)
- Keep reports ≤150 lines with citations; the orchestrator owns synthesis, Q&A and design
- NEVER delegate 1C/1D/1E — the design conversation stays with the orchestrator

## Planning Phase — NO SUB-AGENT

There is no planning sub-agent in cook. The orchestrator writes `plan.md` + `phase-XX-*.md`
itself with the Write tool, next to the design the user approved in Step 1E (`<plan-dir>/design.md`; there is no
`spec.md` in the cook flow). Dispatching `planner` here is FORBIDDEN — it would re-derive
requirements the user already settled in the design conversation.

Each plan file carries: the Acceptance Criteria verbatim, per-phase scope, explicit file
ownership, and "Done when" criteria — those blocks are what get pasted into the Step 3
sub-agent prompts.

## UI Implementation
```
Agent(subagent_type="designer", prompt="Implement [feature] UI following the project's design-system conventions (see CLAUDE.md and the UI package)", description="UI [feature]")
```
- For frontend work
- Follow the project's own design/UI conventions — discover them, don't assume a filename

## Testing
```
Agent(subagent_type="test-engineer", prompt="Run test suite for plan phase [phase-name]", description="Test [phase]")
```
- Must achieve 100% pass rate

## Debugging
```
Agent(subagent_type="debugger", prompt="Analyze failures: [details]", description="Debug [issue]")
```
- Use when tests fail
- Provides root cause analysis

## Code Review
```
Agent(subagent_type="code-reviewer", prompt="Review changes for [phase]. Check security, performance, YAGNI/KISS/DRY. Return score (X/10), critical, warnings, suggestions.", description="Review [phase]")
```

## Plan Sync-Back
```
Agent(subagent_type="executor", prompt="Run full sync-back in [plan-path]: reconcile completed tasks with all phase files, backfill stale completed checkboxes across all phases, update plan.md status/progress, and report unresolved mappings.", description="Update plan")
```

## Documentation
```
Agent(subagent_type="writer", prompt="Update docs for [phase]. Changed files: [list]", description="Update docs")
```

## Git Operations
```
Agent(subagent_type="git-master", prompt="Stage and commit changes with conventional commit message", description="Commit changes")
```

## Parallel Execution
```
Agent(subagent_type="executor", prompt="Implement [phase-file] with file ownership: [files]", description="Implement phase [N]")
```
- Launch multiple for parallel phases — all in a single message so they run concurrently
- Include file ownership boundaries
