#!/usr/bin/env bash
# polyrocket — codegen TS wrapper (v0.90 — Phase 5 build pipeline).
#
# Regenerates `src/types/generated/index.ts` by invoking
# `cargo run --bin gen_ts_types`. Works around the tauri::generate_context!
# panic that happens on a clean checkout without dist/ — the lib build
# `tauri::generate_context!()` macro requires `../dist` to exist.
#
# Pattern lifted from scripts/run-ci-local.sh:178-187.
#
# Exit codes:
#   0 — generated TS up to date
#   1 — cargo run failed (compile error / specta error)
#   2 — setup failed (mkdir/cat)
#
# Usage:
#   bash scripts/gen-ts-with-stub.sh           # regenerate
#   pnpm gen:ts                                # same, via package.json
#
# Cost: ~30-60s (cargo build --bin + specta export). Don't run in hot loops.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

# Stub dist/ so tauri::generate_context!() doesn't panic.
# Pattern: same as scripts/run-ci-local.sh.
if [ ! -d dist ]; then
  mkdir -p dist/assets
  cat > dist/index.html <<'EOF'
<!doctype html><html><head><meta charset="utf-8"><title>polyrocket</title></head><body><div id="root"></div><script type="module" src="/assets/main.js"></script></body></html>
EOF
  echo "console.log('stub')" > dist/assets/main.js
  STUB_CREATED=1
else
  STUB_CREATED=0
fi

# Always clean up the stub afterwards so the next `pnpm dev` isn't confused.
cleanup() {
  if [ "${STUB_CREATED:-0}" = "1" ]; then
    rm -rf dist
  fi
}
trap cleanup EXIT

# Run the codegen bin.
cd src-tauri
cargo run --bin gen_ts_types --quiet
