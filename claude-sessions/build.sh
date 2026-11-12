#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")"
export PATH="$HOME/.cargo/bin:$PATH"
APP="$HOME/Applications/ClaudeSessions.app"
cargo xtask bundle >&2
if pgrep -x ClaudeSessions >/dev/null 2>&1; then
    osascript -e 'quit app id "local.claude-sessions"' >/dev/null 2>&1 || true
    for _ in $(seq 50); do pgrep -x ClaudeSessions >/dev/null 2>&1 || break; sleep 0.1; done
    if pgrep -x ClaudeSessions >/dev/null 2>&1; then
        pkill -x ClaudeSessions || true
        for _ in $(seq 20); do pgrep -x ClaudeSessions >/dev/null 2>&1 || break; sleep 0.5; done
        if pgrep -x ClaudeSessions >/dev/null 2>&1; then
            pkill -9 -x ClaudeSessions || true
            for _ in $(seq 20); do pgrep -x ClaudeSessions >/dev/null 2>&1 || break; sleep 0.5; done
        fi
    fi
fi
mkdir -p "$HOME/Applications"
NEW="$HOME/Applications/.ClaudeSessions.app.new"
rm -rf "$NEW"
ditto target/bundle/ClaudeSessions.app "$NEW" || { rm -rf "$NEW"; echo "build.sh: cannot copy the app; the installed app was left alone" >&2; exit 1; }
rm -rf "$APP"
mv "$NEW" "$APP"
echo "$APP"
