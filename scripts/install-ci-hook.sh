#!/usr/bin/env bash
# polyrocket — install the pre-push CI gate (v0.69h).
#
# Copies scripts/pre-push-hook.sh to .git/hooks/pre-push and
# chmods it. Idempotent — re-run to update the hook after a
# v0.69h+ commit changes the script.
#
# Usage:
#   ./scripts/install-ci-hook.sh
#
# Verify:
#   cat .git/hooks/pre-push   # should match scripts/pre-push-hook.sh
#
# Uninstall:
#   rm .git/hooks/pre-push

set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel)"
HOOK_SRC="$REPO_ROOT/scripts/pre-push-hook.sh"
HOOK_DST="$REPO_ROOT/.git/hooks/pre-push"

if [ ! -f "$HOOK_SRC" ]; then
  echo "✗ $HOOK_SRC not found"
  exit 1
fi

cp "$HOOK_SRC" "$HOOK_DST"
chmod +x "$HOOK_DST"

echo "✓ Installed pre-push hook at $HOOK_DST"
echo
echo "From now on, 'git push' will:"
echo "  1. Run scripts/run-ci-local.sh (4 jobs, mirrors CI)"
echo "  2. Block the push if any job fails"
echo
echo "Bypass:"
echo "  git push --no-verify"
echo "  POLYROCKET_PRE_PUSH_SKIP=1 git push"
echo
echo "Uninstall:"
echo "  rm .git/hooks/pre-push"
