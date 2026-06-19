#!/usr/bin/env bash
# polyrocket — install the pre-push CI gate (v0.73a — HARDENED).
#
# Copies scripts/pre-push-hook.sh to .git/hooks/pre-push and
# chmods it. Idempotent — re-run to update the hook after a
# v0.69h+ commit changes the script.
#
# The v0.73a hardening removed the env-var bypass
# (POLYROCKET_PRE_PUSH_SKIP=1) and the --quick flag from the
# runner. Only `git push --no-verify` remains as an escape hatch,
# which git itself provides and which we cannot block.
#
# Usage:
#   ./scripts/install-ci-hook.sh
#
# Verify:
#   cat .git/hooks/pre-push           # should match scripts/pre-push-hook.sh
#   grep -c POLYROCKET_PRE_PUSH_SKIP scripts/pre-push-hook.sh   # should be 0
#
# Uninstall (NOT recommended):
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
echo "  1. Read .git/CI_VERIFIED state file"
echo "  2. If state is fresh + matches HEAD → push immediately"
echo "  3. Otherwise run scripts/run-ci-local.sh (4 jobs, mirrors CI)"
echo "  4. Block the push if any job fails"
echo
echo "v0.73a hardening (no bypass):"
echo "  - POLYROCKET_PRE_PUSH_SKIP=1 is REMOVED"
echo "  - run-ci-local.sh --quick is REMOVED"
echo "  - All 4 jobs (including cargo) MUST run"
echo
echo "Emergency bypass (NOT recommended — remote CI may still fail):"
echo "  git push --no-verify"
echo
echo "Uninstall (DANGEROUS):"
echo "  rm $HOOK_DST"