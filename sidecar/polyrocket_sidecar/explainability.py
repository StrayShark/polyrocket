"""v0.55 —— 模型可解释性。

计算驱动逻辑回归对给定输入做出预测的主要特征。
我们使用一种类似 SHAP 的线性模型分解：对于权重为
[w0, w1, w2]、特征为 [1, price, market_age_hours]
的逻辑回归，特征 i 对单个预测的贡献为：

    contribution_i = w_i * x_i

（通过 logistic 函数缩放到概率，然后归一化使贡献
之和等于 (p - 0.5)）。对于线性模型而言这是精确的——
不是 SHAP 的近似，而是点积的实际分解。

这是所有可解释性方法中**最便宜**但仍然有意义的方案。
它适用于 polyrocket 自带的 3 特征逻辑模型。对于
基于树的模型，则需要真正的 SHAP 库（TreeSHAP），
属于 v0.55+ 的候选方案。

输出是按 abs_contribution 降序排列的
{ feature, value, contribution, abs_contribution }
列表。L1 将其渲染为水平条形图（正值 vs 负值贡献）。
"""

from __future__ import annotations

from typing import Any

# 特征名称必须与模型训练时的顺序一致。3 特征模型使用：
#   x[0] = 1           （偏置）
#   x[1] = price       （0..1，市场价格）
#   x[2] = market_age_hours （>=0，自市场开启以来的小时数）
FEATURE_NAMES = ("bias", "price", "market_age_hours")


def run_explainability(
    *,
    model_version: str,
    sample: dict[str, Any] | None = None,
) -> dict[str, Any]:
    """计算单个样本的逐特征贡献
    （如果用户未传入，则使用默认的"平均"样本）。

    Args:
      model_version: 例如
        "logistic-train-441c352b"。先在 archive.jsonl
        中查找，再在 active.json 中查找
        （与 run_backtest_model 的逻辑相同）。
      sample: 可选 dict，包含：
        - "price"          （float，0..1，默认 0.5）
        - "market_age_hours" （float，>=0，默认 24）
        如果省略，则使用默认 sample
        （price=0.5, age=24h），以便用户得到
        一个"模型对典型市场的看法"的视图。

    Returns:
      - ok: bool
      - model_version: 所请求的模型（回显）
      - features: 按 abs_contribution 降序排列的
        { feature, value, weight, contribution,
          abs_contribution } 列表
      - prediction: float（模型对该样本的预测
        概率，0..1）
      - sample: { price, market_age_hours }
        （我们使用的输入）
      - message: 人类可读的状态 / 错误信息

    线性模型的数学：
      z = w0 * 1 + w1 * price + w2 * age
      p = 1 / (1 + exp(-z))

    每个特征对 p - 0.5（与无信息先验的偏差）的贡献：
      contribution_i = (w_i * x_i) / 4

    /4 这个归一化系数来自 logistic 在 z=0 处的斜率。
    我们除以 4 是为了让贡献与概率处于同一量级
    （因此贡献 0.1 表示"该特征把概率移动了 0.1"）。
    """
    from .train import _load_model_by_version

    if sample is None:
        sample = {"price": 0.5, "market_age_hours": 24.0}
    if not isinstance(sample, dict):
        return _err(model_version, "'sample' must be a dict")
    try:
        price = float(sample.get("price", 0.5))
        age = float(sample.get("market_age_hours", 24.0))
    except (TypeError, ValueError):
        return _err(model_version, "price / market_age_hours must be numbers")
    # v0.55 —— 域校验。Polymarket 的价格始终在 [0, 1]，
    # age 是自开盘以来的小时数。我们拒绝任何超出
    # 该范围的值，以保证模型输入合理（否则
    # 3 特征模型会进行疯狂的外推）。
    if not (0.0 <= price <= 1.0):
        return _err(model_version, "price must be in [0, 1]")
    if age < 0:
        return _err(model_version, "market_age_hours must be >= 0")

    # v0.55 —— 加载训练后的权重。我们先在
    # archive.jsonl 中查找（这样旧的 / 已回滚的模型
    # 仍然可以被解释），然后回退到 active。
    model = _load_model_by_version(model_version)
    if model is None:
        return _err(
            model_version,
            f"model {model_version!r} not found in archive or active",
        )
    weights = model.get("weights")
    if (
        not isinstance(weights, dict)
        or not all(k in weights for k in ("w0", "w1", "w2"))
    ):
        return _err(
            model_version,
            f"model {model_version!r} has no w0/w1/w2 weights",
        )
    try:
        w0 = float(weights["w0"])
        w1 = float(weights["w1"])
        w2 = float(weights["w2"])
    except (TypeError, ValueError):
        return _err(
            model_version,
            "weights w0/w1/w2 must be numbers",
        )

    # 构建特征向量 + logit + 概率。
    # x[0] 是偏置（始终为 1.0）。w[0] 是截距——
    # 它们一起在 sigmoid 之前平移 logit。
    x = [1.0, price, age]
    w = [w0, w1, w2]
    z = sum(wi * xi for wi, xi in zip(w, x))
    # 数值稳定的 sigmoid。两个分支的形式
    # 可以避免 z 较大负值（e^-z 会非常大）
    # 或较大正值（e^z 同理）时的溢出。
    if z >= 0:
        p = 1.0 / (1.0 + pow(2.718281828459045, -z))
    else:
        ez = pow(2.718281828459045, z)
        p = ez / (1.0 + ez)

    # 每个特征对 (p - 0.5) 的贡献。
    # 这里我们不再用 1/4 来归一化——而是
    # 使用实际的斜率：dp/dz = p(1-p)。
    # 在 z=0 处，斜率为 0.25。我们直接
    # 乘以斜率，使贡献保持在概率的量级。
    slope = p * (1.0 - p)
    contribs = []
    for fname, fvalue, fweight in zip(FEATURE_NAMES, x, w):
        c = fweight * fvalue * slope
        contribs.append(
            {
                "feature": fname,
                "value": round(fvalue, 6),
                "weight": round(fweight, 6),
                "contribution": round(c, 6),
                "abs_contribution": round(abs(c), 6),
            }
        )
    contribs.sort(key=lambda r: r["abs_contribution"], reverse=True)
    return {
        "ok": True,
        "model_version": model_version,
        "features": contribs,
        "prediction": round(p, 6),
        "sample": {
            "price": round(price, 6),
            "market_age_hours": round(age, 6),
        },
        "message": (
            f"per-feature contributions for {model_version} "
            f"at p={p:.4f}"
        ),
    }


def _err(model_version: str, msg: str) -> dict[str, Any]:
    return {
        "ok": False,
        "model_version": model_version,
        "features": [],
        "prediction": None,
        "sample": None,
        "message": msg,
    }
