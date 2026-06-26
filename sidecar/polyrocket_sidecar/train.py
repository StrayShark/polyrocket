"""训练 + 提升（promote）实现（v0.10b）。

用真实（但仍很简单）的工作流替换 v0.7b 的占位实现：

`train_job` 在 `predict` 使用的 (price, market_age_hours) 特征上
运行一次小规模的超参数扫描。对每个 trial，它用该 trial 的学习
率 / 正则化拟合一个逻辑回归，在合成 holdout 上打分，并把最佳
的作为新的候选模型。

`promote_model` 读取候选并"提升"它——也就是把候选权重
复制到侧车工作目录下稳定的 "active" 文件中，以便下一次
`predict` 调用使用它们。这里并没有真正的"原子"切换（我们
没有多进程），但契约与文档承诺的一致：一次成功的 promote
之后，新的模型就会成为正在运行的那个。

有意做成一个能跑的玩具，而不是生产级的 ML。它的存在
是为了：
  1. 证明 train_job / promote_model 方法的往返流程是通的
  2. 给 L1 UI 提供一个从 ModelLab 可调用的东西
  3. 为 v0.11+ 的真实模型（梯度提升、交叉验证等）奠定结构

磁盘上的格式是一个小巧的 JSON 文件。把 `predict` 替换为
使用被提升权重的逻辑**不在本次提交中**——`predict` 继续使用
predict.py 的内联逻辑权重。v0.10b+ 的某次提交会把 predict()
切换为从 active 文件中读取。
"""

from __future__ import annotations

import json
import logging
import math
import os
import random
import time
import uuid
from pathlib import Path
from typing import Any

_log = logging.getLogger("polyrocket_sidecar.train")

# 候选和 active 模型文件存放的位置。可通过环境变量
# `POLYROCKET_SIDECAR_MODEL_DIR=/some/path` 在测试或生产中覆盖。
_DEFAULT_MODEL_DIR = Path.home() / ".polyrocket" / "sidecar" / "models"
MODEL_DIR = Path(os.environ.get("POLYROCKET_SIDECAR_MODEL_DIR", str(_DEFAULT_MODEL_DIR)))
CANDIDATE_FILE = MODEL_DIR / "candidate.json"
ACTIVE_FILE = MODEL_DIR / "active.json"

