#!/usr/bin/env bash
# scripts/e2e_football.sh — run the football market analysis E2E test
#
# Verifies the full pipeline:
#   1. SQLite schema (26 tables via migrations.rs)
#   2. Football market seed (SeedBundle.demo)
#   3. LLM provider setup (Doubao / MiniMax / Qwen / Moonshot / Zhipu fallback)
#   4. football.v1.0 prompt build (Dixon-Coles + Elo + xG + CLV)
#   5. LLM HTTP call → response parse → FootballRecommendationPayload
#
# Exit codes:
#   0 — success: got valid analysis (probability + side + confidence)
#   1 — failure: any step in the pipeline
#   2 — no provider had credentials
#
# Requirements:
#   - At least one of: DOUBAO_API_KEY, MINIMAX_API_KEY, QWEN_API_KEY,
#                      MOONSHOT_API_KEY, ZHIPU_API_KEY (in .env or env)
#   - Or: pre-seeded keyring entries
#
# Usage:
#   ./scripts/e2e_football.sh                 # uses .env automatically
#   ./scripts/e2e_football.sh --strict        # fail if parse_ok=false
#   ./scripts/e2e_football.sh --output FILE    # save output to FILE

set -uo pipefail

cd "$(dirname "$0")/.."
ROOT="$(pwd)"
SRC_TAURI="$ROOT/src-tauri"
ENV_FILE="$ROOT/.env"

# parse args
STRICT=0
OUTPUT_FILE=""
while [[ $# -gt 0 ]]; do
    case "$1" in
        --strict) STRICT=1; shift ;;
        --output) OUTPUT_FILE="$2"; shift 2 ;;
        -h|--help)
            sed -n '2,18p' "$0"
            exit 0
            ;;
        *) echo "unknown arg: $1"; exit 1 ;;
    esac
done

# load .env — only lines that look like KEY=value (skip notes / model names with spaces)
if [[ -f "$ENV_FILE" ]]; then
    echo "[e2e] loading .env"
    while IFS= read -r line; do
        # skip comments / blanks
        [[ -z "$line" || "$line" =~ ^[[:space:]]*# ]] && continue
        # only KEY=VALUE where KEY is uppercase_letters_underscores
        if [[ "$line" =~ ^[[:space:]]*([A-Z_][A-Z0-9_]*)=(.*)$ ]]; then
            key="${BASH_REMATCH[1]}"
            val="${BASH_REMATCH[2]}"
            # strip surrounding quotes
            val="${val%\"}"; val="${val#\"}"
            val="${val%\'}"; val="${val#\'}"
            export "$key"="$val"
        fi
    done < "$ENV_FILE"
fi

# require at least one provider credential
HAVE_ANY=0
for VAR in DOUBAO_API_KEY MINIMAX_API_KEY QWEN_API_KEY MOONSHOT_API_KEY ZHIPU_API_KEY; do
    if [[ -n "${!VAR:-}" ]]; then
        echo "[e2e] ✓ ${VAR}=${!VAR:0:8}..."
        HAVE_ANY=1
    fi
done
if [[ $HAVE_ANY -eq 0 ]]; then
    echo "[e2e] FATAL: no provider API keys in env"
    exit 2
fi

# build (cached if no source change)
echo "[e2e] cargo build --bin e2e_football"
(cd "$SRC_TAURI" && cargo build --bin e2e_football --message-format=short 2>&1 \
    | grep -E "^(error|warning: unused|   Finished|    Finished)" \
    | tail -5)

# run
echo
echo "[e2e] running e2e_football binary"
LOG="$(mktemp)"
if [[ -n "$OUTPUT_FILE" ]]; then
    (cd "$SRC_TAURI" && cargo run --quiet --bin e2e_football 2>/dev/null > "$OUTPUT_FILE")
    EXIT=$?
else
    (cd "$SRC_TAURI" && cargo run --quiet --bin e2e_football 2>/dev/null | tee "$LOG")
    EXIT=${PIPESTATUS[0]}
    OUTPUT_FILE="$LOG"
fi

echo "[e2e] exit code: $EXIT"

# extract key fields from output
PROB="$(grep -E "^probability\s*:" "$OUTPUT_FILE" | tail -1 | awk '{print $NF}')"
SIDE="$(grep -E "^side\s*:" "$OUTPUT_FILE" | tail -1 | awk '{print $NF}')"
CONF="$(grep -E "^confidence\s*:" "$OUTPUT_FILE" | tail -1 | awk '{print $NF}')"
PROVIDER="$(grep -E "Match Analysis Result \(provider=" "$OUTPUT_FILE" | sed 's/.*provider=//;s/).*//')"
HTTP_STATUS="$(grep -E "^http_status\s*:" "$OUTPUT_FILE" | tail -1 | awk '{print $NF}')"
LATENCY="$(grep -E "^latency_ms\s*:" "$OUTPUT_FILE" | tail -1 | awk '{print $NF}')"
TOKENS_IN="$(grep -E "^tokens_in\s*:" "$OUTPUT_FILE" | tail -1 | awk '{print $NF}')"
TOKENS_OUT="$(grep -E "^tokens_out\s*:" "$OUTPUT_FILE" | tail -1 | awk '{print $NF}')"

echo
echo "================================================================"
echo "E2E FOOTBALL ANALYSIS — SUMMARY"
echo "================================================================"
echo "  provider     : $PROVIDER"
echo "  http_status  : $HTTP_STATUS"
echo "  latency_ms   : $LATENCY"
echo "  tokens_in/out: $TOKENS_IN / $TOKENS_OUT"
echo "  probability  : $PROB"
echo "  side         : $SIDE"
echo "  confidence   : $CONF"
echo "================================================================"

# validate result
if [[ "$EXIT" -ne 0 ]]; then
    echo "[e2e] FAIL: binary exited with $EXIT"
    exit 1
fi
if [[ -z "$PROB" || -z "$SIDE" || -z "$CONF" ]]; then
    echo "[e2e] FAIL: missing probability/side/confidence in output"
    exit 1
fi
if [[ "$STRICT" -eq 1 ]]; then
    PARSE_OK="$(grep -E "^parse_ok\s*:" "$OUTPUT_FILE" | tail -1 | awk '{print $NF}')"
    if [[ "$PARSE_OK" != "true" ]]; then
        echo "[e2e] FAIL: --strict but parse_ok=false"
        exit 1
    fi
fi

echo "[e2e] PASS: football.v1.0 E2E produced valid analysis"
exit 0