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

    v0.19a — also maintains a `promotion_history` array inside
    active.json. On each successful promote, the NEW entry is
    appended so the user can audit "which model was active
    when" via the `list_promote_history` method. The history
    is bounded to the last 20 entries to keep the file small.
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
        history: list[dict[str, Any]] = []
        if ACTIVE_FILE.exists():
            previous = str(ACTIVE_FILE)
            # v0.19a — preserve the old payload's history when
            # we overwrite active.json. The "best" dict,
            # job_id, promoted_at_ms, and model_version are
            # the audit-relevant fields.
            try:
                old_active = json.loads(ACTIVE_FILE.read_text())
                if isinstance(old_active, dict):
                    history = list(old_active.get("promotion_history", []))
            except json.JSONDecodeError:
                pass  # corrupt old active; start fresh

        candidate = json.loads(CANDIDATE_FILE.read_text())
        if job_id is not None and candidate.get("job_id") != job_id:
            return {
                "promoted": False,
                "status": "failed",
                "message": f"candidate job_id mismatch: expected {job_id}, got {candidate.get('job_id')}",
            }

        promoted_at_ms = int(time.time() * 1000)
        # v0.20a — extract weights from the candidate's
        # "best" dict so the history entry is self-contained
        # for rollback (no need to re-train or read the
        # candidate file later). The candidate["best"] has
        # {w0, w1, w2, brier, ...}; we only need the weights.
        best = candidate.get("best") or {}
        weights = {
            "w0": best.get("w0"),
            "w1": best.get("w1"),
            "w2": best.get("w2"),
        }
        new_entry: dict[str, Any] = {
            "job_id": candidate.get("job_id"),
            "model_version": f"logistic-{candidate.get('job_id', 'unknown')}",
            "promoted_at_ms": promoted_at_ms,
            "best_brier": best.get("brier"),
            "best_params": candidate.get("best_params"),
            "weights": weights,
        }
        # v0.19a — append the NEW entry; cap at 20 most-recent
        history.append(new_entry)
        if len(history) > 20:
            history = history[-20:]

        promoted_payload = {
            **candidate,
            "promoted_at_ms": promoted_at_ms,
            "promotion_history": history,
        }
        tmp = ACTIVE_FILE.with_suffix(".json.tmp")
        tmp.write_text(json.dumps(promoted_payload, ensure_ascii=False, indent=2))
        os.replace(tmp, ACTIVE_FILE)

        return {
            "promoted": True,
            "status": "ok",
            "previous_path": previous,
            "active_path": str(ACTIVE_FILE),
            "promoted_at_ms": promoted_at_ms,
            "model_version": f"logistic-{candidate.get('job_id', 'unknown')}",
        }
    except (OSError, json.JSONDecodeError) as e:
        return {
            "promoted": False,
            "status": "failed",
            "message": f"promote failed: {e}",
        }


def run_list_promote_history() -> dict[str, Any]:
    """Return the promotion history from active.json.

    v0.19a — `list_promote_history` is the read side of the
    audit trail. Returns:
      - ok          — bool (true if active.json exists and parses)
      - entries     — list of {job_id, model_version,
                      promoted_at_ms, best_brier, best_params}
      - count       — len(entries)
      - message     — error message on failure

    The history is stored INSIDE active.json's
    `promotion_history` array (capped at 20 most-recent
    entries, written by `run_promote_model` on each promote).

    No params. Read-only operation. If active.json is
    missing or malformed, returns ok=false with an empty
    entries list (never raises — the L1 expects a clean
    response shape).
    """
    try:
        if not ACTIVE_FILE.exists():
            return {
                "ok": True,
                "entries": [],
                "count": 0,
                "message": "no active model yet; train + promote to start history",
            }
        data = json.loads(ACTIVE_FILE.read_text())
        if not isinstance(data, dict):
            return {
                "ok": False,
                "entries": [],
                "count": 0,
                "message": f"active.json at {ACTIVE_FILE} is not a JSON object",
            }
        entries = data.get("promotion_history", [])
        if not isinstance(entries, list):
            entries = []
        return {
            "ok": True,
            "entries": entries,
            "count": len(entries),
            "message": None,
        }
    except (OSError, json.JSONDecodeError) as e:
        return {
            "ok": False,
            "entries": [],
            "count": 0,
            "message": f"failed to read promotion history: {e}",
        }


