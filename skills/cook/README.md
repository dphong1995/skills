# Cook Skill

End-to-end feature implementation. Every implementation phase is dispatched to a dedicated sub-agent — the orchestrator never edits production code in-line.

## Installation

Copy the `cook/` folder to your Claude skills directory:
```bash
cp -r cook ~/.claude/skills/
```

## Usage

```bash
/cook <natural language task OR plan path>
```

- New work → full workflow (**brainstorm-pro design** → plan → per-phase sub-agents → test → review → finalize).
- Path to `plan.md` / `phase-*.md` → skip design & planning, start at Step 3.

When no plan is supplied, Step 1 is the `brainstorm-pro` flow run in-line: deep scout →
conditional best-practice research → one-question-at-a-time Q&A → 2-3 approaches → the design
written to `design.md` and approved on a local review page (notes per section). Where brainstorm-pro
writes `spec.md` and hands off to `ck-plan`, cook writes the plan files itself next to the approved
`design.md`, then delegates execution to sub-agents. The plan files still use **`ck-plan`'s format**
(frontmatter, `plan.md` skeleton, phase-file sections) — cook borrows the format without delegating
the writing.

## Examples

```bash
# New feature, full workflow
/cook implement user authentication

# Execute an existing plan
/cook plans/2026-01-20-auth/plan.md
```

## Workflow

```
[Setup] → [Design: scout → research → Q&A → approaches → design.md reviewed on a page]
        → [Plan] → [Phase sub-agents] → [Test sub-agent] → [Review: sized by the diff] → [Finalize sub-agents]
```

Step 1 (Design) is interactive — one round at a time on a local HTML question page (independent
questions may share a page; the page sends the answers back, copy-and-paste as the fallback), then the design on a review page with a note box per section and a Duyệt button.
From Step 2 onward it runs end to end without pausing; only the final commit is confirmed.

## Sub-agent Roles

| Step | Sub-agent |
|------|-----------|
| Design (1A/1B reading only) | `document-specialist`, `Explore` — optional; 1C/1D/1E are orchestrator-run |
| Planning | **none** — orchestrator writes the plan itself from the approved design block, in `ck-plan`'s format |
| Implementation (one per phase) | `executor` / `designer` (frontend) |
| Testing | `test-engineer`, `debugger` |
| Review (sized by the diff) | `code-reviewer` ×1 (small diff) or ×2 (large) — compliance (incl. `in scope` AC rows checked against the saved Jira text)+bugs (gating) / reuse+simplification+efficiency+altitude |
| Fix application | `code-simplifier` (cleanup-only) / `executor` / `debugger` |
| Finalize | `executor` (sync-back), `writer` (docs), `git-master` (commit) |

## Files

```
cook/
├── SKILL.md                           # Main skill definition
├── README.md                          # This file
└── references/
    ├── design-phase.md                # Step 1 — brainstorm-pro design flow, in-line
    ├── workflow-steps.md              # Step definitions
    ├── review-cycle.md                # Review process
    └── subagent-patterns.md           # Subagent usage
```

## Version

5.7.0 - Step 5 sized by the diff (one reviewer for a small diff, two for a large one); re-reviews scoped to the fix
5.6.3 - Step 5 has one severity scale, agent 1's: a cleanup finding gates only when agent 2 labels it OUT-OF-LENS and it grades Critical/High; the old Critical Issues list that made DRY, performance and architecture findings critical is gone
5.6.2 - Implementation, testing, fix-application and finalize subagents are dispatched with `model: "sonnet"` (no exception for hard fixes), matching the always-on `subagent-models` rule
5.6.1 - 1E no longer mentions a section-by-section design step; the design goes straight to `design.md` and its review page
5.6.0 - 1E writes the design to `<plan-dir>/design.md` in Vietnamese (brainstorm-pro's spec structure) and the user approves it on a local review page with notes per section; `plan.md` links `design.md` instead of copying it; question pages pick an option when its card is clicked
5.5.0 - Step 5 review is a 2-agent fan-out: one cleanup agent runs Reuse (first, with a `Reuse searched:` line) + Simplification + Efficiency + Altitude, so Reuse/Simplification duplicates are collapsed by the agent that read the code; a subsumed finding whose higher fix changes behaviour or leaves the diff is kept as a `Fallback:`
5.4.0 - The interview page sends the answers back through a local server started in the background (copy-and-paste stays as the fallback); file paths in the text open in the viewer; screenshots and wireframe mockups; the page is styled with Tailwind
5.3.0 - Interview questions shown as a local HTML page, one round at a time (up to 4 independent questions per page); a floating button copies every answer for the user to paste back (no Artifact, no polling)
5.2.0 - The interview page wakes the session when the user answers (comment sent to Claude); polling only as a fallback
5.1.0 - Step 1 interview questions (1C, 1D approach pick) asked on an Artifact page shared with brainstorm-pro, answers polled from its db
4.0.0 - Step 1 replaced by the brainstorm-pro design flow (scout → research → Q&A → approaches → sectioned design), no spec handoff
3.0.0 - Single workflow, mandatory sub-agent per phase (modes removed)
2.1.0 - Review gates added for human-in-the-loop mode
2.0.0 - Smart intent detection (hybrid approach)
