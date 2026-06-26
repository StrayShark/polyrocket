#!/usr/bin/env bash
# e2e_sync_analyze.sh — run the sync + analyze E2E test.
#
# Usage:
#   ./scripts/e2e_sync_analyze.sh
#
# Reads API keys from .env (or env). Tries the Gamma API with proxy
# support (POLYROCKET_PROXY=127.0.0.1:7897).
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
SRC_TAURI="$PROJECT_ROOT/src-tauri"

# -- load .env if it exists (for API keys + proxy)
if [[ -f "$PROJECT_ROOT/.env" ]]; then
    set -a
    source "$PROJECT_ROOT/.env"
    set +a
fi

echo "=== e2e_sync_analyze: sync real markets + LLM analysis ==="
echo "  proxy: ${POLYROCKET_PROXY:-none}"
echo "  DOUBAO_API_KEY: ${DOUBAO_API_KEY:+set}${DOUBAO_API_KEY:-NOT SET}"
echo "  MINIMAX_API_KEY: ${MINIMAX_API_KEY:+set}${MINIMAX_API_KEY:-NOT SET}"
echo "  QWEN_API_KEY: ${QWEN_API_KEY:+set}${QWEN_API_KEY:-NOT SET}"
echo ""

cd "$SRC_TAURI"
cargo run --bin e2e_sync_analyze -- "$@"
