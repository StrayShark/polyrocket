#!/usr/bin/env bash
# polyrocket — run all CI jobs locally (v0.73a — HARDENED).
#
# Why this exists: each v0.69 fix surfaced an issue that local
# macOS verification missed because:
#   - macOS has Rust 1.96, so 1.85/1.88/1.89 toolchain fixes
#     trivially pass.
#   - macOS already has a leftover dist/ from prior pnpm build,
#     so the tauri::generate_context!() macro doesn't fire its
#     'frontendDist path doesn't exist' panic.
#   - macOS uses webkit via brew (different sys libs from Linux),
#     so missing glib-2.0 on ubuntu-latest isn't reproducible.
#   - pnpm 11 (local) tolerates empty pnpm-workspace.yaml;
#     pnpm 9 (CI) doesn't.
#
# This script mirrors the .github/workflows/ci.yml jobs in order,
# simulating CI's clean state. It runs on macOS (skipping Linux
# apt-get) and on Linux (full pass). The v0.73a release removes
# the `--quick` flag (which used to skip cargo for ~1 minute
# savings) because every "remote pipeline erroring" incident in
# v0.69-v0.72 was traced back to a --quick push where cargo was
# the silent gap. Cargo is now always required.
#
# Jobs mirrored (must match .github/workflows/ci.yml):
#   1. governance guards    (no setup-node needed; pure node scripts)
#   2. L1 typecheck + vitest (setup-node 20 + pnpm 9 + install +
#                              typecheck + vitest + coverage + readme sync)
#   3. Rust cargo test      (cargo 1.89 + dist stub + build + test --threads=1)
#   4. Python sidecar       (pip install -e . + pytest)
#
# Exit codes:
#   0 = all pass (safe to push; state file written)
#   N = job N failed (1-indexed: 1=governance, 2=L1, 3=Rust, 4=Python)
#
# State file (v0.73a):
#   On success, writes .git/CI_VERIFIED with:
#     sha=<current_head_sha>
#     timestamp=<unix_ts>
#     jobs=all
#     runner=<hostname>:<pid>
#   The pre-push hook reads this file and skips re-running if it's
#   fresh (<1h old) and matches HEAD. If you make a new commit, the
#   file becomes stale and the hook re-runs.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_ROOT"

# ----- Args ------------------------------------------------------------
for arg in "$@"; do
  case "$arg" in
    --help|-h)
      sed -n '3,55p' "$0"
      exit 0
      ;;
    --quick)
      echo "✗ --quick flag was removed in v0.73a — cargo must run for every push."
      echo "  (The --quick path was the silent gap behind every 'remote pipeline erroring' incident.)"
      echo "  Re-run without --quick."
      exit 2
      ;;
    *) echo "Unknown arg: $arg"; exit 2 ;;
  esac
done

# ----- Toolchain detection ---------------------------------------------
# CI uses rustc 1.89, pnpm 9, node 20. We check what's installed and
# either use it (if compatible) or fail loudly with install hint.
RUSTC_VERSION=""
if command -v rustup >/dev/null 2>&1; then
  RUSTC_VERSION="$(rustup run 1.89 rustc --version 2>/dev/null | awk '{print $2}' || echo "")"
fi

NODE_VERSION="$(node --version 2>/dev/null | tr -d 'v' || echo "")"
PNPM_VERSION=""
if command -v pnpm >/dev/null 2>&1; then
  PNPM_VERSION="$(pnpm --version 2>/dev/null || echo "")"
fi
PYTHON_VERSION="$(python3 --version 2>/dev/null | awk '{print $2}' || echo "")"

echo "============================================================"
echo "polyrocket local CI runner (v0.73a — HARDENED)"
echo "============================================================"
echo "Platform    : $(uname -s)/$(uname -m)"
echo "Node        : ${NODE_VERSION:-NOT FOUND}"
echo "pnpm        : ${PNPM_VERSION:-NOT FOUND}"
echo "Rust 1.89   : ${RUSTC_VERSION:-NOT INSTALLED}"
echo "Python 3    : ${PYTHON_VERSION:-NOT FOUND}"
echo "============================================================"
echo

# Toolchain preflight — fail fast if anything is missing so the
# user knows to install BEFORE push, not after a red CI.
FAIL=0
[ -z "$NODE_VERSION" ] && { echo "✗ node not found — install Node 20.x"; FAIL=1; }
[ -z "$PNPM_VERSION" ] && { echo "✗ pnpm not found — npm install -g pnpm@9"; FAIL=1; }
[ -z "$RUSTC_VERSION" ] && { echo "✗ rustc 1.89 not installed — rustup toolchain install 1.89 --profile minimal"; FAIL=1; }
[ -z "$PYTHON_VERSION" ] && { echo "✗ python3 not found — install Python 3.9+"; FAIL=1; }
if [ "$FAIL" = "1" ]; then
  echo
  echo "Toolchain preflight failed. Fix the above before running again."
  exit 2
fi

# ----- Job 1: governance guards ----------------------------------------
echo "[1/4] governance guards"
echo "--- L1↔Tauri guard"
node scripts/check-l1-tauri.mjs
echo "--- Layer rules"
node scripts/check-layers.mjs
echo "--- 3-theme contrast (WCAG AA)"
node scripts/check-theme-contrast.mjs
echo "--- comment density"
node scripts/check-comment-density.mjs
echo "✓ governance guards PASS"
echo

