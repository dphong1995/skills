# Jira Story Snapshot

Saves the Jira stories a round works on to `<plan-root>/jira/<KEY>.md`, one file per story, and
reports what changed since the last save. The AC traceability table (below) in the spec, the plan
and cook's review reads the AC text from these files, never from a paraphrase.

## When
- The topic or args name Jira issues (T6IR keys or Jira links) whose type is **Story** or **Epic**.
  Not for Bug or Task: the bug path (`routing-bug-fixes-to-cook` → cook's collapsed Step 1) does not
  use this file.
- Scope is the stories of the current round, the ones the user names or picks. For an Epic, ask on
  the interview page which child stories are in the round; never fetch every child by default.
- Run it at the start (brainstorm-pro Phase 1, or cook 1A when cook is entered with stories), and
  again whenever the user says the tickets changed.

## Who
`general-purpose` sub-agents with `model: "sonnet"`, at most 15 stories each, all dispatched in one
message. The orchestrator reads only their reports, not the ticket text. Give each agent this file's
path, its story keys and the absolute `<plan-root>`.

## Fetch (per story)
- `cloudId`: `getAccessibleAtlassianResources` once, reused for every call.
- `getJiraIssue(cloudId, key, view="full", properties=["acceptanceCriteria"], fields=["summary","status","issuetype","description"])`.
  Without `view: "full"` the response has no `properties` and no `issuetype`.
- `listJiraIssueComments(cloudId, key)`, every page.

AC source, the first that applies:
1. The response has `properties.acceptanceCriteria` with items: `items` is one string; split it before
   each ` AC-<n> `.
2. The response has a `properties` object without `acceptanceCriteria`, or with empty items: the
   "Acceptance criteria" heading in the description; drop the leading `- [ ] ` / `- [x] ` from each line.
3. Neither: write `- none` under the heading.

The response has no `properties` object at all: the call was wrong. Report
`<KEY> error: no properties in the response`; never fall back to the description.

## File
`<plan-root>` is the project's plan root (`plans/` in the main checkout, the same for every
worktree). Copy Jira text verbatim; never summarize, translate or reword it.

```markdown
# <KEY> — <summary>
Type: <issuetype> · Status: <status> · Fetched: <ISO time with offset>
## Acceptance criteria
- AC-1 <verbatim>
- AC-2 <verbatim>
## Description
<verbatim>
## Comments
- #<id> <author> <YYYY-MM-DD>: <verbatim>
```
No comments: write `- none` under `## Comments`.

## Compare with the last save
When `<plan-root>/jira/<KEY>.md` already exists:
1. Compare each `AC-<n>` line with whitespace collapsed, and the set of comment ids. Formatting
   differences elsewhere do not count.
2. Different → move the old file to `<plan-root>/jira/history/<KEY>.<YYYY-MM-DD>.md` (the date of its
   `Fetched:`), then write the new file. Same → leave the file untouched; do not rewrite `Fetched:`.

## Report
One line per story:
- `<KEY> new`
- `<KEY> unchanged`
- `<KEY> changed: AC-8, AC-9 (new), AC-3 (removed), comment #88650`
- `<KEY> no AC`
- `<KEY> error: <message>`

After the report:
- `error` → stop and ask on the interview page (retry, drop the story, or go on without it). Never
  go on as if the story had no AC.
- `no AC` → list the story under the spec's "Còn mở"; never invent criteria.

## Traceability table
The spec (`### Truy vết AC` under `## Thiết kế`), cook's `design.md` and `plan.md`
(`## AC Traceability`) carry this table. The AC text stays English and verbatim.

| AC | Text (verbatim) | Where | Status |
|---|---|---|---|
| 13706 AC-2 | Given the dialog, when no reason is chosen, then Remove stays disabled. | phase-04 | in scope |
| 14409 AC-10 | … | — | jira-unclear: <question> |

Status is one of: `in scope` (Where names the phase), `deferred: <reason>`,
`jira-unclear: <question>`, `out of scope: <reason>`.

Rows in the table:
- First save of a set of stories: every AC of the round's stories.
- A re-run: the `changed` and `new` ACs, plus rows that were ❌ or `deferred` in the previous round.
  ACs reported `unchanged` that were ✅ before are not re-listed.
