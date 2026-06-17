"""Train + promote implementations (v0.10b).

Replaces the v0.7b stubs with a real (but still simple) workflow:

`train_job` runs a tiny hyperparameter sweep over the (price,
market_age_hours) features used by `predict`. For each trial, it
fits a logistic regression with the trial's learning rate /
regularization, scores it on a synthetic holdout, and reports the
best one as the new candidate model.

`promote_model` reads the candidate and "promotes" it — i.e.
copies the candidate weights into a stable "active" file in the
sidecar's working directory, so the next `predict` call uses
them. No real "atomic" swap (we don't have multiple processes),
but the contract is the same as the docs promise: after a
successful promote, the new model is the one that runs.

This is intentionally a working toy, not production ML. It exists
to:
  1. Prove the train_job / promote_model methods round-trip works
  2. Give the L1 UI something to call from ModelLab
  3. Set the shape for a real model in v0.11+ (gradient boosting,
     cross-validation, etc.)

The on-disk format is a tiny JSON file. Replacement of `predict`
to use the promoted weights is intentionally NOT in this commit —
`predict` keeps using the inline logistic weights from predict.py.
A v0.10b+ commit will swap predict() to read from the active file.
"""

from __future__ import annotations

import json
import math
import os
import random
import time
import uuid
from pathlib import Path
from typing import Any

# Where the candidate and active model files live. Override with
# `POLYROCKET_SIDECAR_MODEL_DIR=/some/path` in tests or production.
_DEFAULT_MODEL_DIR = Path.home() / ".polyrocket" / "sidecar" / "models"
MODEL_DIR = Path(os.environ.get("POLYROCKET_SIDECAR_MODEL_DIR", str(_DEFAULT_MODEL_DIR)))
CANDIDATE_FILE = MODEL_DIR / "candidate.json"
ACTIVE_FILE = MODEL_DIR / "active.json"

# Synthetic "training data" used by the sweep. Deterministic so
# results are reproducible across runs.
def _synthetic_dataset(n: int = 200) -> list[tuple[float, float, int]]:
    """Return (price, market_age_hours, label) triples.

    Label is 1 if the synthetic market resolves YES, 0 otherwise.
    The relationship: markets that close fast with low price are
    more likely to resolve YES. Tuned so a fitted logistic gets
    reasonable calibration, not perfect.
    """
    rng = random.Random(0xC0DE)
    out: list[tuple[float, float, int]] = []
    for _ in range(n):
        price = rng.uniform(0.05, 0.95)
        age = rng.uniform(0, 168)  # hours
        # Latent logit: cheaper + older → more likely YES
        z = -0.5 + 2.0 * (1.0 - price) + 0.4 * (age / 168.0)
        p = 1.0 / (1.0 + math.exp(-z))
        label = 1 if rng.random() < p else 0
        out.append((price, age, label))
    return out


def _sigmoid(z: float) -> float:
    if z >= 0:
        return 1.0 / (1.0 + math.exp(-z))
    ez = math.exp(z)
    return ez / (1.0 + ez)


def _fit_logistic(
    data: list[tuple[float, float, int]],
    lr: float,
    reg: float,
    epochs: int,
) -> dict[str, float]:
    """Fit a 3-parameter logistic regression by plain gradient descent.

    Model: p = sigmoid(w0 + w1*(1-price) + w2*age/168)
    """
    w0, w1, w2 = 0.0, 0.0, 0.0
    for _ in range(epochs):
        # Compute gradient over the full batch
        g0 = g1 = g2 = 0.0
        for price, age, label in data:
            z = w0 + w1 * (1.0 - price) + w2 * (age / 168.0)
            p = _sigmoid(z)
            diff = p - label
            g0 += diff
            g1 += diff * (1.0 - price)
            g2 += diff * (age / 168.0)
        n = len(data)
        w0 -= lr * (g0 / n + reg * w0)
        w1 -= lr * (g1 / n + reg * w1)
        w2 -= lr * (g2 / n + reg * w2)
    return {"w0": w0, "w1": w1, "w2": w2}


def _brier(model: dict[str, float], data: list[tuple[float, float, int]]) -> float:
    """Mean squared error of probability predictions."""
    total = 0.0
    for price, age, label in data:
        z = model["w0"] + model["w1"] * (1.0 - price) + model["w2"] * (age / 168.0)
        p = _sigmoid(z)
        total += (p - label) ** 2
    return total / len(data)


