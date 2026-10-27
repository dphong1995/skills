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
conditional best-practice research → one-question-at-a-time Q&A → 2-3 approaches → sectioned
design with per-section approval. Where brainstorm-pro writes `spec.md` and hands off to
`ck-plan`, cook skips the spec: the orchestrator writes the plan files itself from the approved
design, then delegates execution to sub-agents. The plan files still use **`ck-plan`'s format**
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
[Setup] → [Design: scout → research → Q&A → approaches → sectioned design]
        → [Plan] → [Phase sub-agents] → [Test sub-agent] → [Review: 3-agent fan-out] → [Finalize sub-agents]
```

Step 1 (Design) is interactive — one question at a time on an interview Artifact page (answers
polled from the page), approval per design section in chat.
From Step 2 onward it runs end to end without pausing; only the final commit is confirmed.

## Sub-agent Roles

| Step | Sub-agent |
|------|-----------|
| Design (1A/1B reading only) | `document-specialist`, `Explore` — optional; 1C/1D/1E are orchestrator-run |
| Planning | **none** — orchestrator writes the plan itself from the approved design block, in `ck-plan`'s format |
| Implementation (one per phase) | `executor` / `designer` (frontend) |
| Testing | `test-engineer`, `debugger` |
| Review (one fan-out) | `code-reviewer` ×3 — compliance+bugs (gating) / reuse / simplification+efficiency+altitude |
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

5.1.0 - Step 1 interview questions (1C, 1D approach pick) asked on an Artifact page shared with brainstorm-pro, answers polled from its db
4.0.0 - Step 1 replaced by the brainstorm-pro design flow (scout → research → Q&A → approaches → sectioned design), no spec handoff
3.0.0 - Single workflow, mandatory sub-agent per phase (modes removed)
2.1.0 - Review gates added for human-in-the-loop mode
2.0.0 - Smart intent detection (hybrid approach)