# ----- Job 2: L1 typecheck + vitest -----------------------------------
echo "[2/4] L1 typecheck + vitest"
echo "--- pnpm install"
pnpm install --frozen-lockfile
echo "--- pnpm typecheck"
pnpm typecheck
echo "--- pnpm test (vitest run)"
pnpm test
echo "--- pnpm test:coverage"
pnpm test:coverage > /dev/null
echo "--- update-readme-coverage (drift check)"
if node scripts/update-readme-coverage.mjs 2>&1 | grep -q "no changes needed"; then
  echo "✓ README badges in sync"
else
  echo "✗ README badges DRIFT — run: node scripts/update-readme-coverage.mjs"
  exit 1
fi
# Clean up coverage/ (matches CI's 'if: always() rm -rf coverage')
rm -rf coverage
echo "✓ L1 typecheck + vitest PASS"
echo

# ----- Job 3: Rust cargo test (REQUIRED in v0.73a — no --quick) ------
echo "[3/4] Rust cargo test (REQUIRED — v0.73a removes --quick)"
CARGO="rustup run 1.89 cargo"

if [ "$(uname -s)" = "Linux" ]; then
  echo "--- Linux system deps (webkit2gtk-4.1 stack)"
  if command -v apt-get >/dev/null 2>&1; then
    sudo apt-get update
    sudo apt-get install -y --no-install-recommends \
      libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev \
      librsvg2-dev libsoup-3.0-dev libjavascriptcoregtk-4.1-dev \
      pkg-config build-essential curl wget file libssl-dev
  else
    echo "⚠ not apt-get based Linux; skipping system deps"
  fi
else
  echo "--- (macOS: using existing brew-installed webkit/gtk; no apt-get)"
fi

# CRITICAL: remove leftover dist/ from prior pnpm build to simulate
# CI's clean checkout. On CI, dist/ doesn't exist → tauri::generate_context!
# panics with 'frontendDist path doesn't exist'. Locally we have a stale
# dist/ so this never fires — that's the gap v0.69g fixed.
rm -rf dist
mkdir -p dist/assets
cat > dist/index.html <<'EOF'
<!doctype html><html><head><meta charset="utf-8"><title>polyrocket</title></head><body><div id="root"></div><script type="module" src="/assets/main.js"></script></body></html>
EOF
echo "console.log('stub')" > dist/assets/main.js

echo "--- cargo build --lib"
(cd src-tauri && $CARGO build --lib 2>&1 | tee /tmp/cargo-build.log)
if grep -E "^(warning|error):" /tmp/cargo-build.log > /dev/null 2>&1; then
  # Only fail on errors, not warnings (warnings can be fixed in
  # follow-up commits without blocking push).
  if grep -E "^error:" /tmp/cargo-build.log > /dev/null 2>&1; then
    echo "✗ cargo build had errors — see /tmp/cargo-build.log"
    exit 3
  fi
fi

echo "--- cargo test --lib -- --test-threads=1"
(cd src-tauri && $CARGO test --lib -- --test-threads=1 2>&1 | tee /tmp/cargo-test.log)
if ! grep -E "test result: ok" /tmp/cargo-test.log > /dev/null 2>&1; then
  echo "✗ cargo test failed — see /tmp/cargo-test.log"
  exit 3
fi
echo "✓ Rust cargo test PASS"
rm -rf dist
echo

# ----- Job 4: Python sidecar tests ------------------------------------
echo "[4/4] Python sidecar tests"
python3 -m pip install --upgrade pip setuptools wheel --quiet 2>/dev/null || \
  python3 -m pip install --user --upgrade pip setuptools wheel --quiet
(cd sidecar && python3 -m pip install -e . pytest --quiet)
if [ -d "$HOME/.polyrocket/sidecar/models" ]; then
  rm -f "$HOME/.polyrocket/sidecar/models/active.json" \
        "$HOME/.polyrocket/sidecar/models/candidate.json"
fi
(cd sidecar && python3 -m pytest -q 2>&1 | tee /tmp/pytest.log)
if ! grep -E "passed|passed in" /tmp/pytest.log > /dev/null 2>&1; then
  echo "✗ pytest failed — see /tmp/pytest.log"
  exit 4
fi
echo "✓ Python sidecar tests PASS"
echo

# ----- State file (v0.73a) --------------------------------------------
# Write CI_VERIFIED state so the pre-push hook can skip re-running
# on the same commit. The hook will re-run if HEAD changes or the
# state is older than 1 hour.
CURRENT_HEAD="$(git rev-parse HEAD)"
TIMESTAMP="$(date +%s)"
RUNNER_TAG="$(hostname):$$"
STATE_FILE="$REPO_ROOT/.git/CI_VERIFIED"
cat > "$STATE_FILE" <<EOF
sha=$CURRENT_HEAD
timestamp=$TIMESTAMP
jobs=all
runner=$RUNNER_TAG
EOF

echo "============================================================"
echo "ALL 4 JOBS PASSED — safe to push"
echo "CI_VERIFIED state written to $STATE_FILE"
echo "============================================================"