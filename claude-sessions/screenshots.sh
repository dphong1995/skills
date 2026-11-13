#!/bin/bash
# Regenerates docs/images/*.png from the Rust snapshot test (offscreen `render_png`, transparent background, trimmed to the
# widget): no window is opened. Extra arguments go to `cargo test` (before the `--`).
set -euo pipefail
cd "$(dirname "$0")"

names=(
  07-mixed-hover 06-mixed-collapsed 10-last-reply-hover 11-pending-hover
  question-4-two-questions stop-armed 16-todo 15-titles-branch
  35-background-agents 22-pr-lines 31-pr-awaiting-approval 29-link-list-open
  34-mark-both quota-3-high-forecast closed-2-open
  40-procs-live-open 41-procs-closed-open
)

command -v magick >/dev/null || { echo "magick (ImageMagick) not found" >&2; exit 1; }

export PATH="$HOME/.cargo/bin:$PATH"
snaps=target/snapshots
rm -rf "$snaps"
CLAUDE_SESSIONS_GUI_TESTS=1 cargo test -p claude-sessions --test gui "$@"

missing=0
for n in "${names[@]}"; do
  if [ ! -f "$snaps/$n.png" ]; then
    echo "missing snapshot: $n" >&2
    missing=1
  fi
done
[ "$missing" = 0 ] || exit 1

rm -rf docs/images && mkdir -p docs/images
for n in "${names[@]}"; do
  magick "$snaps/$n.png" -trim +repage -strip -define png:exclude-chunks=date,time "docs/images/$n.png"
done
