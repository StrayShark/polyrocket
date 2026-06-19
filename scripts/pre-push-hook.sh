#!/usr/bin/env bash
# polyrocket — pre-push git hook (v0.69h).
#
# Blocks `git push` unless `./scripts/run-ci-local.sh` passes
# (or the user passes --no-verify). Without this, every push
# is a gamble: "did my local checks actually mirror CI's
# ubuntu-latest environment?" — the v0.69 story proved the
# answer is often "no" (dlopen2_derive edition2024 / glib-2.0 /
# dist/index.html stub all hid on macOS until CI surfaced them).
#
# Install:
#   ./scripts/install-ci-hook.sh         # local repo
#   make install-pre-push                # same, via Makefile
#
# Bypass (when you really need to push a WIP branch):
#   git push --no-verify
#
# Override the runner:
#   POLYROCKET_PRE_PUSH_SKIP=1 git push  # env override
#   ./scripts/run-ci-local.sh --quick     # skip cargo (saves ~1m)

set -euo pipefail

# Skip switch — for emergency pushes, parallel branches, or
# when running inside the CI runner itself (which would recurse).
if [ "${POLYROCKET_PRE_PUSH_SKIP:-0}" = "1" ]; then
  echo "[pre-push] POLYROCKET_PRE_PUSH_SKIP=1 — skipping local CI"
  exit 0
fi

# Detect CI environment — if we're already running inside GitHub
# Actions, the CI is THE check, don't recurse.
if [ -n "${GITHUB_ACTIONS:-}" ] || [ -n "${CI:-}" ]; then
  echo "[pre-push] CI environment detected — skipping local CI"
  exit 0
fi

REPO_ROOT="$(git rev-parse --show-toplevel)"
RUNNER="$REPO_ROOT/scripts/run-ci-local.sh"

if [ ! -x "$RUNNER" ]; then
  echo "[pre-push] ERROR: $RUNNER not found or not executable"
  echo "Run: ./scripts/install-ci-hook.sh"
  exit 2
fi

echo "[pre-push] Running local CI gate before push..."
echo "(set POLYROCKET_PRE_PUSH_SKIP=1 to bypass, or git push --no-verify)"
echo

if "$RUNNER"; then
  echo
  echo "[pre-push] ✓ local CI PASSED — push proceeds"
  exit 0
else
  RC=$?
  echo
  echo "[pre-push] ✗ local CI FAILED (exit $RC) — push BLOCKED"
  echo
  echo "To bypass this gate (NOT recommended):"
  echo "  git push --no-verify"
  echo "  POLYROCKET_PRE_PUSH_SKIP=1 git push"
  echo
  echo "To debug, run the runner directly:"
  echo "  $RUNNER"
  exit 1
fi
