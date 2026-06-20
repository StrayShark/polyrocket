#!/usr/bin/env bash
# v0.87fix — pre-push hook: query GHA CI status for the last run
# on the current branch. If the last GHA run on this branch
# failed, block the push (user must fix the remote first).
#
# **Why**: local CI is fast (~3 min) but doesn't catch GHA-specific
# issues (Linux vs macOS renderer, GHA-only env vars, etc.). The
# v0.87 push (a407d2d) passed local CI 5/5 but GHA failed on
# Playwright e2e due to a cross-platform baseline mismatch. If we
# had this check, the bad push would have been blocked.
#
# **Limitations**:
# - Requires `gh` CLI authenticated with repo read access.
# - Only checks the LAST run on the current branch. If the user
#   pushed 3 commits in quick succession, only the most recent
#   GHA run is checked.
# - GHA run takes ~5-15 min. Pushing again before GHA completes
#   will see "no run found" (skipped check) and proceed.
# - 5-min timeout for the gh API call to keep push snappy.
#
# **Bypass**: `POLYROCKET_PRE_PUSH_SKIP_GHA=1 git push` (NOT
# recommended — set only if GHA is having an outage).
#
# **Exit codes**:
#   0 — last GHA run passed (or no run found) → allow
#   1 — last GHA run failed → block push

set -e

REPO_ROOT="$(git rev-parse --show-toplevel)"
BRANCH="$(git rev-parse --abbrev-ref HEAD)"

# v0.87fix — bypass if env var is set
if [ "${POLYROCKET_PRE_PUSH_SKIP_GHA:-0}" = "1" ]; then
  echo "[pre-push/gha] SKIPPED (POLYROCKET_PRE_PUSH_SKIP_GHA=1)"
  exit 0
fi

echo "[pre-push/gha] Checking GHA CI status for branch '$BRANCH'..."

# Auth check
if ! gh auth status > /dev/null 2>&1; then
  echo "[pre-push/gha] WARN: gh CLI not authenticated — skipping GHA check"
  echo "[pre-push/gha] (Run \`gh auth login\` to enable GHA pre-push check)"
  exit 0
fi

# Get the most recent GHA run on this branch
RUN_INFO=$(gh run list \
  --branch "$BRANCH" \
  --limit 1 \
  --json status,conclusion,headSha,name 2>/dev/null) || {
  echo "[pre-push/gha] WARN: gh run list failed (timeout or API error) — skipping"
  exit 0
}

if [ -z "$RUN_INFO" ] || [ "$RUN_INFO" = "[]" ]; then
  echo "[pre-push/gha] No GHA runs found for branch '$BRANCH' — skipping (first push?)"
  exit 0
fi

CONCLUSION=$(echo "$RUN_INFO" | python3 -c "import json,sys; print(json.load(sys.stdin)[0].get('conclusion', ''))")
STATUS=$(echo "$RUN_INFO" | python3 -c "import json,sys; print(json.load(sys.stdin)[0].get('status', ''))")
HEAD_SHA=$(echo "$RUN_INFO" | python3 -c "import json,sys; print(json.load(sys.stdin)[0].get('headSha', ''))")
LOCAL_SHA=$(git rev-parse HEAD)

echo "[pre-push/gha] Last GHA run: $HEAD_SHA (status=$STATUS, conclusion=$CONCLUSION)"
echo "[pre-push/gha] Local HEAD  : $LOCAL_SHA"

# Only check if the GHA run is for the SAME commit we're pushing
# (the latest GHA run might be for a prior commit if user is pushing again
# before GHA finishes)
if [ "$HEAD_SHA" != "$LOCAL_SHA" ]; then
  echo "[pre-push/gha] Last GHA run is for a different commit — skipping"
  echo "[pre-push/gha] (GHA may still be running for this push; check manually)"
  exit 0
fi

if [ "$CONCLUSION" = "success" ]; then
  echo "[pre-push/gha] ✓ GHA CI PASSED for this commit"
  exit 0
elif [ "$STATUS" = "in_progress" ] || [ "$STATUS" = "queued" ]; then
  echo "[pre-push/gha] WARN: GHA CI still running for this commit — cannot verify"
  echo "[pre-push/gha] (Wait for GHA to finish, or push --no-verify if confident)"
  # Don't block — running is not failure
  exit 0
elif [ "$CONCLUSION" = "failure" ]; then
  echo
  echo "================================================================"
  echo "[pre-push/gha] ✗ GHA CI FAILED for this commit — push BLOCKED"
  echo "================================================================"
  echo
  echo "Fix the failing GHA job first, then retry push."
  echo "View the failed run:"
  echo "  gh run view $HEAD_SHA --web"
  echo
  echo "Bypass (NOT recommended — only for GHA outages):"
  echo "  POLYROCKET_PRE_PUSH_SKIP_GHA=1 git push --no-verify"
  echo
  exit 1
else
  echo "[pre-push/gha] WARN: GHA status=$STATUS conclusion=$CONCLUSION — skipping"
  exit 0
fi
