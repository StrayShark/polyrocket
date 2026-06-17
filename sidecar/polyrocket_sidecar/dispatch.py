"""Method dispatch table for the sidecar.

Method names are LOWERCASE to match the Rust `SidecarMethod::as_str`:
  - "ping"                    (Rust: SidecarMethod::Ping)
  - "predict"                 (Rust: SidecarMethod::Predict)
  - "train_job"               (Rust: SidecarMethod::TrainJob)
  - "promote_model"           (Rust: SidecarMethod::PromoteModel)
  - "list_promote_history"    (Rust: SidecarMethod::ListPromoteHistory)  [v0.19a]

If you add a method here, you MUST also:
  1. Add it to `SidecarMethod` enum in domain::lab::sidecar
  2. Add a match arm in `SidecarMethod::as_str`
  3. Add a test in tests/test_sidecar.py::test_all_methods_registered
"""

from __future__ import annotations

import time
from typing import Any, Callable

from .predict import predict_from_markets
from .train import run_promote_model, run_train_job, run_list_promote_history


def ping(_params: dict[str, Any]) -> dict[str, Any]:
    return {"pong": True, "ts_ms": int(time.time() * 1000)}


def predict(params: dict[str, Any]) -> dict[str, Any]:
    markets = params.get("markets", [])
    if not isinstance(markets, list):
        raise ValueError("'markets' must be a list")
    # v0.12a — predict_from_markets now returns the full response
    # shape (predictions + model_version) directly. Don't wrap.
    return predict_from_markets(markets)


def train_job(params: dict[str, Any]) -> dict[str, Any]:
    """Run a small hyperparameter sweep, persist the best model
    as a candidate. Optional params:
      - n_trials: int (default 4, max 4)
      - epochs: int (default 80)
      - job_id: str (ignored; the server generates one)
    """
    n_trials = int(params.get("n_trials", 4))
    epochs = int(params.get("epochs", 80))
    return run_train_job(n_trials=n_trials, epochs=epochs)


def promote_model(params: dict[str, Any]) -> dict[str, Any]:
    """Promote the current candidate to the active slot.

    Optional params:
      - job_id: str (if set, refuses to promote a candidate from
                 a different job — protects against race conditions)
    """
    job_id = params.get("job_id")
    return run_promote_model(job_id=job_id)


def list_promote_history(_params: dict[str, Any]) -> dict[str, Any]:
    """Return the promotion history from active.json.

    v0.19a — read-only audit. No params. Returns:
      { ok, entries, count, message }
    """
    return run_list_promote_history()


DISPATCH: dict[str, Callable[[dict[str, Any]], dict[str, Any]]] = {
    "ping": ping,
    "predict": predict,
    "train_job": train_job,
    "promote_model": promote_model,
    "list_promote_history": list_promote_history,
}
