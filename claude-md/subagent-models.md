## Subagent models
- Spawn every subagent that executes a task with `model: "sonnet"`: executor, test-engineer, debugger, code-simplifier, writer, git-master, and general-purpose agents doing implementation, fixes, tests, docs, commits or plan sync-back.
- Spawn review-only agents with `model: "opus"`: red-team reviewers, cook's Step 5 code-reviewers, critic, architect, verifier, security-reviewer.
- This holds even where a skill names another model or says "unless the fix is genuinely hard".
- Why: predictable cost and latency.
