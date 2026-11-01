# Research & Analysis Phase

**When to skip:** If provided with research reports, skip this phase.

## Core Activities

### Parallel Research Agents

Dispatch via the `Agent` tool, all in one message so they run concurrently. Pick the agent type that
exists in this session's registry — check the list before dispatching, and fall back rightward:

| Need | Agent type (first available wins) |
|------|-----------------------------------|
| External docs, libraries, prior art | `document-specialist` → `general-purpose` |
| Requirements analysis, ambiguity | `analyst` → `general-purpose` |
| Architecture options, trade-offs | `architect` → `general-purpose` |
| Root-cause of an existing defect | `debugger` → `general-purpose` |

Max 2 in parallel for hard/parallel mode, max 5 tool calls each. Wait for all of them before
synthesizing. These agent types cannot write files: each returns its report, and you save it to
`{plan-dir}/research/researcher-{NN}-{aspect}.md`.

### Documentation Research
- Library / framework / SDK / CLI docs: use the **context7** MCP tools
  (`resolve-library-id`, then `query-docs`) — current docs beat training data, even for
  well-known libraries
- Anything context7 does not cover: `WebSearch`, then `WebFetch` on the authoritative page
- Record the exact version you looked up; a plan pinned to the wrong major version is worse than no
  plan

### GitHub Analysis
- Use `gh` command to read and analyze:
  - GitHub Actions logs
  - Pull requests
  - Issues and discussions
- Extract relevant technical context from GitHub resources

### Remote Repository Analysis
When given GitHub repository URL, generate fresh codebase summary:
```bash
# usage: 
repomix --remote <github-repo-url>
# example: 
repomix --remote https://github.com/mrgoonie/human-mcp
```
