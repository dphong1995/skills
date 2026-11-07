# Builder Check

A fresh agent reads the finished document the way the engineer who will plan and build it would, and reports what it would still have to guess. It catches what the author, who wrote from the conversation, cannot see is missing.

## When

- After the four checks of Phase 5, before Phase 6.
- Once per document. It re-runs only when a `Decision: Revise` round changed the chosen approach (the pick line in `## Approach` (Cách làm)); wording edits or added rows in the rejected-alternatives table do not re-run it.
- Never on shortened branches ("just code it", cook's bug-fix shortcut).

## Invoke

`Agent` with `subagent_type: "general-purpose"`, `model: "opus"`, a fresh agent (never `fork`: a fork carries the whole conversation), foreground. Prompt: the text below, with `[REPO_ROOTS]` replaced by the repo root(s) and `[DOCUMENT_PATH]` by the document path.

If no subagent tool is available, read the document yourself in the planner's role and answer the same way.

```text
You are the engineer who will turn a design document into an implementation plan and then build it
through subagents. You cannot talk to its author or to the user, and you were told to skip discovery:
the document is your whole requirement.

Read CLAUDE.md in [REPO_ROOTS], then [DOCUMENT_PATH]. You may open files the document cites to check
a claim. Do not explore further and do not redesign. Treat every row of "## Decisions" (Quyết định) as settled,
whoever made it. The document is in the user's language (the `language` setting when set, else the language the user writes in), with section names per `design-flow.md` § Phase 4; answer in English.

Report only the questions you would still need answered before you could plan this well: places
where you would have to guess and a wrong guess would matter. For each give the question, the
section it comes from, and why the answer changes the plan. Rank them by importance.

Skip what "## Open items" (Còn mở) leaves open on purpose, anything you could reasonably decide yourself, and
wording nitpicks. If nothing important is left, say "nothing important left".
```

## Triage

Sort each reported question into one of three groups:

1. **Already answered** in the conversation, the scout or the code: write the answer into the document; set Source (Nguồn) by who decided it.
2. **Small**: a reasonable builder could decide it without changing what gets built. Decide it yourself and add a `## Decisions` row with Source `Claude`.
3. **Yours (the user's)**: only the user can answer and the answer changes what gets built. Ask them in one interview round on the question page (`~/.claude/skills/brainstorm-pro/references/html-interview.md`, full cards as in Phase 2): at most 4 independent questions. Any beyond 4 go to `## Open items` with their options; so does a question that depends on another one's answer, unless, given that answer, it still changes what gets built — then ask it in one more round.

## After

Update the document with the answers, re-run the four checks of Phase 5 on the changed parts, then go to Phase 6. Report one line:

`✓ Builder check: N câu — A đã có, M nhỏ, K hỏi bạn`
