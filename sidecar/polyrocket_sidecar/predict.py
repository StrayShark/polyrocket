"""Predict method implementation — matches the Rust wire format.

The Rust side (`domain::lab::sidecar::build_predict_request`) is the source
of truth for the protocol shape. The Python sidecar mirrors it exactly:

  request.params = { "markets": [ { "market_id": "...", "price": 0.5 } ] }
  response.result = { "predictions": [
      { "market_id": "...", "prob": 0.5, "confidence": 0.5, "rationale": "..." }
  ] }

The model used for scoring is the most recently promoted one (from
`promote_model`'s active.json file). If no model has been promoted, we
fall back to the inline weights below. See `active.py` for the loader.

Replace the inline `_FALLBACK_*` constants with a real model in a later release.
"""

from __future__ import annotations

from typing import Any

# Inline fallback weights (used when no model has been promoted yet).
_FALLBACK_W0 = -0.5
_FALLBACK_W1 = 2.0   # higher when price is low (cheap YES = better risk/reward)
_FALLBACK_W2 = 0.4   # market age normalized to 0..1 (hours/168)
_HORIZON_NORM_HOURS = 168.0  # 1 week


def predict_logic(price: float, market_age_hours: float) -> float:
    """Backwards-compat: use the inline fallback weights. The dispatch
    layer (in `predict_from_markets`) uses the active model instead.
    """
    import math
    z = _FALLBACK_W0 + _FALLBACK_W1 * (1.0 - price) + _FALLBACK_W2 * (market_age_hours / _HORIZON_NORM_HOURS)
    return 1.0 / (1.0 + math.exp(-z))


def _clamp(v: float, lo: float, hi: float) -> float:
    return max(lo, min(hi, v))


def predict_from_markets(markets: list[dict[str, Any]]) -> list[dict[str, Any]]:
    """Public entry — accepts the Rust-shape markets list, returns the
    Rust-shape predictions list.

    v0.11c: uses the active model (from `active.py`) when available,
    falling back to the inline weights if no model has been promoted
    yet or the active file is malformed.

    `market_age_hours` is optional in the input; missing/garbage values
    default to 0 (matching the Rust `PredictParams` default).
    """
    # Local import to avoid a circular dependency with active.py
    from .active import get_active_weights

    out: list[dict[str, Any]] = []
    weights = get_active_weights()
    w0 = weights["w0"]
    w1 = weights["w1"]
    w2 = weights["w2"]
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

        # Inline sigmoid (don't import from train to keep this file
        # dependency-free of ML modules)
        import math
        z = w0 + w1 * (1.0 - price) + w2 * (age / _HORIZON_NORM_HOURS)
        prob = 1.0 / (1.0 + math.exp(-z))
        # Confidence: derived from how "extreme" the price is
        confidence = abs(price - 0.5) * 2.0

        out.append({
            "market_id": market_id,
            "prob": round(prob, 4),
            "confidence": round(confidence, 4),
            "rationale": (
                f"active: w=({w0:.3f},{w1:.3f},{w2:.3f}) price={price:.3f} age_h={age:.1f} → p={prob:.3f}"
            ),
        })
    return out


_WEIGHTS_VERSION = "0.1.0"
