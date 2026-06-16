"""Method dispatch table for the sidecar.

Method names are LOWERCASE to match the Rust `SidecarMethod::as_str`:
  - "ping"            (Rust: SidecarMethod::Ping)
  - "predict"         (Rust: SidecarMethod::Predict)
  - "train_job"       (Rust: SidecarMethod::TrainJob)
  - "promote_model"   (Rust: SidecarMethod::PromoteModel)

If you add a method here, you MUST also:
  1. Add it to `SidecarMethod` enum in domain::lab::sidecar
  2. Add a match arm in `SidecarMethod::as_str`
  3. Add a test in tests/test_sidecar.py::test_all_methods_registered
"""

from __future__ import annotations

import time
from typing import Any, Callable

from .predict import predict_from_markets


def ping(_params: dict[str, Any]) -> dict[str, Any]:
    return {"pong": True, "ts_ms": int(time.time() * 1000)}


def predict(params: dict[str, Any]) -> dict[str, Any]:
    markets = params.get("markets", [])
    if not isinstance(markets, list):
        raise ValueError("'markets' must be a list")
    predictions = predict_from_markets(markets)
    return {"predictions": predictions}


def train_job(_params: dict[str, Any]) -> dict[str, Any]:
    # Stub: v0.8+ will run a real hyperparameter sweep.
    return {
        "job_id": "stub-train-job",
        "status": "stub",
        "message": "train_job is a placeholder in v0.7b; see polyrocket-sidecar README",
    }


def promote_model(_params: dict[str, Any]) -> dict[str, Any]:
    # Stub: v0.8+ will atomically swap the active model.
    return {
        "promoted": True,
        "status": "stub",
        "message": "promote_model is a placeholder in v0.7b; see polyrocket-sidecar README",
    }


DISPATCH: dict[str, Callable[[dict[str, Any]], dict[str, Any]]] = {
    "ping": ping,
    "predict": predict,
    "train_job": train_job,
    "promote_model": promote_model,
}
