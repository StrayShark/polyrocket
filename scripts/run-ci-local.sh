#!/usr/bin/env bash
# polyrocket — run all CI jobs locally (v0.69h).
#
# Why this exists: each v0.69 fix surfaced an issue that local
# macOS verification missed because:
#   - macOS has Rust 1.96, so 1.85/1.88/1.89 toolchain fixes
#     trivially pass.
#   - macOS already has a leftover dist/ from prior `pnpm build`,
#     so the tauri::generate_context!() macro doesn't fire its
#     'frontendDist path doesn't exist' panic.
#   - macOS uses webkit via brew (different sys libs from Linux),
#     so missing glib-2.0 on ubuntu-latest isn't reproducible.
#   - pnpm 11 (local) tolerates empty pnpm-workspace.yaml;
#     pnpm 9 (CI) doesn't.
#
# This script mirrors the .github/workflows/ci.yml jobs in order,
# simulating CI's clean state. It runs on macOS (skipping Linux
# apt-get) and on Linux (full pass). Use it as a pre-push gate:
#
#   $ ./scripts/run-ci-local.sh              # must exit 0 before push
#   $ ./scripts/run-ci-local.sh --quick      # skip cargo (saves 1m)
#
# Jobs mirrored (must match .github/workflows/ci.yml):
#   1. governance guards    (no setup-node needed; pure node scripts)
#   2. L1 typecheck + vitest (setup-node 20 + pnpm 9 + install +
#                              typecheck + vitest + coverage + readme sync)
#   3. Rust cargo test      (cargo 1.89 + dist stub + build + test --threads=1)
#   4. Python sidecar       (pip install -e . + pytest)
#
# Exit codes:
#   0 = all pass (safe to push)
#   N = job N failed (1-indexed: 1=governance, 2=L1, 3=Rust, 4=Python)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_ROOT"

# ----- Args ------------------------------------------------------------
QUICK=false
for arg in "$@"; do
  case "$arg" in
    --quick) QUICK=true ;;
    --help|-h)
      sed -n '3,40p' "$0"
      exit 0
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
# Prefer CI's pnpm 9 (matches GHA pnpm/action-setup@v4 with version: 9)
if command -v pnpm >/dev/null 2>&1; then
  PNPM_VERSION="$(pnpm --version 2>/dev/null || echo "")"
fi
PYTHON_VERSION="$(python3 --version 2>/dev/null | awk '{print $2}' || echo "")"

echo "============================================================"
echo "polyrocket local CI runner (v0.69h)"
echo "============================================================"
echo "Platform    : $(uname -s)/$(uname -m)"
echo "Node        : ${NODE_VERSION:-NOT FOUND}"
echo "pnpm        : ${PNPM_VERSION:-NOT FOUND}"
echo "Rust 1.89   : ${RUSTC_VERSION:-NOT INSTALLED}"
echo "Python 3    : ${PYTHON_VERSION:-NOT FOUND}"
echo "Quick mode  : $QUICK"
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
# README sync step is moved to job 2 on CI; see .github/workflows/ci.yml.
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
# CI step is 'grep -q no changes needed' — do the same locally
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

# ----- Job 3: Rust cargo test -----------------------------------------
if [ "$QUICK" = "true" ]; then
  echo "[3/4] Rust cargo test — SKIPPED (--quick)"
else
  echo "[3/4] Rust cargo test"
  # CI's job uses cargo 1.89 explicitly. Use rustup run to ensure
  # we test with the SAME toolchain, not whatever's the host default.
  CARGO="rustup run 1.89 cargo"

  # CI installs Linux system deps first. We can't apt-get on macOS;
  # the macOS equivalent (brew install glib gtk+3 ...) is already
  # done by the user for `pnpm tauri dev` to work. So we skip
  # system deps on macOS and just rely on whatever's installed.
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
  (cd src-tauri && $CARGO build --lib)

  echo "--- cargo test --lib -- --test-threads=1"
  (cd src-tauri && $CARGO test --lib -- --test-threads=1)
  echo "✓ Rust cargo test PASS"
  # Clean up stub dist/ so local dev doesn't accidentally use it
  rm -rf dist
  echo
fi

# ----- Job 4: Python sidecar tests ------------------------------------
echo "[4/4] Python sidecar tests"
# CI uses Python 3.11 with modern pip. macOS system python3 is 3.9
# with pip 21.x — too old for PEP 517 editable installs of a
# pyproject.toml-only package. Upgrade pip first.
python3 -m pip install --upgrade pip setuptools wheel --quiet 2>/dev/null || \
  python3 -m pip install --user --upgrade pip setuptools wheel --quiet
(cd sidecar && python3 -m pip install -e . pytest --quiet)
# CI starts with empty ~/.polyrocket/sidecar/models/ (fresh runner).
# Locally we have stale state from prior `python3 -m polyrocket_sidecar`
# runs that breaks test_e2e_auto_promote_if_better_no_active (which
# assumes no active model exists).
if [ -d "$HOME/.polyrocket/sidecar/models" ]; then
  rm -f "$HOME/.polyrocket/sidecar/models/active.json" \
        "$HOME/.polyrocket/sidecar/models/candidate.json"
fi
(cd sidecar && python3 -m pytest -q)
echo "✓ Python sidecar tests PASS"
echo

echo "============================================================"
echo "ALL 4 JOBS PASSED — safe to push"
echo "============================================================"
