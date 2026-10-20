#!/usr/bin/env bash
# Install the OMC HUD statusline in this folder.
# Existing files are kept next to the original as <file>.<timestamp>.bak.
# settings.json: only the statusLine and omcHud keys are replaced; other keys are kept.
set -euo pipefail

SRC="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
STAMP="$(date +%Y%m%d-%H%M%S)"

install_file() {  # $1 = source path, $2 = destination path
  if [ -e "$2" ]; then
    cp -p "$2" "$2.$STAMP.bak"
  fi
  mkdir -p "$(dirname "$2")"
  cp "$1" "$2"
  echo "  $2"
}

install_file "$SRC/omc-hud.mjs" "$HOME/.claude/hud/omc-hud.mjs"
install_file "$SRC/PATCH-NOTES.md" "$HOME/.claude/hud/PATCH-NOTES.md"

SETTINGS="$HOME/.claude/settings.json"
if [ -e "$SETTINGS" ]; then
  cp -p "$SETTINGS" "$SETTINGS.$STAMP.bak"
fi
node -e '
const fs = require("fs");
const [target, source] = process.argv.slice(1);
const settings = fs.existsSync(target) ? JSON.parse(fs.readFileSync(target, "utf8")) : {};
const hud = JSON.parse(fs.readFileSync(source, "utf8"));
fs.writeFileSync(target, JSON.stringify({ ...settings, ...hud }, null, 2) + "\n");
' "$SETTINGS" "$SRC/settings.json"
echo "  $SETTINGS (statusLine, omcHud)"
