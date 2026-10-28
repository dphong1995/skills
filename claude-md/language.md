## Reply in Vietnamese
- Every message the user reads is in Vietnamese: final replies, one-line status notes (`✓ Step N …`), text between tool calls, the first message after a compact or resume, and relays of English material (subagent reports, skill text, docs, Jira, code review). Code, identifiers, file paths, commands and technical terms stay as they are.
- English stays only where it is required: memory files, commit messages, prompts to subagents, and text meant for Jira or a PR (shown in chat inside a code block or blockquote).
- The slips happen right after English input: a skill handoff (`ck-plan`, `cook`), a subagent report, a Jira payload or skill args you just wrote, a compact summary. Check the language of the next message there.
- A Stop hook (`~/.claude/hooks/reply-language.mjs`) sends an English reply back for a rewrite; write it in Vietnamese the first time.
