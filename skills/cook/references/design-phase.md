# Step 1: Design (brainstorm-pro flow, in-line)

Runs ONLY when no plan path was provided. Read `~/.claude/skills/brainstorm-pro/references/design-flow.md` and run its Phases 1–6 as written — unless § Escalation below collapses Step 1. Do not read `~/.claude/skills/brainstorm-pro/SKILL.md`: its Phase 7 hands off to ck-plan, which cook never does.

**Terminal difference vs `brainstorm-pro`:** where brainstorm-pro writes `spec.md` and hands
off to `ck-plan`, cook writes the design to `<plan-dir>/design.md` and, once the user approves it,
goes to Step 2, where the **orchestrator itself** writes the plan files next to it (no `planner`
sub-agent), and execution continues in the same run.

**The orchestrator runs this step itself** — scout/Q&A/design are conversational and must not
be delegated. Sub-agents are allowed only as research helpers in 1A/1B and the builder check in 1E.

## Mapping to `design-flow.md`

| Cook sub-step | `design-flow.md` section |
|---|---|
| 1A Deep Scout | Phase 1 |
| 1B Best-Practice Research | Phase 1.5 |
| 1C Q&A Loop (incl. the "Thẻ phát lại" playback card) | Phase 2 |
| 1D Propose 2-3 Approaches | Phase 3 |
| 1E Design on a review page (write, self-review + builder check, user review gate) | Phases 4–6 |

In those phases, *the document* = `<plan-dir>/design.md` (resolve the plan directory before Phase 4:
ck-plan's SKILL.md § "Plan Directory Resolution"; see `workflow-steps.md` § Step 2) and *the next
step* = Step 2.

## Cook-only rules

- 1A: **Save the Jira stories** when the topic names Jira issues (not in the collapsed bug-fix Step 1): run `../brainstorm-pro/references/jira-snapshot.md` (Sonnet agents in the background; it keeps Story/Epic only). Read their one-line reports, then the saved files of the stories you design.

**1A budget:** 5-15 actions typical. `document-specialist` / `Explore` sub-agents may be dispatched in
parallel for distinct areas (reports carry conclusions and citations, not file dumps); the orchestrator still owns synthesis.

Anti-Rationalization rows (in addition to `design-flow.md`'s):

| Thought | Reality |
|---------|---------|
| "Cook is supposed to run without stopping" | The no-stop rule starts at Step 2. Step 1 is interactive by design. |
| "Design approved, let me start editing files" | No. Write the plan yourself first (Step 2), then dispatch sub-agents. |
| "Design approved, hand it to `planner`" | No `planner` in cook. You held the conversation, so you write the plan. |

**Outputs** (one line per sub-step):

- `✓ Step 1A: Scout complete - [N] actions, [M] reuse opportunities`
- `✓ Step 1B: Research [done - N sources | skipped - <reason>]`
- `✓ Step 1C: Q&A complete - [N] rounds, playback ok`
- `✓ Step 1D: Approach selected - [name]`
- `✓ Step 1E: Design approved - [N] sections, [M] acceptance criteria, [K] review rounds, builder check: N câu — A đã có, M nhỏ, K hỏi bạn`

`design.md` is not a `spec.md` handoff: no ck-plan, and the plan files land next to it.

## Escalation

- **User says "just code it" / "skip planning"** → respect it. Collapse Step 1 to: minimal scout
  (≤3 actions) + one confirmation question + a 5-line design block, then go to Step 2.
- **Bug fix handed off by `routing-bug-fixes-to-cook`** (root cause confirmed) → same collapse, with
  the investigation as the scout: Q&A only for a genuinely open question, then ONE interview-page
  question that is the approval: the candidate fixes as options with a `recommendation` (a single
  candidate → options Approve (Duyệt) / Revise (Cần sửa)) and the 5-line design block in `context`; files that differ
  per candidate go in each option's `means`. A listed fix or Approve starts Step 2, with no chat
  approval, no `design.md` and no review page. Revise (Cần sửa) or `Other: <text>` → apply the note to the design block
  and ask again under a new id.
- **Topic too large** → decompose into sub-projects; cook only the first.
- **Scope keeps drifting** → freeze scope explicitly; document drifted items as Open Items.
- **No clear recommendation possible** → present approaches with trade-offs, ask the user to
  pick, do NOT push.

## Design Step Checklist

- [ ] Every item of the `design-flow.md` checklist is ticked
- [ ] `<plan-dir>/design.md` written in the user's language (the `language` setting when set, else the language the user writes in) (with its Done criteria (Tiêu chí hoàn thành)), self-reviewed, approved on its review page
- [ ] No implementation code written during Step 1
- [ ] No `spec.md` written and no handoff to `ck-plan` — Step 2 continues in this run
