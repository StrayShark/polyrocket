"""Predict method implementation — matches the Rust wire format.

The Rust side (`domain::lab::sidecar::build_predict_request`) is the source
of truth for the protocol shape. The Python sidecar mirrors it exactly:

  request.params = { "markets": [ { "market_id": "...", "price": 0.5 } ] }
  response.result = { "predictions": [
      { "market_id": "...", "prob": 0.5, "confidence": 0.5, "rationale": "..." }
  ] }

v0.11c: the model used for scoring is the most recently promoted one
(from `promote_model`'s active.json). v0.11d: the per-call hot path
is hoisted — module-level imports, pre-bound sigmoid, single pass.
"""

from __future__ import annotations

import math
from typing import Any

# Inline fallback weights (used when no model has been promoted yet).
_FALLBACK_W0 = -0.5
_FALLBACK_W1 = 2.0
_FALLBACK_W2 = 0.4
_HORIZON_NORM_HOURS = 168.0  # 1 week
_INV_HORIZON = 1.0 / _HORIZON_NORM_HOURS  # pre-computed for hot path


def _sigmoid(z: float) -> float:
    """Numerically stable sigmoid. v0.11d: pre-bound to local in hot path."""
    if z >= 0.0:
        return 1.0 / (1.0 + math.exp(-z))
    ez = math.exp(z)
    return ez / (1.0 + ez)


def predict_logic(price: float, market_age_hours: float) -> float:
    """Backwards-compat: use the inline fallback weights. The dispatch
    layer (`predict_from_markets`) uses the active model instead.
    """
    z = _FALLBACK_W0 + _FALLBACK_W1 * (1.0 - price) + _FALLBACK_W2 * (market_age_hours * _INV_HORIZON)
    return _sigmoid(z)


def _clamp(v: float, lo: float, hi: float) -> float:
    return lo if v < lo else hi if v > hi else v


def predict_from_markets(markets: list[dict[str, Any]]) -> list[dict[str, Any]]:
    """Public entry — accepts the Rust-shape markets list, returns the
    Rust-shape predictions list.

    v0.11d: hot-path optimised.
      - `get_active_weights()` is called once at the top
      - `_sigmoid` is bound to a local for the loop
      - `math.exp` is bound to a local
      - the result dict shape is identical (protocol-stable)
    """
    # Local import: keeps the import graph small when only the
    # fallback path is needed (e.g. during a unit test that never
    # touches active.py).
    from .active import get_active_weights

    # Bind the hot-path functions to locals. CPython's LOAD_FAST
    # is ~30% faster than LOAD_GLOBAL, and a 50-market batch
    # means 50 sigmoid calls. The savings add up.
    sigmoid = _sigmoid
    exp = math.exp
    inv_horizon = _INV_HORIZON

    weights = get_active_weights()
    w0 = weights["w0"]
    w1 = weights["w1"]
    w2 = weights["w2"]
    del weights  # don't hold a reference past the loop

    out: list[dict[str, Any]] = []
    # Pre-format the weight tuple once (used in every rationale).
    w_str = f"({w0:.3f},{w1:.3f},{w2:.3f})"

    for m in markets:
        market_id = m.get("market_id", "")
        if not market_id:
            continue
        # Coerce price; default 0.5
        raw_price = m.get("price", 0.5)
        try:
            price = float(raw_price)
        except (TypeError, ValueError):
            price = 0.5
        if price < 0.0:
            price = 0.0
        elif price > 1.0:
            price = 1.0
        # Coerce age; default 0.0
        raw_age = m.get("market_age_hours", 0.0)
        try:
            age = float(raw_age)
        except (TypeError, ValueError):
            age = 0.0
        if age < 0.0:
            age = 0.0

        z = w0 + w1 * (1.0 - price) + w2 * (age * inv_horizon)
        prob = sigmoid(z)
        # Confidence: 0 at price=0.5, 1 at price=0 or 1
        confidence = (0.5 - price) * -2.0 if price < 0.5 else (price - 0.5) * 2.0

        out.append({
            "market_id": market_id,
            "prob": round(prob, 4),
            "confidence": round(confidence, 4),
            "rationale": f"active: w={w_str} price={price:.3f} age_h={age:.1f} → p={prob:.3f}",
        })
    return out


_WEIGHTS_VERSION = "0.1.0"


# v0.11d — micro-benchmark hook. Run with:
#   python3 -c "from polyrocket_sidecar import predict; predict.bench(n=10000)"
def bench(n: int = 10000) -> float:
    """Score `n` synthetic markets. Returns elapsed seconds.

    Used to verify the v0.11d hot-path optimisation actually helps.
    """
    import time
    markets = [
        {"market_id": f"m{i}", "price": (i * 0.0001) % 1.0, "market_age_hours": i * 0.1}
        for i in range(n)
    ]
    started = time.perf_counter()
    out = predict_from_markets(markets)
    elapsed = time.perf_counter() - started
    assert len(out) == n
    return elapsed
