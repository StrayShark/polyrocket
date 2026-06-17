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


def run_promote_model(
    *,
    job_id: str | None = None,
    trial_index: int | None = None,
) -> dict[str, Any]:
    """Promote the current candidate to the active slot.

    Args:
      job_id: optional safety check. If set, refuses to
        promote a candidate from a different job.
      trial_index: optional (v0.21a — bulk promote). If
        None (default), promotes the candidate's `best`
        trial (current behavior). If set to an integer
        in [0, n_trials), promotes that specific trial
        from `all_trials[]` instead. The promoted model
        version is suffixed with `-t{trial_index}` so the
        user can distinguish bulk-promoted trials in
        the history panel.

    Returns:
      - promoted       — bool
      - previous_path  — where the old active file was (or null)
      - active_path    — where the new active file is
      - promoted_at_ms — timestamp
      - model_version  — e.g. "logistic-train-441c352b" or
                         "logistic-train-441c352b-t2" (bulk)
      - trial_index    — int|null (which trial was promoted)

    v0.19a — also maintains a `promotion_history` array inside
    active.json. On each successful promote, the NEW entry is
    appended so the user can audit "which model was active
    when" via the `list_promote_history` method. The history
    is bounded to the last 20 entries to keep the file small.

    v0.21a — bulk promote. The user can promote any of the
    4 trials in a train sweep, not just the best. The
    "best by synthetic Brier" isn't always the "best in
    production" — the synthetic data is a stand-in. v0.21
    lets the user see all 4 in the history panel and pick
    the actual winner.
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

        # v0.21a — pick the trial to promote. trial_index
        # None → use the best (default). trial_index 0..n-1
        # → use all_trials[trial_index].
        all_trials = candidate.get("all_trials", [])
        if trial_index is not None:
            if not isinstance(trial_index, int) or trial_index < 0 or trial_index >= len(all_trials):
                return {
                    "promoted": False,
                    "status": "failed",
                    "message": (
                        f"trial_index {trial_index!r} out of range "
                        f"(0..{len(all_trials) - 1} valid)"
                    ),
                }
            trial = all_trials[trial_index]
            # v0.21a — the trial dict has `weights: {w0, w1, w2}`
            # nested, not flat. We need to extract the inner
            # dict (this differs from `best` which has w0/w1/w2
            # directly — see run_train_job's `best` shape vs
            # `trials` list shape).
            trial_weights = trial.get("weights") or {}
            weights = {
                "w0": trial_weights.get("w0"),
                "w1": trial_weights.get("w1"),
                "w2": trial_weights.get("w2"),
            }
            # v0.21a — for non-best trials, the "best" dict
            # is the candidate's best (for reference), but
            # the weights are from the specific trial.
            best = candidate.get("best") or {}
            best_brier = trial.get("brier")
            best_params = {"lr": trial.get("lr"), "reg": trial.get("reg")}
            version_suffix = f"-t{trial_index}"
        else:
            best = candidate.get("best") or {}
            weights = {
                "w0": best.get("w0"),
                "w1": best.get("w1"),
                "w2": best.get("w2"),
            }
            best_brier = best.get("brier")
            best_params = candidate.get("best_params")
            version_suffix = ""

        promoted_at_ms = int(time.time() * 1000)
        # v0.20a — extract weights from the candidate's
        # "best" dict so the history entry is self-contained
        # for rollback (no need to re-train or read the
        # candidate file later). The candidate["best"] has
        # {w0, w1, w2, brier, ...}; we only need the weights.
        new_entry: dict[str, Any] = {
            "job_id": candidate.get("job_id"),
            "model_version": f"logistic-{candidate.get('job_id', 'unknown')}{version_suffix}",
            "promoted_at_ms": promoted_at_ms,
            "best_brier": best_brier,
            "best_params": best_params,
            "weights": weights,
            # v0.21a — record the trial_index so the user
            # can see "this was the best trial" vs "this
            # was trial 2 of 4" in the history panel.
            "trial_index": trial_index,
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
            "model_version": f"logistic-{candidate.get('job_id', 'unknown')}{version_suffix}",
            "trial_index": trial_index,
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


def run_auto_promote_if_better(
    *,
    brier_margin: float = 0.005,
    trial_index: int | None = None,
) -> dict[str, Any]:
    """Promote the candidate only if it's meaningfully better than the active model.

    v0.23a — auto-promote guard. Compares the candidate's
    best Brier (or a specific trial's Brier) to the
    active model's Brier. If the candidate is at least
    `brier_margin` better, promote it; otherwise, do
    nothing and return a clear "skipped" reason.

    The "margin" prevents promoting models that are
    within noise of the current active model. Default
    0.005 — the user has to actually beat the current
    model by 0.005 Brier to auto-promote.

    Args:
      brier_margin: how much better the candidate must
        be (lower Brier = better). Default 0.005.
      trial_index: which trial to use (None = best).
        Same semantics as run_promote_model.

    Returns:
      - promoted        — bool (true if auto-promoted)
      - skipped         — bool (true if candidate wasn't
                          meaningfully better)
      - reason          — human-readable string ("not
                          better" / "no candidate" / etc.)
      - candidate_brier — the candidate's brier (or null)
      - active_brier    — the active model's brier (or null)
      - margin          — the brier_margin used
      - On success, also returns the same fields as
        PromoteResult (model_version, promoted_at_ms, etc.)
    """
    # Read the active model's brier
    active_brier: float | None = None
    if ACTIVE_FILE.exists():
        try:
            active_data = json.loads(ACTIVE_FILE.read_text())
            # The active.json's "best" dict (or top-level "best"
            # from v0.18a back-compat) has the brier
            best = active_data.get("best") or {}
            active_brier = best.get("brier")
        except (OSError, json.JSONDecodeError):
            active_brier = None

    # If there's no active model, just promote (it's
    # automatically the best by definition)
    if active_brier is None:
        result = run_promote_model(trial_index=trial_index)
        return {
            "promoted": result.get("promoted", False),
            "skipped": False,
            "reason": "no active model; auto-promoted the candidate",
            "candidate_brier": None,
            "active_brier": None,
            "margin": brier_margin,
            "model_version": result.get("model_version"),
            "promoted_at_ms": result.get("promoted_at_ms"),
            "message": result.get("message"),
        }

    # Read the candidate's brier
    if not CANDIDATE_FILE.exists():
        return {
            "promoted": False,
            "skipped": True,
            "reason": "no candidate; run train_job first",
            "candidate_brier": None,
            "active_brier": active_brier,
            "margin": brier_margin,
            "model_version": None,
            "promoted_at_ms": None,
            "message": f"no candidate at {CANDIDATE_FILE}",
        }
    try:
        candidate = json.loads(CANDIDATE_FILE.read_text())
    except (OSError, json.JSONDecodeError) as e:
        return {
            "promoted": False,
            "skipped": True,
            "reason": f"failed to read candidate: {e}",
            "candidate_brier": None,
            "active_brier": active_brier,
            "margin": brier_margin,
            "model_version": None,
            "promoted_at_ms": None,
            "message": str(e),
        }

    # Pick the candidate's brier: best (default) or a
    # specific trial
    all_trials = candidate.get("all_trials", [])
    if trial_index is not None:
        if not (0 <= trial_index < len(all_trials)):
            return {
                "promoted": False,
                "skipped": True,
                "reason": f"trial_index {trial_index} out of range",
                "candidate_brier": None,
                "active_brier": active_brier,
                "margin": brier_margin,
                "model_version": None,
                "promoted_at_ms": None,
                "message": f"trial_index {trial_index} out of range (0..{len(all_trials) - 1})",
            }
        trial = all_trials[trial_index]
        candidate_brier = trial.get("brier")
    else:
        best = candidate.get("best") or {}
        candidate_brier = best.get("brier")

    if candidate_brier is None:
        return {
            "promoted": False,
            "skipped": True,
            "reason": "candidate has no brier; cannot compare",
            "candidate_brier": None,
            "active_brier": active_brier,
            "margin": brier_margin,
            "model_version": None,
            "promoted_at_ms": None,
            "message": "candidate.best.brier is null",
        }

    # Compare: lower brier = better. Auto-promote only if
    # candidate is meaningfully better.
    improvement = active_brier - candidate_brier
    if improvement < brier_margin:
        return {
            "promoted": False,
            "skipped": True,
            "reason": (
                f"candidate brier {candidate_brier:.4f} is not "
                f"at least {brier_margin} better than active "
                f"{active_brier:.4f} (improvement: {improvement:+.4f})"
            ),
            "candidate_brier": candidate_brier,
            "active_brier": active_brier,
            "margin": brier_margin,
            "model_version": None,
            "promoted_at_ms": None,
            "message": None,
        }

    # Candidate is meaningfully better — promote it
    result = run_promote_model(trial_index=trial_index)
    return {
        "promoted": result.get("promoted", False),
        "skipped": False,
        "reason": f"auto-promoted: improvement {improvement:+.4f} > margin {brier_margin}",
        "candidate_brier": candidate_brier,
        "active_brier": active_brier,
        "margin": brier_margin,
        "model_version": result.get("model_version"),
        "promoted_at_ms": result.get("promoted_at_ms"),
        "message": result.get("message"),
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


def run_promote_all_trials() -> dict[str, Any]:
    """Promote every trial from the current candidate.

    v0.25a — bulk-promote all 4 trials in one call.
    For each trial in `candidate.all_trials[]`, calls
    `run_promote_model(trial_index=i)` and collects
    the results. Each call writes a new entry to
    `promotion_history` (capped at 20 most-recent
    entries on the write side, so all 4 fit).

    This is for A/B comparison: the user can see how
    all 4 trials perform on real markets, then roll
    back to the winner via the v0.20c Rollback button.

    Returns:
      - ok           — bool (true if all promotes succeeded)
      - results      — list of per-trial promote results,
                       each with {trial_index, promoted,
                       status, model_version, promoted_at_ms,
                       message}
      - count        — len(results)
      - message      — overall error message on failure

    On failure (e.g. no candidate), returns ok=false
    with results=[] and a clear message. Individual
    trial failures (rare; the file is read+written
    atomically) are included in the per-trial results.
    """
    if not CANDIDATE_FILE.exists():
        return {
            "ok": False,
            "results": [],
            "count": 0,
            "message": f"no candidate found at {CANDIDATE_FILE}; run train_job first",
        }
    try:
        candidate = json.loads(CANDIDATE_FILE.read_text())
    except (OSError, json.JSONDecodeError) as e:
        return {
            "ok": False,
            "results": [],
            "count": 0,
            "message": f"failed to read candidate: {e}",
        }

    all_trials = candidate.get("all_trials", [])
    if not isinstance(all_trials, list) or len(all_trials) == 0:
        return {
            "ok": False,
            "results": [],
            "count": 0,
            "message": "candidate has no all_trials; cannot bulk-promote",
        }

    # Loop over each trial and promote. The promote
    # function handles atomic file writes, so calling
    # it 4 times in a row is safe (each gets the
    # current active.json + appends to history).
    results: list[dict[str, Any]] = []
    for i in range(len(all_trials)):
        r = run_promote_model(trial_index=i)
        results.append({
            "trial_index": i,
            "promoted": r.get("promoted", False),
            "status": r.get("status", "unknown"),
            "model_version": r.get("model_version", ""),
            "promoted_at_ms": r.get("promoted_at_ms"),
            "message": r.get("message"),
        })

    # "ok" = all 4 promoted. Partial success is still
    # "ok" (the user can see the per-trial results).
    all_ok = all(r["promoted"] for r in results)
    return {
        "ok": all_ok,
        "results": results,
        "count": len(results),
        "message": None if all_ok else "one or more trial promotes failed",
    }
