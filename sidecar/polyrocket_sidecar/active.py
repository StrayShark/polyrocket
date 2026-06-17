"""Active-model loader (v0.11c).

Reads `~/.polyrocket/sidecar/models/active.json` (the file written
by `promote_model`) and returns the weights. Used by `predict` to
score markets with the most recently promoted model instead of
the hard-coded inline weights.

Caching strategy:
  - Read the file once per process (mtime-checked) and cache the
    parsed weights. Subsequent calls are O(1).
  - If the file is missing or malformed, fall back to the inline
    weights and log a warning (so `predict` is never broken by
    a missing model file).
  - The cache is invalidated if the file's mtime changes, so a
    new `promote_model` call is picked up on the next predict.
"""

from __future__ import annotations

import json
import logging
import time
from pathlib import Path
from typing import Any

from .train import ACTIVE_FILE, _sigmoid

_log = logging.getLogger("polyrocket_sidecar.model")

# Inline fallback weights (the same as predict.py before v0.11c).
_FALLBACK_WEIGHTS: dict[str, float] = {
    "w0": -0.5,
    "w1": 2.0,
    "w2": 0.4,
}

_cached_weights: dict[str, float] | None = None
_cached_mtime_ns: int | None = None
_cached_at: float = 0.0


def get_active_weights() -> dict[str, float]:
    """Return the active model's weights, or the fallback if not
    available. Caches by mtime so the cost is one stat() per
    process after the first call.
    """
    global _cached_weights, _cached_mtime_ns, _cached_at

    # Coarse 1-second cache to avoid a stat() on every predict call
    if _cached_weights is not None and (time.time() - _cached_at) < 1.0:
        return _cached_weights

    try:
        if not ACTIVE_FILE.exists():
            if _cached_weights != _FALLBACK_WEIGHTS:
                _log.info("no active model at %s; using inline fallback", ACTIVE_FILE)
            _cached_weights = dict(_FALLBACK_WEIGHTS)
            _cached_mtime_ns = None
            _cached_at = time.time()
            return _cached_weights
        st = ACTIVE_FILE.stat()
        if _cached_weights is not None and st.st_mtime_ns == _cached_mtime_ns:
            return _cached_weights
        # New mtime (or first read): re-parse
        data = json.loads(ACTIVE_FILE.read_text())
        best = data.get("best", {})
        weights = {
            "w0": float(best.get("w0", _FALLBACK_WEIGHTS["w0"])),
            "w1": float(best.get("w1", _FALLBACK_WEIGHTS["w1"])),
            "w2": float(best.get("w2", _FALLBACK_WEIGHTS["w2"])),
        }
        _cached_weights = weights
        _cached_mtime_ns = st.st_mtime_ns
        _cached_at = time.time()
        _log.info(
            "loaded active model: job_id=%s brier=%s mtime_ns=%s",
            data.get("job_id"),
            best.get("brier"),
            st.st_mtime_ns,
        )
        return weights
    except (OSError, json.JSONDecodeError, ValueError) as e:
        _log.warning("failed to load active model: %s; using fallback", e)
        _cached_weights = dict(_FALLBACK_WEIGHTS)
        _cached_mtime_ns = None
        _cached_at = time.time()
        return _cached_weights


def reset_cache() -> None:
    """Force the next call to re-read the file (used by tests)."""
    global _cached_weights, _cached_mtime_ns, _cached_at
    _cached_weights = None
    _cached_mtime_ns = None
    _cached_at = 0.0


def predict_with_active_model(price: float, market_age_hours: float) -> float:
    """Score one market using the active model's weights."""
    w = get_active_weights()
    z = w["w0"] + w["w1"] * (1.0 - price) + w["w2"] * (market_age_hours / 168.0)
    return _sigmoid(z)
