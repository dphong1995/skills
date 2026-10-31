# Jira Story Snapshot

Saves the Jira stories a round works on to `<plan-root>/jira/<KEY>.md`, one file per story, and
reports what changed since the last save. The AC traceability table (below) in the spec, the plan
and cook's review reads the AC text from these files, never from a paraphrase.

## When
- The topic or args name Jira issues (keys or links). The sub-agents keep Story and Epic issues (an issue type whose name starts with `Story`, e.g. T6IR's `Story/New Feature Request`, or `Epic`) and report the rest as `skipped`. Bug fixes do not run this file: the bug path (`routing-bug-fixes-to-cook` → cook's collapsed Step 1) skips it.
- Scope is the stories of the current round, the ones the user names or picks. For an Epic, ask on
  the interview page which child stories are in the round; never fetch every child by default.
- Run it at the start (brainstorm-pro Phase 1, or cook 1A when cook is entered with stories), and
  again whenever the user says the tickets changed.

## Who
`general-purpose` sub-agents with `model: "sonnet"`, at most 15 issues each, all dispatched in one message. Give each agent this file's path, its issue keys and the absolute `<plan-root>`. The orchestrator does not read ticket text in the agents' reports; it reads the saved `<plan-root>/jira/<KEY>.md` of the stories it designs and copies the traceability table's AC text from there.

## Fetch (per issue)
- `getJiraIssue(cloudId, key, view="full", properties=["acceptanceCriteria"], fields=["summary","status","issuetype","description"])`.
- `listJiraIssueComments(cloudId, key)`, every page.

The response has no `issuetype`: the call was wrong (without `view: "full"` the response has neither
`issuetype` nor `properties`). Report `<KEY> error: no issuetype in the response`; never fall back to
the description. `issuetype` is not a Story type or Epic: write nothing and report
`<KEY> skipped: <issuetype>`.

AC source, the first that applies:
1. `properties.acceptanceCriteria` with items: `items` is one string; split it before each ` AC-<n> `.
2. One heading named "Acceptance criteria" in the description: its lines, without a leading `- [ ] ` /
   `- [x] `. Two or more such headings: report `<KEY> error: several AC headings in the description`.
3. Neither: write `- none` under the heading.

Keep an item's own `AC-<n>`; number unnumbered description lines in order.

## File
`<plan-root>`: the plan root from ck-plan's SKILL.md § Plan Directory Resolution; in a worktree, the main checkout's, so every worktree shares one `jira/`. Copy Jira text verbatim; never summarize, translate or reword it.

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
1. Compare the `AC-<n>` lines under `## Acceptance criteria` (whitespace collapsed) and the set of
   comment ids.
2. Different → move the old file to `<plan-root>/jira/history/<KEY>.<YYYY-MM-DDTHHMM>.md` (its
   `Fetched:` time), then write the new file.
3. Same, but the summary, status or `## Description` differ (whitespace collapsed) → rewrite the file
   in place, no history copy; report `<KEY> unchanged, details edited`.
4. Nothing differs → leave the file untouched; do not rewrite `Fetched:`.

## Report
One line per issue:
- `<KEY> new`
- `<KEY> unchanged`
- `<KEY> unchanged, details edited`
- `<KEY> changed: AC-8, AC-9 (new), AC-3 (removed), comment #88650`
- `<KEY> no AC` (on every run while it has none; append `, changed: comment #<id>` when comments changed)
- `<KEY> skipped: <issuetype>`
- `<KEY> error: <message>`

Orchestrator, after the report:
- `error` → stop and ask on the interview page (retry, drop the story, or go on without it). Never
  go on as if the story had no AC.
- `no AC` → list the story under the spec's "Còn mở"; never invent criteria.

## Traceability table
The spec (`### Truy vết AC` under `## Thiết kế`), cook's `design.md` and `plan.md`
(`## AC Traceability`) carry this table. The AC text stays English and verbatim.

| AC | Text (verbatim) | Where | Status |
|---|---|---|---|
| T6IR-13706 AC-2 | Given the dialog, when no reason is chosen, then Remove stays disabled. | phase-04-remove-verification.md | in scope |
| T6IR-14409 AC-10 | … | — | jira-unclear: <question> |

Status is one of: `in scope` (in plan.md, Where names the phase file), `deferred: <reason>`,
`jira-unclear: <question>`, `out of scope: <reason>`. Where is `—` in the spec and design (no phases yet); plan.md fills in the owning phase file.

Rows in the table:
- First save of a set of stories: every AC of the round's stories.
- A re-run in the same round (the spec or design is not approved yet): keep the current table;
  update the rows of `changed` ACs, add the `new` ones, and mark removed ACs `out of scope: removed in Jira`.
- A new round on stories saved before: the `changed` and `new` ACs, plus every row of the previous
  round's table whose Status is not `in scope`. Find that table in the newest plan.md whose
  `## AC Traceability` names the story, searching the main checkout's plan root and each worktree's
  (`git worktree list`). `in scope` rows of unchanged ACs are not re-listed: they passed that round's
  review (a Section A ❌ blocks cook's finalize).
