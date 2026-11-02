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
├── phase-01-oauth-provider-config.md          # one phase per deliverable
├── phase-02-token-exchange-api.md
├── phase-03-login-ui.md
└── phase-04-live-login-check.md               # live check of the whole flow; no single phase owns it
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

1. **Create the plan directory** and write `plan.md`: frontmatter (`output-standards.md`) and the
   sections in `plan-writing.md` § plan.md.

2. **Write one `phase-XX-{deliverable}.md` per phase**, numbered zero-padded and in execution
   order, with the sections in `plan-writing.md` § Phase file.

3. **Keep the Phases table in lock-step with the phase files.** You own it end to end: one row per
   `phase-XX-*.md`, in the same order, link text = the human-readable phase name (not the filename),
   `Status` starting at `Pending`. Example:

| Phase | Name | Status |
|-------|------|--------|
| 1 | [OAuth Provider Config](./phase-01-oauth-provider-config.md) | Pending |
| 2 | [Token Exchange API](./phase-02-token-exchange-api.md) | Pending |
| 3 | [Login UI](./phase-03-login-ui.md) | Pending |
| 4 | [Live Login Check](./phase-04-live-login-check.md) | Pending |

<!-- IMPORTANT: Link text MUST be human-readable names (not filenames).
     Bad:  [phase-01-oauth-provider-config.md](./phase-01-oauth-provider-config.md)
     Good: [OAuth Provider Config](./phase-01-oauth-provider-config.md) -->

4. **Status updates are plain edits.** To mark a phase done, edit its row's `Status` cell to
   `Complete` and tick its step checkboxes (`[ ]` → `[x]`). To add a phase, write the new
   `phase-XX-*.md` and insert its row — renumber following phases only if you also rename their
   files.

**Then run `plan-writing.md` § Self-Review** (its Format check includes the `ls {plan-dir}` check).


## File Structure

The `plan.md` and phase-file templates live in `plan-writing.md`.
