#!/usr/bin/env bash
# Install the cmux settings in this folder.
# Existing files are kept next to the original as <file>.<timestamp>.bak.
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

install_file "$SRC/cmux.json" "$HOME/.config/cmux/cmux.json"
install_file "$SRC/config.ghostty" "$HOME/Library/Application Support/com.cmuxterm.app/config.ghostty"

if command -v cmux >/dev/null 2>&1; then
  cmux reload-config || echo "cmux is not running; the settings apply on next launch."
fi
