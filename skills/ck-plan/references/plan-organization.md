# Plan Creation & Organization

## Directory Structure

### Plan Location

Compute it yourself — see SKILL.md "Plan Directory Resolution". Shape:
`{plan-root}/{YYYY-MM-DD}-{kebab-slug}/`, where `{plan-root}` is the project's existing plan
directory (`plans/`, else `ai_docs/`, else `docs/plans/`; create `plans/` if none exists).

**Example:** `plans/2026-09-10-oauth-authentication/`

If the task has a tracker id, put it in the slug: `plans/2026-09-10-TALOS-371-form-keys/`.

### File Organization

IN CURRENT WORKING PROJECT DIRECTORY:
```
{plan-dir}/                                    # Resolved per SKILL.md
├── research/
│   ├── researcher-XX-report.md
│   └── ...
├── reports/
│   ├── scout-report.md
│   ├── researcher-report.md
│   └── ...
├── plan.md                                    # Overview access point
├── phase-01-setup-environment.md              # Setup environment
├── phase-02-implement-database.md             # Database models
├── phase-03-implement-api-endpoints.md        # API endpoints
├── phase-04-implement-ui-components.md        # UI components
├── phase-05-implement-authentication.md       # Auth & authorization
├── phase-06-implement-profile.md              # Profile page
└── phase-07-write-tests.md                    # Tests
```

### Task Hydration — not done here

`cook` owns task hydration. After creating plan.md and phase files, hand off; do not call
`TaskCreate` / `TaskUpdate` in this skill.

### Active Plan State Tracking

There is no state file and nothing injects the active plan. Resolution is re-derived every run from
the filesystem — see SKILL.md "Plan Directory Resolution". Consequences:

- `plan.md` frontmatter `status:` **is** the source of truth for whether a plan is still open.
  Keep it accurate; a stale `in-progress` is what a later run will offer to resume.
- Always pass the absolute plan path explicitly to cook and to every subagent. Nothing can look it
  up for them.
- Reports go under the plan directory (`{plan-dir}/reports/`), never a shared global path.

## Plan Creation

After determining phases from research/design, write the files directly with `Write` — no CLI, no
scaffolding tool:

1. **Create the plan directory** and write `plan.md` with full YAML frontmatter (schema in
   `output-standards.md`) plus `## Overview`, the `## Phases` table, and `## Dependencies`.

2. **Write one `phase-XX-{slug}.md` per phase**, numbered zero-padded and in execution order, each
   with: architecture, related code files, implementation steps, todo checkboxes, success criteria,
   risk assessment, security considerations.

3. **Keep the Phases table in lock-step with the phase files.** You own it end to end: one row per
   `phase-XX-*.md`, in the same order, link text = the human-readable phase name (not the filename),
   `Status` starting at `Pending`.

4. **Status updates are plain edits.** To mark a phase done, edit its row's `Status` cell to
   `Complete` and tick the phase file's todo boxes (`[ ]` → `[x]`). To add a phase, write the new
   `phase-XX-*.md` and insert its row — renumber following phases only if you also rename their
   files.

**Verify before handing off:** `ls {plan-dir}` and confirm every `phase-XX-*.md` on disk has exactly
one row in the table and every row resolves to a file that exists.

## File Structure

### Overview Plan (plan.md)

**IMPORTANT:** All plan.md files MUST include YAML frontmatter. See `output-standards.md` for schema.

**Example plan.md structure:**
```markdown
---
title: "Feature Implementation Plan"
description: "Add user authentication with OAuth2 support"
status: pending
priority: P1
effort: 8h
issue: 123
branch: kai/feat/oauth-auth
tags: [auth, backend, security]
created: 2025-12-16
---

# Feature Implementation Plan

## Overview

Brief description of what this plan accomplishes.

## Phases

| Phase | Name | Status |
|-------|------|--------|
| 1 | [Setup Environment](./phase-01-setup.md) | Pending |
| 2 | [Core Implementation](./phase-02-impl.md) | Pending |
| 3 | [Testing & Validation](./phase-03-test.md) | Pending |

<!-- IMPORTANT: Link text MUST be human-readable names (not filenames).
     Bad:  [phase-01-setup.md](./phase-01-setup.md)
     Good: [Setup Environment](./phase-01-setup.md) -->

## Dependencies

- List key dependencies here

## AC Traceability
<!-- Only when the spec or design has "Truy vết AC". Copy AC, Text and Status verbatim and fill Where with the owning phase file; format: ../brainstorm-pro/references/jira-snapshot.md § Traceability table. -->
| AC | Text (verbatim) | Where | Status |
|---|---|---|---|
```

**Guidelines:**
- Keep generic and under 80 lines
- List each phase with status/progress
- Link to detailed phase files
- Key dependencies
- The AC Traceability table is copied whole and does not count toward the 80 lines.

### Phase Files (phase-XX-name.md)
Fully respect the project's own conventions file (`CLAUDE.md` / `AGENTS.md`, plus anything under `docs/`).
Each phase file should contain:

**Context Links**
- Links to related reports, files, documentation

**Overview**
- Priority
- Current status
- Brief description

**Key Insights**
- Important findings from research
- Critical considerations

**Requirements**
- Functional requirements
- Non-functional requirements

**Architecture**
- System design
- Component interactions
- Data flow

**Related Code Files**
- List of files to modify
- List of files to create
- List of files to delete

**Implementation Steps**
- Detailed, numbered steps
- Specific instructions

**Todo List**
- Checkbox list for tracking

**Success Criteria**
- Definition of done
- Validation methods
- The ids of the AC Traceability rows this phase owns (e.g. "T6IR-13706 AC-2"), when plan.md has the table

**Risk Assessment**
- Potential issues
- Mitigation strategies

**Security Considerations**
- Auth/authorization
- Data protection

**Next Steps**
- Dependencies
- Follow-up tasks