def run_rollback_model(*, model_version: str) -> dict[str, Any]:
    """Roll back the active model to a previous version.

    v0.20a — looks up the entry in active.json's
    `promotion_history` by `model_version`, restores its
    weights as the new active model, and records the
    rollback as a new history entry (so the audit trail
    shows "this version was rolled back to").

    The history entry must include `weights` (v0.20a
    onwards). Older entries from v0.19 that don't have
    `weights` will be skipped with a clear error.

    Args:
      model_version: the version to roll back to, e.g.
        "logistic-train-441c352b". Must match a history
        entry's `model_version` exactly.

    Returns:
      - rolled_back   — bool
      - previous_path — the old active.json path (always
                        the standard one)
      - active_path   — the new active.json path
      - rolled_back_at_ms — timestamp
      - model_version — the version that was rolled back to
      - message       — human-readable error on failure
    """
    if not ACTIVE_FILE.exists():
        return {
            "rolled_back": False,
            "status": "failed",
            "message": f"no active model at {ACTIVE_FILE}; cannot rollback",
        }
    try:
        data = json.loads(ACTIVE_FILE.read_text())
        if not isinstance(data, dict):
            return {
                "rolled_back": False,
                "status": "failed",
                "message": f"active.json at {ACTIVE_FILE} is not a JSON object",
            }
        history = data.get("promotion_history", [])
        if not isinstance(history, list):
            return {
                "rolled_back": False,
                "status": "failed",
                "message": "promotion_history is not a list",
            }
        # v0.20a — find the entry by model_version. We match
        # the EXACT version string (the L1 should pass back
        # the model_version from the history panel verbatim).
        target = None
        for entry in history:
            if (
                isinstance(entry, dict)
                and entry.get("model_version") == model_version
            ):
                target = entry
                break
        if target is None:
            return {
                "rolled_back": False,
                "status": "failed",
                "message": f"model_version {model_version!r} not found in promotion history",
            }
        weights = target.get("weights")
        if not isinstance(weights, dict) or not all(
            k in weights for k in ("w0", "w1", "w2")
        ):
            # v0.20a — entries from v0.19 don't have weights.
            # The user needs to retrain to roll back to those.
            return {
                "rolled_back": False,
                "status": "failed",
                "message": (
                    f"model_version {model_version!r} has no weights stored "
                    "(promoted before v0.20); cannot rollback"
                ),
            }

        # v0.20a — write the rollback as a new active.json
        # with the target's weights and a new "rolled_back"
        # marker. We KEEP the same history (so the user can
        # see all their past promotes), and prepend a
        # rollback note to the message.
        rolled_back_at_ms = int(time.time() * 1000)
        new_active = {
            **data,  # preserve all other fields (e.g. best_params)
            "weights": weights,  # the new active weights
            "best": {**weights, "brier": target.get("best_brier")},
            "model_version": model_version,
            "job_id": target.get("job_id"),
            "rolled_back_at_ms": rolled_back_at_ms,
            "rolled_back_from": target.get("model_version"),
        }
        # v0.20a — also append a marker to the history so the
        # user can see "this rollback happened on date X" in
        # the audit trail. We don't bump the cap because the
        # history is already bounded.
        rollback_marker: dict[str, Any] = {
            "kind": "rollback",
            "model_version": model_version,
            "job_id": target.get("job_id"),
            "rolled_back_at_ms": rolled_back_at_ms,
            "previous_active": data.get("model_version"),
        }
        new_active["promotion_history"] = history + [rollback_marker]

        tmp = ACTIVE_FILE.with_suffix(".json.tmp")
        tmp.write_text(json.dumps(new_active, ensure_ascii=False, indent=2))
        os.replace(tmp, ACTIVE_FILE)

        return {
            "rolled_back": True,
            "status": "ok",
            "previous_path": str(ACTIVE_FILE),
            "active_path": str(ACTIVE_FILE),
            "rolled_back_at_ms": rolled_back_at_ms,
            "model_version": model_version,
        }
    except (OSError, json.JSONDecodeError) as e:
        return {
            "rolled_back": False,
            "status": "failed",
            "message": f"rollback failed: {e}",
        }