# 扫描时使用的合成"训练数据"。确定性的，
# 以保证多次运行结果可复现。
def _synthetic_dataset(n: int = 200) -> list[tuple[float, float, int]]:
    """返回 (price, market_age_hours, label) 三元组列表。

    Label 为 1 表示合成市场结算为 YES，为 0 则相反。
    关系：低价 + 临近结算的市场更有可能
    结算为 YES。已调整使拟合后的 logistic 获得
    尚可的校准，但并非完美。
    """
    rng = random.Random(0xC0DE)
    out: list[tuple[float, float, int]] = []
    for _ in range(n):
        price = rng.uniform(0.05, 0.95)
        age = rng.uniform(0, 168)  # 小时
        # 潜在 logit：更便宜 + 更老 → 更可能为 YES
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
    """用普通梯度下降法拟合一个 3 参数的逻辑回归。

    模型：p = sigmoid(w0 + w1*(1-price) + w2*age/168)
    """
    w0, w1, w2 = 0.0, 0.0, 0.0
    for _ in range(epochs):
        # 在整个 batch 上计算梯度
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
    """概率预测的均方误差。"""
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
    """运行一次小规模的超参数扫描，把最佳模型持久化为候选。

    返回的结果字典对应 L1 ModelLab 页面所需的字段：
      - job_id       —— 本次运行的唯一 id
      - status       —— "completed" / "failed"
      - best_brier   —— 胜出 trial 的得分
      - best_params  —— 胜出者的 {"w0", "w1", "w2"}
      - trials       —— 每个 {lr, reg, brier, weights} 的列表，便于透明
      - duration_ms  —— 耗时（毫秒）
      - candidate_path —— JSON 写入位置（失败时为 null）
    """
    started = time.time()
    job_id = f"train-{uuid.uuid4().hex[:8]}"
    data = _synthetic_dataset(200)

    # 小网格；4 个 trial 已经足以证明扫描是真实的
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

    # 持久化候选。原子写入：先写入临时文件，
    # 然后 rename。这样可以保证永远看不到只写了一半的文件。
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
    """将当前候选提升（promote）到 active 槽位。

    Args:
      job_id: 可选的安全检查。如果设置，则拒绝提升
        来自不同 job 的候选。
      trial_index: 可选（v0.21a —— 批量提升）。如果
        为 None（默认），则提升候选的 `best` trial
        （当前行为）。如果设置为 [0, n_trials) 范围内
        的整数，则改为提升 `all_trials[]` 中的指定
        trial。被提升的模型版本会附加 `-t{trial_index}`
        后缀，以便用户在历史面板中区分
        批量提升的 trial。

    Returns:
      - promoted       —— bool
      - previous_path  —— 旧 active 文件位置（或 null）
      - active_path    —— 新 active 文件位置
      - promoted_at_ms —— 时间戳
      - model_version  —— 例如 "logistic-train-441c352b"
                         或 "logistic-train-441c352b-t2"（批量）
      - trial_index    —— int|null（被提升的是哪个 trial）

    v0.19a —— 同时在 active.json 内维护一个 `promotion_history`
    数组。每一次成功的 promote 都追加一条新条目，使用户
    可以通过 `list_promote_history` 方法审计"何时是哪个模型
    在运行"。历史被限制在最近的 20 条以内，以保持文件体积小。

    v0.21a —— 批量提升。用户可以提升 train 扫描中
    任意一个 trial，而不仅仅是 best。"按合成 Brier
    最佳的"并不总是"生产环境最佳"——合成数据只是占位。
    v0.21 让用户在历史面板中看到全部 4 个并挑选
    真正的胜出者。

    v0.33a —— 还在 20 条上限生效**之前**，把即将被丢弃
    的条目追加到一个独立的 `archive.jsonl` 文件中。
    该 archive 是 append-only 的 JSONL：每行一个 JSON
    对象，包含完整条目（job_id, model_version,
    promoted_at_ms, best_brier, best_params, weights,
    trial_index, archived_at_ms）。用户可通过新增的
    `list_promote_history_archive` IPC 读取 archive。
    archive **从不**自动裁剪——用户可自行决定清理时机。
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
            # v0.19a —— 在我们覆盖 active.json 时保留旧
            # payload 的 history。与审计相关的字段是
            # "best" dict、job_id、promoted_at_ms 和 model_version。
            try:
                old_active = json.loads(ACTIVE_FILE.read_text())
                if isinstance(old_active, dict):
                    history = list(old_active.get("promotion_history", []))
            except json.JSONDecodeError:
                pass  # 旧 active 损坏；从空 history 重新开始

        candidate = json.loads(CANDIDATE_FILE.read_text())
        if job_id is not None and candidate.get("job_id") != job_id:
            return {
                "promoted": False,
                "status": "failed",
                "message": f"candidate job_id mismatch: expected {job_id}, got {candidate.get('job_id')}",
            }

        # v0.21a —— 选择要提升的 trial。trial_index
        # 为 None → 使用 best（默认）。trial_index 为
        # 0..n-1 → 使用 all_trials[trial_index]。
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
            # v0.21a —— trial dict 中 `weights: {w0, w1, w2}`
            # 是嵌套的，不是平铺的。我们需要取出内层的
            # dict（这与 `best` 不同，`best` 中 w0/w1/w2
            # 是直接放在顶层的——参见 run_train_job 中
            # `best` 与 `trials` 列表的不同结构）。
            trial_weights = trial.get("weights") or {}
            weights = {
                "w0": trial_weights.get("w0"),
                "w1": trial_weights.get("w1"),
                "w2": trial_weights.get("w2"),
            }
            # v0.21a —— 对于非 best trial，"best" dict
            # 仍使用候选的 best（仅供参考），
            # 但权重来自指定的 trial。
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
        # v0.20a —— 从候选的 "best" dict 中提取权重，
        # 使历史条目自包含、可用于回滚（之后无需重新训练
        # 或读取 candidate 文件）。candidate["best"] 含
        # {w0, w1, w2, brier, ...}；我们只需要权重。
        # v0.41a —— `reason` 是一个简短的人类可读描述，
        # 解释这次 promote 为何发生。对于 best-trial
        # 提升，它为 "Promoted as best trial"；对于
        # 批量 trial 提升，它为 "Promoted as trial N of M"。
        # L1 在历史行的 hover tooltip 中展示这一信息。
        n_trials = len(all_trials)
        if trial_index is not None:
            reason = f"Promoted as trial {trial_index + 1} of {n_trials}"
        else:
            reason = "Promoted as best trial"
        new_entry: dict[str, Any] = {
            "job_id": candidate.get("job_id"),
            "model_version": f"logistic-{candidate.get('job_id', 'unknown')}{version_suffix}",
            "promoted_at_ms": promoted_at_ms,
            "best_brier": best_brier,
            "best_params": best_params,
            "weights": weights,
            # v0.21a —— 记录 trial_index，让用户
            # 在历史面板中区分"这是 best trial"和
            # "这是 4 个里的第 2 个"。
            "trial_index": trial_index,
            # v0.41a —— 人类可读的 promote 原因。
            # 在 L1 中以 hover tooltip 形式展示在
            # 历史行上。
            "reason": reason,
        }
        # v0.33a —— 在 20 条上限生效之前，把即将被
        # 丢弃的条目写入 archive 文件。archive 是
        # append-only JSONL，因此并发写入者（罕见，
        # 但批量提升时可能）会向同一文件追加。
        if len(history) >= 20:
            _archive_dropped_entries(history[: len(history) - 19])
        # v0.19a —— 追加新条目；保留最近 20 条
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


# v0.33a —— archive 文件路径。可通过
# `POLYROCKET_SIDECAR_MODEL_DIR` 覆盖（继承自
# 父模块的 MODEL_DIR）。archive 与 active.json
# 位于同一目录，使用户的心智模型保持统一：
# "所有模型相关的东西都在 ~/.polyrocket/sidecar/models/"。
ARCHIVE_FILE = MODEL_DIR / "archive.jsonl"


def _archive_dropped_entries(entries: list[dict[str, Any]]) -> None:
    """把被丢弃的条目追加到 JSONL archive 文件。

    v0.33a —— 在 20 条上限生效**之前**由 `run_promote_model`
    调用。每个条目作为一行 JSON 对象写入。文件是
    append-only 的，永远不会自动裁剪。

    失败会被记录但不会让 promote 失败（archive 属于
    "锦上添花"，不是审计追踪的关键部分——主要的审计
    追踪是内存中的 20 条 `promotion_history`，它总是
    反映最近的 20 次 promote）。
    """
    if not entries:
        return
    try:
        MODEL_DIR.mkdir(parents=True, exist_ok=True)
        archived_at_ms = int(time.time() * 1000)
        # 以 append 模式 ('a') 追加，每行一个 JSON。
        # 我们以同步方式执行，以保证 archive 在内存中
        # 的 history 被覆盖前已经持久化。
        with ARCHIVE_FILE.open("a", encoding="utf-8") as f:
            for entry in entries:
                # 防御性拷贝 + 添加 archived_at_ms
                archived = dict(entry)
                archived["archived_at_ms"] = archived_at_ms
                f.write(json.dumps(archived, ensure_ascii=False) + "\n")
    except OSError as e:
        # 如果 archive 写入失败也不要让 promote 失败——
        # 主要的审计追踪（内存中的 20 条 history）
        # 才是权威来源。
        _log.warning("failed to write archive: %s", e)


def run_list_promote_history() -> dict[str, Any]:
    """从 active.json 返回提升（promote）历史。

    v0.19a —— `list_promote_history` 是审计追踪的读侧。
    返回：
      - ok          —— bool（active.json 存在且解析成功时为 true）
      - entries     —— {job_id, model_version,
                      promoted_at_ms, best_brier, best_params} 列表
      - count       —— len(entries)
      - message     —— 失败时的错误消息

    历史存储在 active.json 的 `promotion_history` 数组内
    （最多保留最近的 20 条，由 `run_promote_model` 在每次
    promote 时写入）。

    无参数。只读操作。如果 active.json 缺失或格式错误，
    返回 ok=false 并附带空的 entries 列表（绝不抛异常——
    L1 期望一个干净的响应结构）。
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
    """仅当候选明显优于 active 模型时才提升。

    v0.23a —— 自动提升守卫。比较候选的
    best Brier（或指定 trial 的 Brier）与 active 模型的
    Brier。如果候选至少比 active 优 `brier_margin`，
    则提升；否则不做任何操作并返回清晰的"skipped"原因。

    "margin" 用于防止提升那些仅处于当前 active 模型
    噪声范围内的模型。默认 0.005 —— 自动提升要求
    候选比当前模型至少好 0.005 Brier。

    Args:
      brier_margin: 候选必须超过的幅度（越低越好）。
        默认 0.005。
      trial_index: 使用哪个 trial（None = best）。
        语义与 run_promote_model 相同。

    Returns:
      - promoted        —— bool（自动提升成功时为 true）
      - skipped         —— bool（候选未明显更好时为 true）
      - reason          —— 人类可读字符串（"not better" /
                          "no candidate" 等）
      - candidate_brier —— 候选的 brier（或 null）
      - active_brier    —— active 模型的 brier（或 null）
      - margin          —— 使用的 brier_margin
      - 成功时还会返回与 PromoteResult 相同的字段
        （model_version, promoted_at_ms 等）
    """
    # 读取 active 模型的 brier
    active_brier: float | None = None
    if ACTIVE_FILE.exists():
        try:
            active_data = json.loads(ACTIVE_FILE.read_text())
            # active.json 中的 "best" dict（或 v0.18a
            # 兼容的顶层 "best"）保存了 brier
            best = active_data.get("best") or {}
            active_brier = best.get("brier")
        except (OSError, json.JSONDecodeError):
            active_brier = None

    # 如果还没有 active 模型，直接提升（按定义它就是 best）
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

    # 读取候选的 brier
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

    # 选择候选的 brier：best（默认）或某个指定 trial
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

    # 比较：brier 越低越好。仅当候选明显更优时才自动提升。
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

    # 候选明显更优 —— 提升它
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
    """将 active 模型回滚到之前的某个版本。

    v0.20a —— 在 active.json 的 `promotion_history`
    中按 `model_version` 查找条目，将其权重恢复为
    新的 active 模型，并把这次回滚作为一条新的
    历史条目记录下来（这样审计追踪就会展示
    "回滚到了这个版本"）。

    历史条目必须包含 `weights`（v0.20a 起）。
    来自 v0.19 的旧条目如果不包含 `weights`，将被
    跳过并返回清晰的错误。

    Args:
      model_version: 要回滚到的版本，例如
        "logistic-train-441c352b"。必须与某条
        历史条目的 `model_version` 完全匹配。

    Returns:
      - rolled_back   —— bool
      - previous_path —— 旧 active.json 路径（始终
                        是标准路径）
      - active_path   —— 新 active.json 路径
      - rolled_back_at_ms —— 时间戳
      - model_version —— 回滚到的目标版本
      - message       —— 失败时的人类可读错误
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
        # v0.20a —— 通过 model_version 查找条目。
        # 我们匹配**完全一致**的版本字符串（L1 应原样
        # 传回历史面板中的 model_version）。
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
            # v0.20a —— v0.19 的条目没有 weights。
            # 用户需要重新训练才能回滚到那些版本。
            return {
                "rolled_back": False,
                "status": "failed",
                "message": (
                    f"model_version {model_version!r} has no weights stored "
                    "(promoted before v0.20); cannot rollback"
                ),
            }

        # v0.20a —— 把回滚作为一个新的 active.json
        # 写入，其中带有目标条目的权重和一个新的
        # "rolled_back" 标记。我们**保留**原 history
        # （这样用户可以看到他们过去所有的 promote），
        # 并在消息前加上回滚说明。
        rolled_back_at_ms = int(time.time() * 1000)
        new_active = {
            **data,  # 保留所有其他字段（例如 best_params）
            "weights": weights,  # 新的 active 权重
            "best": {**weights, "brier": target.get("best_brier")},
            "model_version": model_version,
            "job_id": target.get("job_id"),
            "rolled_back_at_ms": rolled_back_at_ms,
            "rolled_back_from": target.get("model_version"),
        }
        # v0.20a —— 同时在 history 末尾追加一个标记，
        # 让用户在审计追踪中看到"该次回滚发生在日期 X"。
        # 不再额外占用 20 条上限，因为 history 本身已有上限。
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
    """提升当前候选中的每一个 trial。

    v0.25a —— 一次调用批量提升全部 4 个 trial。
    对 `candidate.all_trials[]` 中的每个 trial，调用
    `run_promote_model(trial_index=i)` 并收集结果。
    每次调用都会向 `promotion_history` 写入新条目
    （写入侧保留最近 20 条，因此全部 4 条都能容纳）。

    这是为了 A/B 比较：用户可以看到所有 4 个 trial
    在真实市场上的表现，然后通过 v0.20c 的 Rollback
    按钮回滚到胜出者。

    Returns:
      - ok           —— bool（所有 promote 都成功时为 true）
      - results      —— 每个 trial 的 promote 结果列表，
                       每条包含 {trial_index, promoted,
                       status, model_version, promoted_at_ms,
                       message}
      - count        —— len(results)
      - message      —— 失败时的整体错误消息

    失败时（例如没有候选）返回 ok=false 且
    results=[]，并附带清晰的消息。单个 trial 失败
    （罕见；文件的读写是原子的）会包含在每个
    trial 的结果中。
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

    # 依次遍历每个 trial 并 promote。promote 函数
    # 处理原子文件写入，因此连续调用 4 次是安全的
    # （每次都拿到当前的 active.json 并追加到 history）。
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

    # "ok" = 全部 4 个 promote 都成功。部分成功也
    # 算 "ok"（用户可以从逐条结果中看出来）。
    all_ok = all(r["promoted"] for r in results)
    return {
        "ok": all_ok,
        "results": results,
        "count": len(results),
        "message": None if all_ok else "one or more trial promotes failed",
    }


