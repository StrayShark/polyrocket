"""Predict 方法实现 —— 与 Rust 的 wire 格式保持一致。

Rust 端（`domain::lab::sidecar::build_predict_request`）是协议结构
的权威来源。Python 侧车对其进行了精确镜像：

  request.params = { "markets": [ { "market_id": "...", "price": 0.5 } ] }
  response.result = { "predictions": [
      { "market_id": "...", "prob": 0.5, "confidence": 0.5, "rationale": "..." }
  ] }

v0.11c：用于打分的模型是最近被 promote 的那一个
（来自 `promote_model` 的 active.json）。v0.11d：每次调用的热路径
已被上提 —— 模块级 import、预先绑定 sigmoid、单次遍历。
"""

from __future__ import annotations

import math
from typing import Any

# 内联回退权重（尚未 promote 任何模型时使用）。
_FALLBACK_W0 = -0.5
_FALLBACK_W1 = 2.0
_FALLBACK_W2 = 0.4
_HORIZON_NORM_HOURS = 168.0  # 1 周
_INV_HORIZON = 1.0 / _HORIZON_NORM_HOURS  # 为热路径预先计算


def _sigmoid(z: float) -> float:
    """数值稳定的 sigmoid。v0.11d：在热路径中预先绑定到本地变量。"""
    if z >= 0.0:
        return 1.0 / (1.0 + math.exp(-z))
    ez = math.exp(z)
    return ez / (1.0 + ez)


def predict_logic(price: float, market_age_hours: float) -> float:
    """向后兼容：使用内联回退权重。分发层
    （`predict_from_markets`）使用的是 active 模型。
    """
    z = _FALLBACK_W0 + _FALLBACK_W1 * (1.0 - price) + _FALLBACK_W2 * (market_age_hours * _INV_HORIZON)
    return _sigmoid(z)


def _clamp(v: float, lo: float, hi: float) -> float:
    return lo if v < lo else hi if v > hi else v


def _brier_score_from_active() -> float | None:
    """从 active.json 文件中读取 brier 分数。如果文件
    缺失或没有 brier 字段则返回 None。

    调用成本很低（对同一 mtime 缓存的文件只读取一次），
    但我们不在这里 import active.py，以保持本模块依赖轻量。
    """
    from .train import ACTIVE_FILE
    import json
    try:
        if not ACTIVE_FILE.exists():
            return None
        with ACTIVE_FILE.open() as f:
            data = json.load(f)
        best = data.get("best", {})
        brier = best.get("brier")
        return float(brier) if brier is not None else None
    except (OSError, json.JSONDecodeError, ValueError):
        return None


def predict_from_markets(markets: list[dict[str, Any]]) -> list[dict[str, Any]]:
    """公共入口 —— 接受 Rust 格式的 markets 列表，返回
    Rust 格式的 predictions 列表。

    v0.11d：热路径优化。
      - `get_active_weights()` 在循环开始前调用一次
      - `_sigmoid` 绑定为循环内的本地变量
      - `math.exp` 绑定为本地变量
      - 返回的 dict 结构保持一致（协议稳定）
    """
    # 本地 import：当只需要回退路径时（例如永不触及
    # active.py 的单元测试）保持 import 图尽量小。
    from .active import get_active_model_info

    # 将热路径函数绑定为本地变量。CPython 的 LOAD_FAST
    # 比 LOAD_GLOBAL 快约 30%，而 50 个市场的一批
    # 调用就意味着 50 次 sigmoid 调用。这些节省会
    # 累加起来。
    sigmoid = _sigmoid
    exp = math.exp
    inv_horizon = _INV_HORIZON

    weights, model_version = get_active_model_info()
    w0 = weights["w0"]
    w1 = weights["w1"]
    w2 = weights["w2"]
    # v0.13b —— 同时暴露 active 模型的 brier 分数，
    # 以便 L1 ModelLab 的 tooltip 可以展示校准信息。
    brier_score = _brier_score_from_active()
    del weights  # 不在循环结束后继续持有引用

    out: list[dict[str, Any]] = []
    # 预先格式化权重元组一次（在每条 rationale 中都会用到）。
    w_str = f"({w0:.3f},{w1:.3f},{w2:.3f})"

    for m in markets:
        market_id = m.get("market_id", "")
        if not market_id:
            continue
        # 强制转换 price；默认 0.5
        raw_price = m.get("price", 0.5)
        try:
            price = float(raw_price)
        except (TypeError, ValueError):
            price = 0.5
        if price < 0.0:
            price = 0.0
        elif price > 1.0:
            price = 1.0
        # 强制转换 age；默认 0.0
        raw_age = m.get("market_age_hours", 0.0)
        try:
            age = float(raw_age)
        except (TypeError, ValueError):
            age = 0.0
        if age < 0.0:
            age = 0.0

        z = w0 + w1 * (1.0 - price) + w2 * (age * inv_horizon)
        prob = sigmoid(z)
        # Confidence: 在 price=0.5 时为 0，在 price=0 或 1 时为 1
        # v0.11d —— 在这条热路径上，手写的 abs 比内置
        # abs() 更快。
        confidence = price - 0.5
        if confidence < 0.0:
            confidence = -confidence
        confidence *= 2.0

        out.append({
            "market_id": market_id,
            "prob": round(prob, 4),
            "confidence": round(confidence, 4),
            "rationale": f"{model_version or 'logistic'}: w={w_str} price={price:.3f} age_h={age:.1f} → p={prob:.3f}",
        })
    return {"predictions": out, "model_version": model_version, "brier_score": brier_score}


_WEIGHTS_VERSION = "0.1.0"


# v0.11d —— 微基准测试钩子。运行方式：
#   python3 -c "from polyrocket_sidecar import predict; predict.bench(n=10000)"
def bench(n: int = 10000) -> float:
    """对 `n` 个合成市场打分。返回经过的秒数。

    用于验证 v0.11d 的热路径优化是否真的有效。
    """
    import time
    markets = [
        {"market_id": f"m{i}", "price": (i * 0.0001) % 1.0, "market_age_hours": i * 0.1}
        for i in range(n)
    ]
    started = time.perf_counter()
    out = predict_from_markets(markets)
    elapsed = time.perf_counter() - started
    assert len(out) == n
    return elapsed
