"""Method dispatch table for the sidecar.

**Method names are LOWERCASE** to match the Rust `SidecarMethod::as_str`:
  - `"ping"`                    (Rust: SidecarMethod::Ping)
  - `"predict"`                 (Rust: SidecarMethod::Predict)
  - `"train_job"`               (Rust: SidecarMethod::TrainJob)
  - `"promote_model"`           (Rust: SidecarMethod::PromoteModel)
  - `"list_promote_history"`    (Rust: SidecarMethod::ListPromoteHistory)  [v0.19a]
  - `"rollback_model"`          (Rust: SidecarMethod::RollbackModel)        [v0.20a]
  - `"auto_promote_if_better"`  (Rust: SidecarMethod::AutoPromoteIfBetter)  [v0.23a]
  - `"promote_all_trials"`      (Rust: SidecarMethod::PromoteAllTrials)      [v0.25a]
  - `"backtest_model"`          (Rust: SidecarMethod::BacktestModel)        [v0.43a]
  - `"explain_model"`           (Rust: SidecarMethod::ExplainModel)          [v0.55]
  - `"shap_explain"`            (Rust: SidecarMethod::ShapExplain)           [v0.59]

**If you add a method here, you MUST also**:
  1. Add it to `SidecarMethod` enum in `domain::lab::sidecar`
  2. Add a match arm in `SidecarMethod::as_str`
  3. Add a test in `tests/test_sidecar.py::test_all_methods_registered`
  4. Update the module-level list in `docs/coding-spec.md` (sidecar protocols section)

**`DISPATCH` dict** at the bottom maps method name → handler. `__main__.py` looks up
`DISPATCH.get(req.method)` and returns `ERR_METHOD_NOT_FOUND` if missing.
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
    run_auto_promote_if_better,
    run_promote_all_trials,
    run_backtest_model,
)
from .explainability import run_explainability
from .shap import run_shap_explainability


def ping(_params: dict[str, Any]) -> dict[str, Any]:
    """Health check. **No params**. Returns `{ pong: True, ts_ms: int }`.

    **调用方**：`commands::sidecar::sidecar_health_now`（IPC `sidecar_health_now`）
    每 30s 调一次（`run_sidecar_health_loop` scheduler）。
    """
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


def auto_promote_if_better(params: dict[str, Any]) -> dict[str, Any]:
    """Promote the candidate only if it's meaningfully better.

    v0.23a — auto-promote guard. Compares the candidate's
    brier to the active model's brier. If the candidate
    is at least `brier_margin` better, promote it;
    otherwise, do nothing and return a clear "skipped"
    reason.

    Optional params:
      - brier_margin: float (default 0.005 — the
        candidate must beat the active by this much)
      - trial_index: int (None = best, 0..n-1 for
        a specific trial; same as promote_model)
    """
    brier_margin = params.get("brier_margin", 0.005)
    if not isinstance(brier_margin, (int, float)) or brier_margin < 0:
        return {
            "promoted": False,
            "skipped": True,
            "reason": f"brier_margin must be a non-negative number, got {brier_margin!r}",
            "candidate_brier": None,
            "active_brier": None,
            "margin": brier_margin,
            "model_version": None,
            "promoted_at_ms": None,
            "message": None,
        }
    trial_index = params.get("trial_index")
    if trial_index is not None and not isinstance(trial_index, int):
        return {
            "promoted": False,
            "skipped": True,
            "reason": f"trial_index must be an int, got {type(trial_index).__name__}",
            "candidate_brier": None,
            "active_brier": None,
            "margin": brier_margin,
            "model_version": None,
            "promoted_at_ms": None,
            "message": None,
        }
    return run_auto_promote_if_better(brier_margin=float(brier_margin), trial_index=trial_index)


def promote_all_trials(_params: dict[str, Any]) -> dict[str, Any]:
    """Promote every trial from the current candidate.

    v0.25a — bulk-promote all 4 trials in one call.
    No params. Returns a list of per-trial results.
    """
    return run_promote_all_trials()


def backtest_model(params: dict[str, Any]) -> dict[str, Any]:
    """v0.43a — replay a saved model against a list of
    (price, market_age_hours, outcome) samples and
    return Brier + calibration + per-sample predictions.

    **Params**:
      - `model_version` (str, required): e.g. `"logistic-train-441c352b"`. Looked up
        in `archive.jsonl` first, then `active.json`.
      - `samples` (list, required): each item is a dict with `price` (0..1),
        `market_age_hours` (≥0), `outcome` (0 or 1), and optional `label`.

    **调用方**：L1 「ModelLab → Backtest」表单提交后 → `sidecar_backtest_model` IPC →
    这个 fn。

    **Sidecar 保持纯**：除了读 model 文件，**不**做 IO。`samples` 由 L1 从
    `markets` 表的 resolved market 转换而来。
    """
    model_version = params.get("model_version")
    if not isinstance(model_version, str):
        raise ValueError("'model_version' must be a string")
    samples = params.get("samples", [])
    if not isinstance(samples, list):
        raise ValueError("'samples' must be a list")
    return run_backtest_model(model_version=model_version, samples=samples)


def explain_model(params: dict[str, Any]) -> dict[str, Any]:
    """v0.55 — per-feature contribution for one sample（**exact-decomposition**）。

    **Params**:
      - `model_version` (str, required): e.g. `"logistic-train-441c352b"`.
      - `sample` (dict, optional): `{ price, market_age_hours }`。
        缺省 → 默认 sample (price=0.5, age=24h) — 「model 对 typical market 的看法」。

    **vs `shap_explain` (v0.59)**：exact-decomposition 公式 `c_i = w_i * x_i * p(1-p)`，
    对线性模型是精确的但**不**满足 SHAP efficiency axiom。`shap_explain` 走
    KernelExplainer，满足 `Σφ_i = f(x) - E[f(x)]`（多 100µs/次）。

    **Returns**：见 `explainability.run_explainability`。
    """
    """v0.55 — per-feature contribution for one sample.

    Params:
      - model_version (str, required): e.g.
          "logistic-train-441c352b". Looked up in
          archive.jsonl first, then active.json.
      - sample (dict, optional): { price, market_age_hours }.
          When omitted, uses a default sample
          (price=0.5, age=24h) so the user gets a
          "what would the model say for a typical
          market" view.

    Returns: see explainability.run_explainability.
    """
    model_version = params.get("model_version")
    if not isinstance(model_version, str) or not model_version:
        raise ValueError("'model_version' must be a non-empty string")
    sample = params.get("sample")
    return run_explainability(model_version=model_version, sample=sample)


def shap_explain(params: dict[str, Any]) -> dict[str, Any]:
    """v0.59 — **真 SHAP values** via KernelExplainer。

    **Params**:
      - `model_version` (str, required)
      - `sample` (dict, optional)

    **vs `explain_model` (v0.55)**：v0.55 用 exact-decomposition，公式快但
    **不**满足 SHAP efficiency axiom。KernelSHAP 满足 `Σφ_i = f(x) - E[f(x)]`。
    Response 多一个 `efficiency_diff` 字段，让 L1 可以显示「SHAP 值正好
    等于 prediction - baseline」作为 sanity check hint。

    **Cost**：3-feature polyrocket 模型 = 8 次 coalition 评估（~100µs）。
    M=10 时升到 1024 次，所以 M > 5 就要换 TreeSHAP（v0.63+ candidate）。
    """
    """v0.59 — true SHAP values via KernelExplainer.

    Params:
      - model_version (str, required): e.g.
          "logistic-train-441c352b".
      - sample (dict, optional): { price, market_age_hours }.

    Unlike v0.55's exact-decomposition
    (contribution_i = w_i * x_i * p(1-p)),
    KernelSHAP satisfies the *efficiency*
    axiom: φ_0 + Σφ_i = f(x) - E[f(x)]. We
    surface the efficiency gap as
    `efficiency_diff` in the response so the
    L1 can show "the SHAP values sum to the
    deviation from baseline" as a hint.

    For the 3-feature polyrocket model the
    cost is 8 coalition evaluations (~100µs).
    For a tree-based model with M > 5, we'd
    need a different algorithm (TreeSHAP).

    Returns: see shap.run_shap_explainability.
    """
    model_version = params.get("model_version")
    if not isinstance(model_version, str) or not model_version:
        raise ValueError("'model_version' must be a non-empty string")
    sample = params.get("sample")
    return run_shap_explainability(model_version=model_version, sample=sample)


DISPATCH: dict[str, Callable[[dict[str, Any]], dict[str, Any]]] = {
    "ping": ping,
    "predict": predict,
    "train_job": train_job,
    "promote_model": promote_model,
    "list_promote_history": list_promote_history,
    "rollback_model": rollback_model,
    "auto_promote_if_better": auto_promote_if_better,
    "promote_all_trials": promote_all_trials,
    "backtest_model": backtest_model,
    "explain_model": explain_model,
    "shap_explain": shap_explain,
}
