# Claude Code skills — brainstorm-pro / ck-plan / cook / routing-bug-fixes-to-cook

A personal Claude Code setup: four skills (design, plan, implement, bug-fix routing), the 19 subagents they
dispatch, two hooks, one mod, always-on `CLAUDE.md` rule blocks and settings fragments. Plus three tools
that ship in the same repo: cmux settings, the OMC HUD and the ClaudeSessions macOS widget. The full
documentation is in [README.md](README.md) (Vietnamese); this file covers installing.

## Install

```bash
git clone git@github.com:dphong1995/skills.git ~/.claude/skills-repo
bash ~/.claude/skills-repo/install.sh
```

`bash install.sh` opens a menu (needs a terminal). Only `skills` is ticked by default.
The first `--update` after upgrading from the old installer has no remembered selection yet: it shows the menu with only `skills` ticked (without a terminal it exits with code 2). Pick your items once, e.g. `bash install.sh --only skills,subagent,vietnamese,compact,org-skills`.

| Item | What it installs | Where | Default |
|---|---|---|---|
| `skills` | 4 skills + 19 agents + `web-pages` rule block + todo tools (`settings/skills.json`) | `~/.claude/skills/`, `~/.claude/agents/`, `~/.claude/CLAUDE.md`, `~/.claude/settings.json` | ticked |
| `subagent` | `subagent-models` rule block + `subagent-context-guard` hook (install it with `cook`) | `~/.claude/hooks/`, `~/.claude/CLAUDE.md`, `~/.claude/settings.json` | no |
| `vietnamese` | `language` rule block + `reply-language` hook + `language: "vietnamese"` | `~/.claude/hooks/`, `~/.claude/CLAUDE.md`, `~/.claude/settings.json` | no |
| `compact` | `main-context-compact` mod + `CLAUDE_CODE_PLUGIN_DIRS` | `~/.claude/mods/`, `~/.claude/settings.json` | no |
| `org-skills` | `skillOverrides` that switch off org skills not used for code | `~/.claude/settings.json` | no |
| `cmux` | cmux settings (`cmux/install.sh`) | `~/.config/cmux/`, `~/Library/Application Support/com.cmuxterm.app/` | no |
| `hud` | OMC HUD (`hud/install.sh`) | `~/.claude/hud/`, `~/.claude/settings.json` | no |
| `widget` | ClaudeSessions widget: install the release pinned in `claude-sessions/release.lock` after verifying it (SHA-256, `codesign`, bundle id), then replace the running app; until a `release.lock` exists it builds with `swift` as before (needs macOS) | `~/Applications/ClaudeSessions.app` | no |

Flags (a usage error exits with code 2 and installs nothing):

- `bash install.sh --list`: print the items and exit.
- `bash install.sh --only a,b`: install just these items (added to the remembered selection).
- `bash install.sh --all`: install all 8 items.
- `bash install.sh --update`: reinstall the remembered selection.

The selection is remembered in `~/.claude/.skills-repo-selection` (one item name per line). Unticking an item
later does not remove anything from your machine. Files that already exist are moved to
`~/.claude/backups/skills-install-<timestamp>/` before being overwritten. Restart Claude Code after installing.

## Update

```bash
cd ~/.claude/skills-repo && git pull && bash install.sh --update
```

The install is a copy, not a symlink: rerun the installer after every `git pull`.

## Language

Without `vietnamese`, specs, plans and pages follow your language and the page chrome (buttons, labels, help)
is English. With `vietnamese`, every message to you is in Vietnamese. Pages pick `vi` or `en` from the
`language` key in `~/.claude/settings.json` (lowercased, diacritics stripped; containing `viet` or equal to
`vi` gives `vi`, anything else or an unreadable file gives `en`); `--lang vi|en` forces one. Answers sent
back from a page always use English labels (`Pick:`, `Note:`, `Decision: Approve`, ...).

## More

Everything else (what each skill does, hooks, settings, cmux, HUD, widget) is documented in
[README.md](README.md), in Vietnamese.
