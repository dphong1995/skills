# Output Standards & Quality

## Plan File Format

### YAML Frontmatter (Required for plan.md)

All `plan.md` files MUST include YAML frontmatter at the top:

```yaml
---
title: "{Brief plan title}"
description: "{One-sentence summary for card preview}"
status: pending  # pending | in-progress | completed | cancelled
priority: P2     # P1 (High) | P2 (Medium) | P3 (Low)
effort: 4h       # Estimated total effort
issue: 74        # GitHub issue number (if applicable)
branch: kai/feat/feature-name
tags: [frontend, api]  # Category tags
created: 2025-12-16
---
```

### Auto-Population Rules

When creating plans, auto-populate these fields:
- **title**: Extract from task description
- **description**: First sentence of Overview section
- **status**: Always `pending` for new plans
- **priority**: From user request or default `P2`
- **effort**: Sum of phase estimates
- **issue**: Parse from branch name or context
- **branch**: Current git branch (`git branch --show-current`)
- **tags**: Infer from task keywords (e.g., frontend, backend, api, auth)
- **created**: Today's date in YYYY-MM-DD format

### Tag Vocabulary (Recommended)

Use these predefined tags for consistency:
- **Type**: `feature`, `bugfix`, `refactor`, `docs`, `infra`
- **Domain**: `frontend`, `backend`, `database`, `api`, `auth`
- **Scope**: `critical`, `tech-debt`, `experimental`

### Task Naming Conventions

**subject** (imperative): Action verb + deliverable, <60 chars
  Examples: "Setup database migrations", "Implement OAuth2 flow"

**activeForm** (continuous): Present participle of subject
  Examples: "Setting up database", "Implementing OAuth2"

**description**: 1-2 sentences, concrete deliverables, reference phase file

See `task-management.md` for full TaskCreate patterns and metadata.

How to size phases and write steps and file lists: `plan-writing.md` § Sizing phases, § Phase file and § Steps.

## Output Requirements

### What Planners Do
- Create plans ONLY (no implementation)
- Provide plan file path and summary
- Self-contained plans with necessary context
- Code only as `plan-writing.md` § Steps allows
- Multiple options with trade-offs when appropriate
- Fully respect the project's own conventions file (`CLAUDE.md` / `AGENTS.md`, plus anything under `docs/`).

### Writing Style
Plan text is pasted verbatim into prompts for agents with no other context, so each block must read on its own:
- Short, complete sentences; bullets for steps and file lists
- Cut words that don't change what the reader would do; keep the ones that make a step unambiguous
- Prioritize actionable info

### Unresolved Questions
At the end, ask unresolved questions on the interview page (`~/.claude/skills/brainstorm-pro/references/html-interview.md`):
- Questions needing clarification
- Technical decisions requiring input
- Unknowns impacting implementation
- Trade-offs requiring business decisions
Revise the plan and phases based on the answers.

## Quality Standards

- Detailed enough for a developer (or agent) with no other context to implement each phase; validated against existing patterns and codebase standards
- Cover the edge cases, failure modes, security and performance concerns this change actually raises; plan no scalability, configurability or abstraction the task did not ask for
- When uncertain, research; give trade-offs where the options are real
- Document assumptions and the rationale for each decision
- Fully respect the project's own conventions file (`CLAUDE.md` / `AGENTS.md`, plus anything under `docs/`).
