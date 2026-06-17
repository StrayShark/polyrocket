#!/usr/bin/env bash
# v0.37a — Rotate the snapshot history.
#
# Keeps the last 7 days of snapshots in
# docs/previews/history/YYYY-MM-DD/. Each day
# gets its own subdir; older days are deleted.
#
# Usage:
#   ./scripts/rotate-snapshots.sh           # rotate the current snapshot
#   ./scripts/rotate-snapshots.sh --days=14 # keep 14 days
#
# The "current" snapshot is at docs/previews/;
# the "history" snapshots are at
# docs/previews/history/YYYY-MM-DD/.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
PREVIEWS_DIR="$REPO_ROOT/docs/previews"
HISTORY_DIR="$PREVIEWS_DIR/history"

# Default: keep 7 days
DAYS=7
for arg in "$@"; do
  case "$arg" in
    --days=*) DAYS="${arg#--days=}" ;;
    *) echo "Unknown arg: $arg" >&2; exit 1 ;;
  esac
done

if [ ! -d "$PREVIEWS_DIR" ]; then
  echo "Error: $PREVIEWS_DIR does not exist" >&2
  echo "Run snapshot_pages.py first to generate snapshots" >&2
  exit 1
fi

mkdir -p "$HISTORY_DIR"

# Today's date in YYYY-MM-DD
TODAY=$(date -u +%Y-%m-%d)
TODAY_DIR="$HISTORY_DIR/$TODAY"

# If today's snapshot already exists, skip (the
# user has already rotated today). The user
# can pass --force to overwrite.
if [ -d "$TODAY_DIR" ]; then
  echo "Today's snapshot ($TODAY) already exists at $TODAY_DIR"
  echo "Skipping. Remove the dir to force re-rotation."
  exit 0
fi

echo "Rotating snapshot: $PREVIEWS_DIR → $TODAY_DIR"
mv "$PREVIEWS_DIR" "$TODAY_DIR"

# Re-create the empty previews dir (the snapshot
# script will write to it on the next run).
mkdir -p "$PREVIEWS_DIR"

# Delete old history dirs (older than N days)
echo "Pruning history older than $DAYS days..."
DELETED=0
for d in "$HISTORY_DIR"/*/; do
  [ -d "$d" ] || continue
  dir_name=$(basename "$d")
  # Parse the date; if invalid, skip
  if ! date -d "$dir_name" -u +%Y-%m-%d >/dev/null 2>&1; then
    continue
  fi
  # Compute days ago
  days_ago=$(python3 -c "
from datetime import date, datetime
d = date.fromisoformat('$dir_name')
today = date.today()
print((today - d).days)
")
  if [ "$days_ago" -gt "$DAYS" ]; then
    echo "  removing $dir_name ($days_ago days old)"
    rm -rf "$d"
    DELETED=$((DELETED + 1))
  fi
done

echo ""
echo "✓ Rotation complete"
echo "  today: $TODAY"
echo "  history kept: $DAYS days"
echo "  pruned: $DELETED older dir(s)"
