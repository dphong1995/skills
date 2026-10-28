#!/usr/bin/env bash
# Install the skills and agents in this repo into ~/.claude.
# Existing files with the same name are moved to ~/.claude/backups/ first.
set -euo pipefail

SRC="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DEST="${CLAUDE_HOME:-$HOME/.claude}"
BACKUP="$DEST/backups/skills-install-$(date +%Y%m%d-%H%M%S)"

install_entry() {  # $1 = source path, $2 = destination path
  if [ -e "$2" ]; then
    mkdir -p "$(dirname "$BACKUP/${2#$DEST/}")"
    mv "$2" "$BACKUP/${2#$DEST/}"
  fi
  mkdir -p "$(dirname "$2")"
  cp -R "$1" "$2"
}

mkdir -p "$DEST/skills" "$DEST/agents"

for skill in "$SRC"/skills/*/; do
  name="$(basename "$skill")"
  install_entry "${skill%/}" "$DEST/skills/$name"
  echo "  skill  $name"
done

for agent in "$SRC"/agents/*.md; do
  name="$(basename "$agent")"
  install_entry "$agent" "$DEST/agents/$name"
  echo "  agent  $name"
done

find "$DEST/skills" "$DEST/agents" -name '.DS_Store' -delete 2>/dev/null || true

# Always-on rules: each claude-md/<name>.md is kept in $DEST/CLAUDE.md between its own markers.
# The rest of that file is left alone; the text between the markers is replaced on every install.
for block in "$SRC"/claude-md/*.md; do
  [ -e "$block" ] || continue
  name="$(basename "$block" .md)"
  target="$DEST/CLAUDE.md"
  begin="<!-- skills-repo: $name -->"
  end="<!-- /skills-repo: $name -->"
  if [ -e "$target" ] && [ ! -e "$BACKUP/CLAUDE.md" ]; then
    mkdir -p "$BACKUP"
    cp "$target" "$BACKUP/CLAUDE.md"
  fi
  tmp="$(mktemp)"
  if [ -f "$target" ] && grep -qxF "$begin" "$target"; then
    if ! grep -qxF "$end" "$target"; then
      echo "$target has \"$begin\" without \"$end\"; fix it by hand and run again." >&2
      exit 1
    fi
    awk -v b="$begin" -v e="$end" -v f="$block" '
      $0 == b { print; while ((getline line < f) > 0) print line; skip = 1; next }
      $0 == e { skip = 0 }
      !skip { print }' "$target" > "$tmp"
  else
    if [ -s "$target" ]; then cat "$target"; printf '\n'; fi > "$tmp"
    { printf '%s\n' "$begin"; cat "$block"; printf '%s\n' "$end"; } >> "$tmp"
  fi
  cat "$tmp" > "$target"
  rm -f "$tmp"
  echo "  block  $name -> CLAUDE.md"
done

echo
echo "Installed into $DEST"
if [ -d "$BACKUP" ]; then echo "Replaced files backed up to $BACKUP"; fi
echo "Restart Claude Code (or run /doctor) to pick them up."