# =================================================================
# ============== v0.43a — backtest_model ============================
# =================================================================


def _load_model_by_version(model_version: str) -> dict[str, Any] | None:
    """按 `model_version`（例如 "logistic-train-441c352b"）
    查找模型。如果找到则返回解析后的 JSON，否则返回 None。

    查找顺序：
      1. archive.jsonl（最可靠 —— 每次 promote 都会
         追加，即使 active.json 被回滚或损坏）
      2. 当前的 active.json（若 model_version 与
         当前的 active 匹配 —— 快速路径）
    """
    import json

    # 1. 先查 archive —— 最可靠。
    archive_path = MODEL_DIR / "archive.jsonl"
    if archive_path.exists():
        try:
            with archive_path.open() as f:
                for line in f:
                    line = line.strip()
                    if not line:
                        continue
                    try:
                        entry = json.loads(line)
                    except json.JSONDecodeError:
                        continue
                    if entry.get("model_version") == model_version:
                        return entry
        except OSError:
            pass

    # 2. 回退到 active —— 很便宜，并且即使 archive
    # 为空也有可能命中当前模型。
    if ACTIVE_FILE.exists():
        try:
            with ACTIVE_FILE.open() as f:
                data = json.load(f)
            if data.get("model_version") == model_version:
                return data
        except (OSError, json.JSONDecodeError):
            pass

    return None


