"""Method dispatch table for the sidecar.

Method names are LOWERCASE to match the Rust `SidecarMethod::as_str`:
  - "ping"                    (Rust: SidecarMethod::Ping)
  - "predict"                 (Rust: SidecarMethod::Predict)
  - "train_job"               (Rust: SidecarMethod::TrainJob)
  - "promote_model"           (Rust: SidecarMethod::PromoteModel)
  - "list_promote_history"    (Rust: SidecarMethod::ListPromoteHistory)  [v0.19a]
  - "rollback_model"          (Rust: SidecarMethod::RollbackModel)        [v0.20a]

If you add a method here, you MUST also:
  1. Add it to `SidecarMethod` enum in domain::lab::sidecar
  2. Add a match arm in `SidecarMethod::as_str`
  3. Add a test in tests/test_sidecar.py::test_all_methods_registered
"""

from __future__ import annotations

import time
from typing import Any, Callable

from .predict import predict_from_markets
from .train import (
    run_promote_model,
    run_train_job,
    run_list_promote_history,
    run_rollback_model,
)


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
      - trial_index: int (v0.21a — bulk promote. If set, promotes
                 that specific trial from all_trials[] instead of
                 the best. 0..n_trials-1.)
    """
    job_id = params.get("job_id")
    trial_index = params.get("trial_index")
    # v0.21a — only pass trial_index if it's an int
    if trial_index is not None:
        if not isinstance(trial_index, int):
            return {
                "promoted": False,
                "status": "failed",
                "message": f"trial_index must be an int, got {type(trial_index).__name__}",
            }
        return run_promote_model(job_id=job_id, trial_index=trial_index)
    return run_promote_model(job_id=job_id)


def list_promote_history(_params: dict[str, Any]) -> dict[str, Any]:
    """Return the promotion history from active.json.

    v0.19a — read-only audit. No params. Returns:
      { ok, entries, count, message }
    """
    return run_list_promote_history()


def rollback_model(params: dict[str, Any]) -> dict[str, Any]:
    """Roll back the active model to a previous version.

    v0.20a — looks up the entry in active.json's
    promotion_history by model_version and restores
    its weights. The history entry must include
    `weights` (set by promote_model in v0.20a+).

    Required params:
      - model_version: str (e.g. "logistic-train-441c352b")
    """
    model_version = params.get("model_version")
    if not isinstance(model_version, str) or not model_version:
        return {
            "rolled_back": False,
            "status": "failed",
            "message": "missing required param: model_version (string)",
        }
    return run_rollback_model(model_version=model_version)


DISPATCH: dict[str, Callable[[dict[str, Any]], dict[str, Any]]] = {
    "ping": ping,
    "predict": predict,
    "train_job": train_job,
    "promote_model": promote_model,
    "list_promote_history": list_promote_history,
    "rollback_model": rollback_model,
}
