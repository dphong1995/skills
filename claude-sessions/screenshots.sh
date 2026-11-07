#!/bin/bash
# Regenerates docs/images/*.png from the snapshot tests (transparent background, trimmed to the widget).
# Extra arguments go to `swift test` (e.g. -Xswiftc -plugin-path -Xswiftc <CLT testing plugins dir> in a sandbox).
set -euo pipefail
cd "$(dirname "$0")"

names=(
  07-mixed-hover 06-mixed-collapsed 10-last-reply-hover 11-pending-hover
  question-4-two-questions stop-armed 16-todo 15-titles-branch
  35-background-agents 22-pr-lines 31-pr-awaiting-approval 29-link-list-open
  34-mark-both quota-3-high-forecast closed-2-open
)

command -v magick >/dev/null || { echo "magick (ImageMagick) not found" >&2; exit 1; }

rm -rf .build/snapshots
SNAPSHOT_TRANSPARENT=1 swift test --filter SnapshotTests "$@"

missing=0
for n in "${names[@]}"; do
  if [ ! -f ".build/snapshots/$n.png" ]; then
    echo "missing snapshot: $n" >&2
    missing=1
  fi
done
[ "$missing" = 0 ] || exit 1

rm -rf docs/images && mkdir -p docs/images
for n in "${names[@]}"; do
  magick ".build/snapshots/$n.png" -trim +repage -strip -define png:exclude-chunks=date,time "docs/images/$n.png"
done
