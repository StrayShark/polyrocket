#!/usr/bin/env python3
"""Smoke test for the sidecar — round-trips a few requests via the real subprocess.

Uses the Rust wire format (lowercase methods, {"markets": [...]} params).

Usage:
    python3 scripts/smoke.py
"""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent

PY = sys.executable
CMD = [PY, "-m", "polyrocket_sidecar"]


def main() -> int:
    print(f"→ spawning: {' '.join(CMD)} (cwd={ROOT})")
    proc = subprocess.Popen(
        CMD,
        cwd=str(ROOT),
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        bufsize=1,
    )
    assert proc.stdin is not None and proc.stdout is not None

    cases = [
        {"id": 1, "method": "ping", "params": {}},
        {
            "id": 2,
            "method": "predict",
            "params": {
                "markets": [
                    {"market_id": "m1", "price": 0.2, "market_age_hours": 48},
                    {"market_id": "m2", "price": 0.8, "market_age_hours": 48},
                ]
            },
        },
        {"id": 3, "method": "train_job", "params": {"trial": 1}},
        {"id": 4, "method": "promote_model", "params": {"name": "logistic-0.2.0"}},
        {"id": 5, "method": "what_is_this", "params": {}},
    ]

    all_ok = True
    for c in cases:
        line = json.dumps(c)
        proc.stdin.write(line + "\n")
        proc.stdin.flush()
        out = proc.stdout.readline().strip()
        try:
            obj = json.loads(out)
        except json.JSONDecodeError as e:
            print(f"❌ {line!r} → !parse {e} (raw: {out!r})")
            all_ok = False
            continue
        if obj.get("error"):
            print(f"  → id={obj['id']} ERROR: {obj['error']}")
        elif obj.get("ok") is False:
            print(f"  → id={obj['id']} FAILED: {obj.get('error') or 'unknown'}")
        else:
            print(f"  → id={obj['id']} result={obj['result']}")
        import time
        time.sleep(0.05)

    proc.stdin.close()
    proc.terminate()
    try:
        proc.wait(timeout=2)
    except subprocess.TimeoutExpired:
        proc.kill()

    if all_ok:
        print("✓ smoke OK")
        return 0
    print("❌ smoke FAILED")
    return 1


if __name__ == "__main__":
    sys.exit(main())
