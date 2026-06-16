"""Predict method implementation — matches the Rust wire format.

The Rust side (`domain::lab::sidecar::build_predict_request`) is the source
of truth for the protocol shape. The Python sidecar mirrors it exactly:

  request.params = { "markets": [ { "market_id": "...", "price": 0.5 } ] }
  response.result = { "predictions": [
      { "market_id": "...", "prob": 0.5, "confidence": 0.5, "rationale": "..." }
  ] }

The model itself is a deterministic logistic regression over (price, market_age).
Replace the body of `predict_logic()` with a real model in a later release.
"""

from __future__ import annotations

import math
from typing import Any

# Hand-fitted logistic weights (rough, but stable & deterministic).
# p = sigmoid(w0 + w1*(1-price) + w2*market_age_normalized)
_W0 = -0.5
_W1 = 2.0   # higher when price is low (cheap YES = better risk/reward)
_W2 = 0.4   # market age normalized to 0..1 (hours/168)
_HORIZON_NORM_HOURS = 168.0  # 1 week


def predict_logic(price: float, market_age_hours: float) -> float:
    """Return a probability in [0, 1]."""
    z = _W0 + _W1 * (1.0 - price) + _W2 * (market_age_hours / _HORIZON_NORM_HOURS)
    return 1.0 / (1.0 + math.exp(-z))


def _clamp(v: float, lo: float, hi: float) -> float:
    return max(lo, min(hi, v))


def predict_from_markets(markets: list[dict[str, Any]]) -> list[dict[str, Any]]:
    """Public entry — accepts the Rust-shape markets list, returns the
    Rust-shape predictions list.

    `market_age_hours` is optional in the input; missing/garbage values
    default to 0 (matching the Rust `PredictParams` default).
    """
    out: list[dict[str, Any]] = []
    for m in markets:
        market_id = str(m.get("market_id", ""))
        if not market_id:
            continue
        try:
            price = float(m.get("price", 0.5))
        except (TypeError, ValueError):
            price = 0.5
        price = _clamp(price, 0.0, 1.0)
        try:
            age = float(m.get("market_age_hours", 0.0))
        except (TypeError, ValueError):
            age = 0.0
        age = max(0.0, age)

        prob = predict_logic(price, age)
        # Confidence: derived from how "extreme" the price is (extremes = more confident)
        # 0.5 → 0.0, 0 or 1 → 1.0
        confidence = abs(price - 0.5) * 2.0

        out.append({
            "market_id": market_id,
            "prob": round(prob, 4),
            "confidence": round(confidence, 4),
            "rationale": (
                f"logistic-0.1.0: price={price:.3f} age_h={age:.1f} → p={prob:.3f}"
            ),
        })
    return out


_WEIGHTS_VERSION = "0.1.0"
