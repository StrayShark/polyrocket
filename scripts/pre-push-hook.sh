#!/usr/bin/env bash
# polyrocket — pre-push git hook (v0.73a — HARDENED).
#
# Blocks `git push` unless `./scripts/run-ci-local.sh` passes
# AND its CI_VERIFIED state file matches the current HEAD. The
# v0.69h version had two escape hatches that allowed a sloppy
# push to slip through:
#
#   1. `POLYROCKET_PRE_PUSH_SKIP=1 git push` — fully bypass
#   2. `./scripts/run-ci-local.sh --quick` — skip cargo test
#
# Both were removed in v0.73a because they kept the door open
# for "remote pipeline keeps erroring" — the failure mode where
# local green but remote red, often from race conditions or a
# toolchain mismatch (Rust 1.96 pinned, pnpm 9 vs 11, glib-2.0
# missing on ubuntu-latest). When the local gate is bypassed or
# short-circuited, the user only learns the truth when GitHub
# Actions turns red, which wastes 10-15 minutes of remote
# runner time per bad push.
#
# **What's left as bypass**:
#   - `git push --no-verify` — git's native flag. We can't
#     block it without modifying git itself. It's reserved
#     for emergencies (e.g., re-pushing a known-good CI-green
#     commit to recover a broken remote state).
#
# **What is enforced**:
#   - Local pipeline MUST run to completion (all 4 jobs).
#   - Cargo MUST run (no `--quick`).
#   - The CI_VERIFIED state file MUST exist AND match the
#     current HEAD SHA. If you make a new commit, the state
#     file becomes stale and the gate re-runs the full
#     pipeline from scratch.
#
# Install:
#   ./scripts/install-ci-hook.sh         # local repo
#   make install-pre-push                # same, via Makefile
#
# Uninstall (NOT recommended):
#   rm .git/hooks/pre-push

set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel)"
RUNNER="$REPO_ROOT/scripts/run-ci-local.sh"
STATE_FILE="$REPO_ROOT/.git/CI_VERIFIED"

# ----- Detect CI environment ------------------------------------------
# If we're running inside GitHub Actions, the CI IS the check;
# recursion would deadlock. Auto-skip is preserved here.
if [ -n "${GITHUB_ACTIONS:-}" ] || [ -n "${CI:-}" ]; then
  echo "[pre-push] CI environment detected — skipping local CI gate"
  exit 0
fi

# ----- Toolchain preflight --------------------------------------------
# Fail fast and LOUDLY if the toolchain isn't ready. Better to
# block push with a clear install hint than to push and have CI
# fail with the same hint.
if [ ! -x "$RUNNER" ]; then
  echo "[pre-push] ✗ $RUNNER not found or not executable"
  echo "Run: ./scripts/install-ci-hook.sh"
  exit 2
fi

# ----- Block banner ----------------------------------------------------
echo "================================================================"
echo "[pre-push] polyrocket CI gate (v0.73a — HARDENED)"
echo "================================================================"
echo "Local pipeline MUST pass before push is allowed."
echo "Bypass (NOT recommended): git push --no-verify"
echo "================================================================"
echo

# ----- State file check -----------------------------------------------
# The runner writes $STATE_FILE on success. If the file is missing,
# stale, or for a different HEAD, we re-run the full pipeline.
CURRENT_HEAD="$(git rev-parse HEAD)"

need_rerun=1
if [ -f "$STATE_FILE" ]; then
  STATE_HEAD="$(grep -E '^sha=' "$STATE_FILE" 2>/dev/null | head -1 | cut -d= -f2 || echo "")"
  STATE_TS="$(grep -E '^timestamp=' "$STATE_FILE" 2>/dev/null | head -1 | cut -d= -f2 || echo "")"
  STATE_JOBS="$(grep -E '^jobs=' "$STATE_FILE" 2>/dev/null | head -1 | cut -d= -f2 || echo "")"
  NOW="$(date +%s)"
  STATE_AGE=$(( NOW - ${STATE_TS:-0} ))

  if [ "$STATE_HEAD" = "$CURRENT_HEAD" ] && [ "$STATE_JOBS" = "all" ] && [ "$STATE_AGE" -lt 3600 ]; then
    echo "[pre-push] ✓ CI_VERIFIED state is fresh (head=$STATE_HEAD, age=${STATE_AGE}s)"
    echo "[pre-push] ✓ Skipping re-run. Push proceeds."
    echo
    exit 0
  else
    echo "[pre-push] CI_VERIFIED state is stale or mismatched:"
    [ -n "$STATE_HEAD" ] && echo "  recorded head : $STATE_HEAD"
    echo "  current head  : $CURRENT_HEAD"
    [ -n "$STATE_JOBS" ] && echo "  recorded jobs : $STATE_JOBS"
    [ -n "$STATE_TS" ] && echo "  recorded at   : $STATE_TS (${STATE_AGE}s ago)"
    echo "[pre-push] Re-running full local pipeline..."
  fi
else
  echo "[pre-push] No CI_VERIFIED state found — running full local pipeline..."
fi

# ----- v0.87fix — GHA CI gate ----------------------------------------
# Query GitHub Actions for the last run on this branch. Block the push
# if the previous GHA run on this commit failed. Catches the
# "local green but remote red" failure mode (e.g. v0.87's cross-
# platform baseline mismatch that local CI couldn't catch).

if [ "${POLYROCKET_PRE_PUSH_SKIP_GHA:-0}" != "1" ]; then
  if [ -f "$REPO_ROOT/scripts/check-gha-ci.sh" ]; then
    if ! "$REPO_ROOT/scripts/check-gha-ci.sh"; then
      echo
      echo "================================================================"
      echo "[pre-push] ✗ GHA CI check FAILED — push BLOCKED"
      echo "================================================================"
      exit 1
    fi
  else
    echo "[pre-push] WARN: scripts/check-gha-ci.sh not found — skipping GHA check"
  fi
fi

# ----- Run the local pipeline -----------------------------------------
echo
if "$RUNNER"; then
  echo
  echo "================================================================"
  echo "[pre-push] ✓ local CI PASSED for head $CURRENT_HEAD"
  echo "================================================================"
  exit 0
else
  RC=$?
  echo
  echo "================================================================"
  echo "[pre-push] ✗ local CI FAILED (exit $RC) — push BLOCKED"
  echo "================================================================"
  echo
  echo "Fix the failing job, then retry:"
  echo "  $RUNNER"
  echo
  echo "Emergency bypass (NOT recommended — remote CI may still fail):"
  echo "  git push --no-verify"
  echo
  echo "To permanently disable this hook (DANGEROUS):"
  echo "  rm $REPO_ROOT/.git/hooks/pre-push"
  echo
  exit 1
fi