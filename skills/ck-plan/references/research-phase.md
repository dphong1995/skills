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
synthesizing. Have each write its report to `{plan-dir}/research/researcher-{NN}-{aspect}.md` and
return only the path plus a short summary.

### Structured Reasoning
- Think through the problem in the main session before and after the research fan-out: enumerate
  approaches, state assumptions explicitly, then revise as reports land
- No external thinking tool is required — extended thinking in this session is the mechanism

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

### Debugger Delegation
- Delegate to `debugger` agent for root cause analysis
- Use when investigating complex issues or bugs
- Debugger agent specializes in diagnostic tasks

## Best Practices

- Research breadth before depth
- Document findings for synthesis phase
- Identify multiple approaches for comparison
- Consider edge cases during research
- Note security implications early