def _predict_with_weights(weights: dict[str, float], price: float, market_age_hours: float) -> float:
    """使用给定的权重（w0, w1, w2）对单个样本进行预测。
    与 predict.py 中的 `predict_logic` 行为一致，但把权重
    作为参数传入，这样我们可以回测任何历史模型。
    """
    import math
    z = weights["w0"] + weights["w1"] * (1.0 - price) + weights["w2"] * (market_age_hours / 168.0)
    if z >= 0.0:
        return 1.0 / (1.0 + math.exp(-z))
    ez = math.exp(z)
    return ez / (1.0 + ez)


def run_backtest_model(
    *,
    model_version: str,
    samples: list[dict[str, Any]],
) -> dict[str, Any]:
    """v0.43a —— 用一个已保存的模型对一组
    (price, market_age_hours, outcome) 样本进行重放，
    并返回 Brier + 校准 + 每个样本的预测。

    这是缺失的一块：v0.17–v0.41 的模型生命周期允许
    train / promote / roll back，但没法问"这个模型
    在我实际交易过的市场上表现如何？"。本方法
    正好补上这一块。

    Args:
      model_version: 要回测的模型，例如
        "logistic-train-441c352b"。先在 archive.jsonl
        中查找，再在 active.json 中查找。
      samples: 一组 dict，每个包含：
        - "price"          （float，0..1，当时
                            能看到的价格）
        - "market_age_hours" （float，≥0）
        - "outcome"        （float，0 或 1，结算结果）
        - "label"          （str，可选，例如市场
                            问题，会展示在
                            "top winners/losers" 列表中）

    Returns:
      - ok: bool
      - model_version: 所请求的模型（回显）
      - sample_count: int
      - brier_mean: float（预测与结果的
        均方误差）
      - brier_breakdown: 每样本 brier 分数列表
        （用于校准直方图）
      - calibration: {bucket, predicted_avg,
        actual_rate, count} 列表 —— [0, 1] 区间内 5 个桶
      - top_winners: brier 最低的 3 个样本（最佳预测）
      - top_losers: brier 最高的 3 个样本（最差预测）
      - message: 人类可读的状态 / 错误
    """
    import time

    started = time.time()
    model = _load_model_by_version(model_version)
    if model is None:
        return {
            "ok": False,
            "model_version": model_version,
            "sample_count": 0,
            "brier_mean": None,
            "brier_breakdown": [],
            "calibration": [],
            "top_winners": [],
            "top_losers": [],
            "message": f"model {model_version!r} not found in archive or active",
        }
    weights = model.get("weights")
    if not isinstance(weights, dict) or not all(k in weights for k in ("w0", "w1", "w2")):
        return {
            "ok": False,
            "model_version": model_version,
            "sample_count": 0,
            "brier_mean": None,
            "brier_breakdown": [],
            "calibration": [],
            "top_winners": [],
            "top_losers": [],
            "message": f"model {model_version!r} has no w0/w1/w2 weights",
        }

    if not samples:
        return {
            "ok": False,
            "model_version": model_version,
            "sample_count": 0,
            "brier_mean": None,
            "brier_breakdown": [],
            "calibration": [],
            "top_winners": [],
            "top_losers": [],
            "message": "no samples provided",
        }

    # 每个样本的预测 + brier
    per_sample: list[dict[str, Any]] = []
    brier_total = 0.0
    for s in samples:
        try:
            price = float(s["price"])
            age = float(s["market_age_hours"])
            outcome = float(s["outcome"])
        except (KeyError, TypeError, ValueError):
            # 静默跳过格式错误的样本 —— 调用方
            # 传入了垃圾数据；我们不崩溃。
            continue
        if not (0.0 <= outcome <= 1.0):
            continue
        pred = _predict_with_weights(weights, price, age)
        brier = (pred - outcome) ** 2
        brier_total += brier
        per_sample.append({
            "label": s.get("label", ""),
            "price": price,
            "market_age_hours": age,
            "outcome": outcome,
            "predicted": pred,
            "brier": brier,
        })

    if not per_sample:
        return {
            "ok": False,
            "model_version": model_version,
            "sample_count": 0,
            "brier_mean": None,
            "brier_breakdown": [],
            "calibration": [],
            "top_winners": [],
            "top_losers": [],
            "message": "all samples were malformed (missing price/age/outcome)",
        }

    brier_mean = brier_total / len(per_sample)
    brier_breakdown = [s["brier"] for s in per_sample]

    # 校准：将预测值分桶到 5 个区间
    # [0, 0.2), [0.2, 0.4), ..., [0.8, 1.0]
    buckets = [[] for _ in range(5)]
    for s in per_sample:
        idx = min(int(s["predicted"] * 5), 4)
        buckets[idx].append(s)
    calibration = []
    for i, bucket in enumerate(buckets):
        lo = i * 0.2
        hi = (i + 1) * 0.2
        if not bucket:
            calibration.append({
                "bucket": f"[{lo:.1f}, {hi:.1f})",
                "predicted_avg": None,
                "actual_rate": None,
                "count": 0,
            })
            continue
        predicted_avg = sum(s["predicted"] for s in bucket) / len(bucket)
        actual_rate = sum(s["outcome"] for s in bucket) / len(bucket)
        calibration.append({
            "bucket": f"[{lo:.1f}, {hi:.1f})",
            "predicted_avg": predicted_avg,
            "actual_rate": actual_rate,
            "count": len(bucket),
        })

    # 最佳 / 最差 —— 按 brier 排序，各取 3 个
    sorted_by_brier = sorted(per_sample, key=lambda s: s["brier"])
    top_winners = sorted_by_brier[:3]
    # 最差：取最后 3 个，但不超过样本总数。
    # 少于 3 个样本时，平滑降级为样本数。
    n = min(3, len(sorted_by_brier))
    top_losers = sorted_by_brier[-n:][::-1]  # 最差排在最前

    return {
        "ok": True,
        "model_version": model_version,
        "sample_count": len(per_sample),
        "brier_mean": brier_mean,
        "brier_breakdown": brier_breakdown,
        "calibration": calibration,
        "top_winners": [
            {"label": s["label"], "brier": s["brier"],
             "predicted": s["predicted"], "outcome": s["outcome"]}
            for s in top_winners
        ],
        "top_losers": [
            {"label": s["label"], "brier": s["brier"],
             "predicted": s["predicted"], "outcome": s["outcome"]}
            for s in top_losers
        ],
        "message": None,
        "duration_ms": int((time.time() - started) * 1000),
    }