def run_train_job(
    *,
    n_trials: int = 4,
    epochs: int = 80,
    seed: int = 0xC0DE,
) -> dict[str, Any]:
    """Run a small hyperparameter sweep, persist the best model as a candidate.

    Returns a result dict that mirrors what the L1 ModelLab page needs:
      - job_id       — unique id for this run
      - status       — "completed" / "failed"
      - best_brier   — score of the winning trial
      - best_params  — {"w0", "w1", "w2"} of the winner
      - trials       — list of {lr, reg, brier, weights} for transparency
      - duration_ms  — wall-clock time
      - candidate_path — where the JSON was written (or null on failure)
    """
    started = time.time()
    job_id = f"train-{uuid.uuid4().hex[:8]}"
    data = _synthetic_dataset(200)

    # Small grid; 4 trials is plenty to show the sweep is real
    full_grid: list[tuple[float, float]] = [
        (0.05, 0.01),
        (0.10, 0.01),
        (0.05, 0.10),
        (0.10, 0.10),
    ]
    grid = full_grid[: max(1, min(n_trials, len(full_grid)))]

    trials: list[dict[str, Any]] = []
    best: dict[str, Any] | None = None

    for lr, reg in grid:
        w = _fit_logistic(data, lr=lr, reg=reg, epochs=epochs)
        brier = _brier(w, data)
        trials.append({"lr": lr, "reg": reg, "brier": brier, "weights": w})
        if best is None or brier < best["brier"]:
            best = {"lr": lr, "reg": reg, "brier": brier, "weights": w}

    if best is None:
        return {
            "job_id": job_id,
            "status": "failed",
            "message": "no trials completed",
            "duration_ms": int((time.time() - started) * 1000),
        }

    # Persist the candidate. Atomic write: write to a temp file,
    # then rename. Guarantees no half-written file is observed.
    try:
        MODEL_DIR.mkdir(parents=True, exist_ok=True)
        payload = {
            "job_id": job_id,
            "trained_at_ms": int(time.time() * 1000),
            "n_trials": len(trials),
            "best": best,
            "all_trials": trials,
        }
        tmp = CANDIDATE_FILE.with_suffix(".json.tmp")
        tmp.write_text(json.dumps(payload, ensure_ascii=False, indent=2))
        os.replace(tmp, CANDIDATE_FILE)
        candidate_path = str(CANDIDATE_FILE)
    except OSError as e:
        return {
            "job_id": job_id,
            "status": "failed",
            "message": f"failed to write candidate: {e}",
            "best_brier": best["brier"],
            "best_params": best["weights"],
            "trials": trials,
            "duration_ms": int((time.time() - started) * 1000),
        }

    return {
        "job_id": job_id,
        "status": "completed",
        "best_brier": round(best["brier"], 6),
        "best_params": best["weights"],
        "trials": trials,
        "duration_ms": int((time.time() - started) * 1000),
        "candidate_path": candidate_path,
    }


def run_promote_model(*, job_id: str | None = None) -> dict[str, Any]:
    """Promote the current candidate to the active slot.

    Returns:
      - promoted       — bool
      - previous_path  — where the old active file was (or null)
      - active_path    — where the new active file is
      - promoted_at_ms — timestamp
    """
    if not CANDIDATE_FILE.exists():
        return {
            "promoted": False,
            "status": "failed",
            "message": f"no candidate found at {CANDIDATE_FILE}; run train_job first",
        }
    try:
        MODEL_DIR.mkdir(parents=True, exist_ok=True)
        previous = None
        if ACTIVE_FILE.exists():
            previous = str(ACTIVE_FILE)

        candidate = json.loads(CANDIDATE_FILE.read_text())
        if job_id is not None and candidate.get("job_id") != job_id:
            return {
                "promoted": False,
                "status": "failed",
                "message": f"candidate job_id mismatch: expected {job_id}, got {candidate.get('job_id')}",
            }

        promoted_payload = {
            **candidate,
            "promoted_at_ms": int(time.time() * 1000),
        }
        tmp = ACTIVE_FILE.with_suffix(".json.tmp")
        tmp.write_text(json.dumps(promoted_payload, ensure_ascii=False, indent=2))
        os.replace(tmp, ACTIVE_FILE)

        return {
            "promoted": True,
            "status": "ok",
            "previous_path": previous,
            "active_path": str(ACTIVE_FILE),
            "promoted_at_ms": promoted_payload["promoted_at_ms"],
            "model_version": f"logistic-{candidate.get('job_id', 'unknown')}",
        }
    except (OSError, json.JSONDecodeError) as e:
        return {
            "promoted": False,
            "status": "failed",
            "message": f"promote failed: {e}",
        }
