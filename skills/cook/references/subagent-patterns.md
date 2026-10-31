# Subagent Patterns

Standard patterns for spawning and using subagents in cook workflows.

## Agent Tool Pattern
```
Agent(subagent_type="[type]", prompt="[task description]", description="[brief]")
```
The tool is named `Agent` here and `Task` in some harnesses — use whichever this session exposes.
A report starting with `PARTIAL` (from the context guard hook) is re-dispatched to a fresh agent, see the "Subagent models" rule in CLAUDE.md.

## Every prompt
Every prompt cook writes for a sub-agent (any type, including `general-purpose` and `designer`, whose definitions do not carry it) ends with these two lines:

> Batch independent reads: when you need several files, line ranges, diffs or searches that do not depend on each other, request them in one turn: parallel tool calls, or one Bash command that prints several ranges (`sed -n '1,80p' a.ts; sed -n '40,120p' b.ts`). Read a long file or diff with parallel offset reads in one turn, not one page per turn. Read files you will edit with parallel `Read` calls (Edit needs a prior Read); the Bash form is for files you only read. Go one call at a time only when the next read depends on what the previous one showed. Every turn re-reads your whole context, so fewer turns cost less.

> You are a sub-agent of a cook run. Do not invoke any skill (the Skill tool), even one whose description says to always use it. Do not spawn fork, executor, designer, reviewer or any other agent that edits or reviews; you may spawn up to 3 read-only `Explore` agents to locate code. If the task is too large for one agent, stop and report BLOCKED with a proposed split.

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

Older names that no longer exist anywhere — `fullstack-developer`, `ui-ux-designer`, `tester`,
`project-manager`, `docs-manager`, `researcher`, `scout` — map through the table
above. Do not dispatch them directly.

## Step 1A/1B Helpers (OPTIONAL — Step 1 itself is orchestrator-run)
```
Agent(subagent_type="document-specialist", prompt="Research [topic]. Report ≤150 lines.", description="Research [topic]")
Agent(subagent_type="Explore", prompt="Find files related to [feature] in codebase. Breadth: medium.", description="Explore [feature]")
```
- Only for parallel reading during deep scout (1A) / best-practice research (1B)
- Keep reports ≤150 lines with citations; the orchestrator owns synthesis, Q&A and design
- NEVER delegate the design conversation (1C/1D/1E) — it stays with the orchestrator. One exception: the 1E builder check runs per `../brainstorm-pro/references/builder-check.md` and reads the written design.md; the orchestrator triages its questions and still owns Q&A, the design and the review page. When cook dispatches it, append the § Every prompt lines like any other prompt cook writes

## Planning Phase — NO SUB-AGENT

There is no planning sub-agent in cook. The orchestrator writes `plan.md` + `phase-XX-*.md`
itself with the Write tool, next to the design the user approved in Step 1E (`<plan-dir>/design.md`; there is no
`spec.md` in the cook flow). Dispatching `planner` here is FORBIDDEN — it would re-derive
requirements the user already settled in the design conversation.

Each plan file carries: the Acceptance Criteria verbatim, per-phase scope, explicit file
ownership, and "Done when" criteria — those blocks are what get pasted into the Step 3
sub-agent prompts.

## Prompts for Steps 3-6

The prompts for Steps 3-6 are defined in `workflow-steps.md` (§ Step 3, § Step 4, § Step 5, § Step 6). This file holds the agent mapping and the § Every prompt lines.
