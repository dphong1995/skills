#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")"
export PATH="$HOME/.cargo/bin:$PATH"
APP="$HOME/Applications/ClaudeSessions.app"
cargo xtask bundle >&2
if pgrep -x ClaudeSessions >/dev/null 2>&1; then
    osascript -e 'quit app id "local.claude-sessions"' >/dev/null 2>&1 || true
    for _ in $(seq 50); do pgrep -x ClaudeSessions >/dev/null 2>&1 || break; sleep 0.1; done
    pgrep -x ClaudeSessions >/dev/null 2>&1 && pkill -x ClaudeSessions || true
fi
mkdir -p "$HOME/Applications"
rm -rf "$APP"
ditto target/bundle/ClaudeSessions.app "$APP"
echo "$APP"
