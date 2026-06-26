"""Active 模型加载器（v0.11c）。

读取 `~/.polyrocket/sidecar/models/active.json`（由 `promote_model`
写入的文件）并返回权重。供 `predict` 使用最近提升（promote）的模型
对市场打分，而不是使用硬编码的内联权重。

缓存策略：
  - 每个进程读取文件一次（通过 mtime 检查）并缓存解析后的权重。
    后续调用为 O(1)。
  - 如果文件缺失或格式错误，回退到内联权重并记录一条警告
    （这样 `predict` 永远不会被缺失的模型文件破坏）。
  - 如果文件的 mtime 发生变化，缓存会失效，以便在下一次
    `predict` 时能拿到新一次 `promote_model` 的结果。
"""

from __future__ import annotations

import json
import logging
import time
from pathlib import Path
from typing import Any

from .train import ACTIVE_FILE, _sigmoid

_log = logging.getLogger("polyrocket_sidecar.model")

# 内联回退权重（与 v0.11c 之前的 predict.py 相同）。
_FALLBACK_WEIGHTS: dict[str, float] = {
    "w0": -0.5,
    "w1": 2.0,
    "w2": 0.4,
}

_cached_weights: dict[str, float] | None = None
_cached_mtime_ns: int | None = None
_cached_at: float = 0.0


_cached_model_version: str | None = None


def get_active_weights() -> dict[str, float]:
    """返回 active 模型的权重，若不可用则返回回退权重。
    按 mtime 缓存，因此首次调用后每次进程只需要一次 stat()。
    """
    # 如果缓存过期则重新读取（同时刷新 model_version 缓存）
    get_active_model_info()
    return _cached_weights if _cached_weights is not None else dict(_FALLBACK_WEIGHTS)


def get_active_model_info() -> tuple[dict[str, float], str | None]:
    """返回 active 模型的 (weights, model_version)。

    v0.12a —— `model_version` 是 train 的 job_id（如 "train-441c352b"），
    并加上 "logistic-" 前缀。对于内联回退版本，版本号为
    "logistic-0.1.0"（原始的 predict.py 版本字符串）。

    返回 (weights, model_version)。model_version 仅在 active.json
    存在但格式错误（罕见）时为 None。
    """
    global _cached_weights, _cached_mtime_ns, _cached_at, _cached_model_version

    # 粗粒度 1 秒缓存，避免每次 predict 都调用 stat()
    if _cached_weights is not None and (time.time() - _cached_at) < 1.0:
        return _cached_weights, _cached_model_version

    try:
        if not ACTIVE_FILE.exists():
            if _cached_weights != _FALLBACK_WEIGHTS:
                _log.info("no active model at %s; using inline fallback", ACTIVE_FILE)
            _cached_weights = dict(_FALLBACK_WEIGHTS)
            _cached_mtime_ns = None
            _cached_model_version = "logistic-0.1.0"
            _cached_at = time.time()
            return _cached_weights, _cached_model_version
        st = ACTIVE_FILE.stat()
        if _cached_weights is not None and st.st_mtime_ns == _cached_mtime_ns:
            return _cached_weights, _cached_model_version
        # 新的 mtime（或首次读取）：重新解析
        data = json.loads(ACTIVE_FILE.read_text())
        best = data.get("best", {})
        weights = {
            "w0": float(best.get("w0", _FALLBACK_WEIGHTS["w0"])),
            "w1": float(best.get("w1", _FALLBACK_WEIGHTS["w1"])),
            "w2": float(best.get("w2", _FALLBACK_WEIGHTS["w2"])),
        }
        _cached_weights = weights
        _cached_mtime_ns = st.st_mtime_ns
        # v0.12a —— model version 由 train 的 job_id 推导得出
        job_id = data.get("job_id", "unknown")
        _cached_model_version = f"logistic-{job_id}"
        _cached_at = time.time()
        _log.info(
            "loaded active model: job_id=%s brier=%s mtime_ns=%s",
            data.get("job_id"),
            best.get("brier"),
            st.st_mtime_ns,
        )
        return _cached_weights, _cached_model_version
    except (OSError, json.JSONDecodeError, ValueError) as e:
        _log.warning("failed to load active model: %s; using fallback", e)
        _cached_weights = dict(_FALLBACK_WEIGHTS)
        _cached_mtime_ns = None
        _cached_model_version = "logistic-0.1.0"
        _cached_at = time.time()
        return _cached_weights, _cached_model_version


def reset_cache() -> None:
    """强制下一次调用重新读取文件（供测试使用）。"""
    global _cached_weights, _cached_mtime_ns, _cached_at, _cached_model_version
    _cached_weights = None
    _cached_mtime_ns = None
    _cached_at = 0.0
    _cached_model_version = None


def predict_with_active_model(price: float, market_age_hours: float) -> float:
    """使用 active 模型的权重对单个市场进行打分。"""
    w = get_active_weights()
    z = w["w0"] + w["w1"] * (1.0 - price) + w["w2"] * (market_age_hours / 168.0)
    return _sigmoid(z)
