"""侧车的方法分发表。

**方法名为小写**，以匹配 Rust 的 `SidecarMethod::as_str`：
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

**如果在这里添加方法，你还必须**：
  1. 在 `domain::lab::sidecar` 的 `SidecarMethod` 枚举中添加它
  2. 在 `SidecarMethod::as_str` 中添加一个 match 分支
  3. 在 `tests/test_sidecar.py::test_all_methods_registered` 中添加一个测试
  4. 更新 `docs/coding-spec.md` 中模块级别的列表（sidecar protocols 部分）

底部的 **`DISPATCH` 字典** 用于映射方法名 → 处理函数。`__main__.py` 通过
`DISPATCH.get(req.method)` 查找，若缺失则返回 `ERR_METHOD_NOT_FOUND`。
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
    """健康检查。**无参数**。返回 `{ pong: True, ts_ms: int }`。

    **调用方**：`commands::sidecar::sidecar_health_now`（IPC `sidecar_health_now`）
    每 30s 调一次（`run_sidecar_health_loop` scheduler）。
    """
    return {"pong": True, "ts_ms": int(time.time() * 1000)}


def predict(params: dict[str, Any]) -> dict[str, Any]:
    markets = params.get("markets", [])
    if not isinstance(markets, list):
        raise ValueError("'markets' must be a list")
    # v0.12a —— predict_from_markets 现在直接返回完整的响应结构
    # （predictions + model_version），不再额外包装。
    return predict_from_markets(markets)


def train_job(params: dict[str, Any]) -> dict[str, Any]:
    """运行一次小规模的超参数扫描，将最佳模型保存为候选。

    可选参数：
      - n_trials: int（默认 4，最大 4）
      - epochs: int（默认 80）
      - job_id: str（被忽略；由服务端生成）
    """
    n_trials = int(params.get("n_trials", 4))
    epochs = int(params.get("epochs", 80))
    return run_train_job(n_trials=n_trials, epochs=epochs)


def promote_model(params: dict[str, Any]) -> dict[str, Any]:
    """将当前候选模型提升（promote）到 active 槽位。

    可选参数：
      - job_id: str（如果设置，则拒绝提升来自不同 job 的候选
                 ——防止竞态条件）
      - trial_index: int（v0.21a —— 批量提升。如果设置，则提升
                 `all_trials[]` 中该特定 trial，而不是最佳 trial。取值 0..n_trials-1。）
    """
    job_id = params.get("job_id")
    trial_index = params.get("trial_index")
    # v0.21a —— 仅当 trial_index 是 int 时才传入
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
    """从 active.json 返回提升（promote）历史。

    v0.19a —— 只读审计。无参数。返回：
      { ok, entries, count, message }
    """
    return run_list_promote_history()


def rollback_model(params: dict[str, Any]) -> dict[str, Any]:
    """将 active 模型回滚到之前的某个版本。

    v0.20a —— 在 active.json 的 promotion_history 中按
    model_version 查找条目并恢复其权重。历史条目
    必须包含 `weights`（由 v0.20a+ 的 promote_model 设置）。

    必需参数：
      - model_version: str（如 "logistic-train-441c352b"）
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
    """仅当候选模型明显优于 active 模型时才提升。

    v0.23a —— 自动提升守卫。比较候选模型的
    brier 与 active 模型的 brier。如果候选模型
    至少优于 `brier_margin`，则提升它；否则
    不做任何操作并返回清晰的 "skipped"（跳过）原因。

    可选参数：
      - brier_margin: float（默认 0.005 —— 候选模型
        必须以这个幅度击败当前 active 模型）
      - trial_index: int（None 表示最佳，0..n-1 表示
        特定 trial；语义与 promote_model 相同）
    """
    brier_margin = params.get("brier_margin", 0.005)
    # v0.23a —— 预先校验参数，以便 L1 收到清晰的
    # "skipped" 原因，而不是来自 run_auto_promote_if_better
    # 的晦涩错误。
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
    # v0.23a —— trial_index 是可选的。None 表示
    # "使用最佳 trial"，0..n-1 表示
    # "使用该特定 trial"（语义与 promote_model 相同）。
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
    """提升当前候选模型中的每一个 trial。

    v0.25a —— 一次调用批量提升全部 4 个 trial。
    无参数。返回每个 trial 的结果列表。
    """
    return run_promote_all_trials()


def backtest_model(params: dict[str, Any]) -> dict[str, Any]:
    """v0.43a —— 用一个已保存的模型重放一组
    (price, market_age_hours, outcome) 样本，并
    返回 Brier + 校准 + 每个样本的预测。

    **Params**：
      - `model_version` (str, 必需)：例如 `"logistic-train-441c352b"`。
        先在 `archive.jsonl` 中查找，再在 `active.json` 中查找。
      - `samples` (list, 必需)：每个元素是包含 `price` (0..1)、
        `market_age_hours` (≥0)、`outcome` (0 或 1) 以及可选 `label`
        的 dict。

    **调用方**：L1「ModelLab → Backtest」表单提交后 → `sidecar_backtest_model`
    IPC → 本函数。

    **Sidecar 保持纯净**：除读取模型文件外，**不**做任何 IO。`samples`
    由 L1 从 `markets` 表中已 resolved 的市场转换而来。
    """
    model_version = params.get("model_version")
    if not isinstance(model_version, str):
        raise ValueError("'model_version' must be a string")
    samples = params.get("samples", [])
    if not isinstance(samples, list):
        raise ValueError("'samples' must be a list")
    return run_backtest_model(model_version=model_version, samples=samples)


def explain_model(params: dict[str, Any]) -> dict[str, Any]:
    """v0.55 —— 单个样本的逐特征贡献（**exact-decomposition**）。

    **Params**：
      - `model_version` (str, 必需)：例如 `"logistic-train-441c352b"`。
      - `sample` (dict, 可选)：`{ price, market_age_hours }`。
        缺省 → 使用默认 sample (price=0.5, age=24h) ——「模型对典型市场的看法」。

    **vs `shap_explain` (v0.59)**：exact-decomposition 公式
    `c_i = w_i * x_i * p(1-p)`，对线性模型是精确的，但**不**满足
    SHAP efficiency axiom。`shap_explain` 走 KernelExplainer，
    满足 `Σφ_i = f(x) - E[f(x)]`（每次多 100µs）。

    **Returns**：见 `explainability.run_explainability`。
    """
    model_version = params.get("model_version")
    if not isinstance(model_version, str) or not model_version:
        raise ValueError("'model_version' must be a non-empty string")
    sample = params.get("sample")
    return run_explainability(model_version=model_version, sample=sample)


def shap_explain(params: dict[str, Any]) -> dict[str, Any]:
    """v0.59 —— 通过 KernelExplainer 计算**真正的 SHAP values**。

    **Params**：
      - `model_version` (str, 必需)
      - `sample` (dict, 可选)

    **vs `explain_model` (v0.55)**：v0.55 使用 exact-decomposition，
    公式快但**不**满足 SHAP efficiency axiom。KernelSHAP 满足
    `Σφ_i = f(x) - E[f(x)]`。响应中多一个 `efficiency_diff` 字段，
    让 L1 可以显示「SHAP values 之和正好等于 prediction - baseline」
    作为 sanity check 提示。

    **Cost**：3-feature 的 polyrocket 模型 = 8 次 coalition 评估
    （约 100µs）。当 M=10 时上升到 1024 次，所以 M > 5 时就要改用
    TreeSHAP（v0.63+ 的候选方案）。
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
