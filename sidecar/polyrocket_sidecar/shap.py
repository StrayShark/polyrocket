"""v0.59 —— 通过 KernelExplainer 计算真正的 SHAP values。

v0.55 对 3 特征 logistic 模型提供了精确分解
`contribution_i = w_i * x_i * p(1-p)`。这在线性模型中是
正确的，但在特征相关时**不**满足 SHAP 的 *efficiency*
公理（φ 值之和等于 prediction - baseline）。v0.59
通过真正的 KernelExplainer 来修复这个问题。

## 为什么使用 KernelExplainer？

我们有一个 3 特征模型。线性模型下 SHAP 的精确闭式
是已知的（Lundberg 2017, eq. 9），但 polyrocket 模型
是 logistic 的，不是线性的。KernelExplainer 是模型
无关的：它把模型当作一个黑盒，按 SHAP 的核权重
采样特征组合（coalition），并计算加权最小二乘来
拟合可加的属性分配。

代价是 O(2^M) 次 coalition 评估，其中 M 是特征数。
当 M=3 时，是 8 次 coalition + 一次空 coalition。非常便宜。

## 为什么不直接 import `shap` 库？

有两个原因：
  1. PyPI 上的 `shap` 包会引入 `numpy` + `scipy` +
     `pandas`（约 30MB）。侧车目前零依赖。
     为了 100 行数学代码添加 30MB 的依赖得不偿失。
  2. polyrocket 侧车作为子进程运行。沉重的 ML 依赖
     会拖慢启动。如果模型增长到 5 个特征以上
     （那时精确的线性 SHAP 失效，KernelExplainer
     也会变慢），我们之后再重新引入 `shap`。

本实现使用纯 Python（仅用标准库的 math）。对于
3 特征模型，每个样本的处理时间在亚毫秒级。

## 算法

对 M 个特征的 KernelSHAP：

  1. 生成所有 2^M 个二元 coalition mask
     （z ∈ {0,1}^M）。
  2. 将每个 mask 转换为一个"imputed"输入，
     用背景（取中位数）的特征值替换掉不在的
     特征。
  3. 对每个 imputed 输入评估模型。
  4. 用 SHAP 核权重为每个 coalition 分配权重：
     w(z) = (M-1) / (C(M, |z|) * |z| * (M - |z|))
     —— Lundberg & Lee (2017) 给出的闭式。
  5. 对模型输出在 mask 特征（one-hot）上拟合
     加权线性回归。其系数就是 SHAP values φ_i。
  6. 约束：φ_0 + Σφ_i = f(x) - f(bg)
     （efficiency 公理）。

我们使用正规方程的闭式实现步骤 5-6。对于 2^M 个
样本和 M+1 个参数，回归是超定的（8 > 4），系统有
唯一解。

## 输出

返回一个按 abs_shap 降序排列的
{ feature, value, shap_value, abs_shap } 列表。
结构与 v0.55 的可解释性贡献相同；只是数学上
现在是 SHAP 而不再是 exact-decomposition。
"""

from __future__ import annotations

import itertools
import math
from typing import Any

# v0.59 —— 特征名称必须与模型训练时的顺序一致。
# 3 特征模型使用：
#   x[0] = 1           （偏置）
#   x[1] = price       （0..1）
#   x[2] = market_age_hours （>=0）
FEATURE_NAMES = ("bias", "price", "market_age_hours")

# 背景（baseline）值。"空 coalition" 的预测会
# 使用这些值。从合成数据集（v0.12+）中选取：
# 典型市场的 price=0.5 且 market_age_hours=24。
# 偏置在我们的特征向量中按定义恒为 1.0。
_BACKGROUND = (1.0, 0.5, 24.0)


def _predict_logistic(
    weights: tuple[float, float, float],
    x: tuple[float, float, float],
) -> float:
    """数值稳定的 sigmoid。与 predict._sigmoid 相同。"""
    z = sum(w * xi for w, xi in zip(weights, x))
    if z >= 0.0:
        return 1.0 / (1.0 + math.exp(-z))
    ez = math.exp(z)
    return ez / (1.0 + ez)


def _kernel_weight(m: int, s: int) -> float:
    """大小为 s（特征总数为 m）的 coalition 的 SHAP 核
    权重。Lundberg & Lee 2017 给出的闭式：

        w(z) = (M-1) / (C(M, |z|) * |z| * (M - |z|))

    特殊情况：当 |z| = 0 或 |z| = M 时，权重为
    `infinity`（空 / 完整 coalition 是超定的）。
    我们用一个大常数（1e6）作为软上限。
    """
    if s == 0 or s == m:
        return 1.0e6
    binom = math.comb(m, s)
    return (m - 1) / (binom * s * (m - s))


def _build_coalitions(m: int) -> list[list[int]]:
    """所有 2^M 个二元 coalition mask。第 i 个元素
    为 1 表示特征 i 在该 coalition 中"存在"。"""
    return [
        [int(b) for b in format(idx, f"0{m}b")]
        for idx in range(2**m)
    ]


def _fit_weighted_ls(
    masks: list[list[int]],
    outputs: list[float],
    weights: list[float],
    n_features: int,
) -> list[float]:
    """对 one-hot mask 编码 + 偏置列上的输出进行
    加权最小二乘拟合。返回 [φ_0, φ_1, ..., φ_M]，
    其中 φ_0 是偏置项，φ_i 是特征 i 的 SHAP value。

    我们使用正规方程：
        (X^T W X) β = X^T W y

    其中 X 是 (n_coalitions, n_features+1) 的设计矩阵，
    W 是 diag(weights)，y 是模型的输出。
    """
    n = len(masks)
    # 构造 X^T W X 与 X^T W y
    # X 的列依次为：[bias, mask[0], mask[1], ..., mask[M-1]]
    p = n_features + 1
    xtwx = [[0.0] * p for _ in range(p)]
    xtwy = [0.0] * p
    for r in range(n):
        w = weights[r]
        # X 的第 r 行
        row = [1.0] + [float(masks[r][i]) for i in range(n_features)]
        # 更新 X^T W X
        for i in range(p):
            for j in range(p):
                xtwx[i][j] += row[i] * w * row[j]
            xtwy[i] += row[i] * w * outputs[r]
    # 通过高斯消元求解。
    return _solve_linear(xtwx, xtwy)


def _solve_linear(a: list[list[float]], b: list[float]) -> list[float]:
    """通过带部分主元的高斯消元求解 Ax = b。在正常用法中
    `a` 是一个 (n, n) 的方阵，但我们也处理超定情形
    （行数多于未知数）——算法只是不访问额外的行。
    矩阵的形状为 (rows, n+1)，其中 n 是未知数的数量
    （= a 的列数）。
    """
    n = len(a[0]) if a else 0
    rows = len(a)
    # 增广矩阵：rows × (n+1)
    m = [a[i][:] + [b[i]] for i in range(rows)]
    # 前向消元。我们只消到第 n 行（不是 rows-1），
    # 因为我们有 n 个未知数；额外的行（如果有）
    # 是未使用的残差。
    for i in range(n):
        # 寻找主元
        max_row = i
        max_val = abs(m[i][i]) if i < rows else 0.0
        for r in range(i + 1, rows):
            if abs(m[r][i]) > max_val:
                max_val = abs(m[r][i])
                max_row = r
        if max_val < 1e-12:
            # 奇异 —— 返回零（模型输出不依赖于
            # 任何特征，这在实践中不应发生）。
            return [0.0] * n
        if max_row != i:
            m[i], m[max_row] = m[max_row], m[i]
        # 向下消元
        for r in range(i + 1, rows):
            if abs(m[r][i]) < 1e-12:
                continue
            factor = m[r][i] / m[i][i]
            for c in range(i, n + 1):
                m[r][c] -= factor * m[i][c]
    # 回代
    x = [0.0] * n
    for i in range(n - 1, -1, -1):
        s = m[i][n]
        for j in range(i + 1, n):
            s -= m[i][j] * x[j]
        if abs(m[i][i]) < 1e-12:
            x[i] = 0.0
        else:
            x[i] = s / m[i][i]
    return x


def run_shap_explainability(
    *,
    model_version: str,
    sample: dict[str, Any] | None = None,
) -> dict[str, Any]:
    """为单个样本计算真正的 SHAP values（如果用户
    未传入 sample，则使用默认样本）。

    输出按 abs_shap 降序排列，这样 L1 的条形图就能
    把最重要的特征排在最前面。
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
    if not (0.0 <= price <= 1.0):
        return _err(model_version, "price must be in [0, 1]")
    if age < 0:
        return _err(model_version, "market_age_hours must be >= 0")

    model = _load_model_by_version(model_version)
    if model is None:
        return _err(
            model_version,
            f"model {model_version!r} not found in archive or active",
        )
    weights_dict = model.get("weights")
    if (
        not isinstance(weights_dict, dict)
        or not all(k in weights_dict for k in ("w0", "w1", "w2"))
    ):
        return _err(
            model_version,
            f"model {model_version!r} has no w0/w1/w2 weights",
        )
    try:
        w = (
            float(weights_dict["w0"]),
            float(weights_dict["w1"]),
            float(weights_dict["w2"]),
        )
    except (TypeError, ValueError):
        return _err(model_version, "weights w0/w1/w2 must be numbers")

    # 构造 2^M 个 coalition。
    m = len(FEATURE_NAMES)
    coalitions = _build_coalitions(m)
    n_coalitions = len(coalitions)

    # 将每个 coalition 转换为 imputed 输入
    # （用背景值替换不在的特征）并评估模型。
    x_target = (1.0, price, age)
    outputs: list[float] = []
    weights: list[float] = []
    for mask in coalitions:
        imputed = tuple(
            x_target[i] if mask[i] else _BACKGROUND[i]
            for i in range(m)
        )
        outputs.append(_predict_logistic(w, imputed))
        s = sum(mask)
        weights.append(_kernel_weight(m, s))

    # 拟合加权 LS 以提取 SHAP values。
    # 我们把偏置（mask 第 0 列）视作设计矩阵中
    # 的常数 1；其系数就是"expected value 项"，
    # 它满足 SHAP values 之和为
    # f(x) - E[f(x)] = f(x) - baseline_pred。
    phi = _fit_weighted_ls(coalitions, outputs, weights, m)

    # 基准预测（空 coalition）。
    baseline_pred = _predict_logistic(w, _BACKGROUND)
    target_pred = _predict_logistic(w, x_target)
    # 拟合出的 φ 值在构造上已经满足
    # Σφ_i = f(x) - E[f(x)]。我们把
    # 它们的幅值暴露给 L1。

    features: list[dict[str, Any]] = []
    for i, name in enumerate(FEATURE_NAMES):
        features.append(
            {
                "feature": name,
                "value": round(x_target[i], 6),
                "weight": round(w[i], 6),
                "shap_value": round(phi[i + 1], 6),
                "abs_shap": round(abs(phi[i + 1]), 6),
            }
        )
    features.sort(key=lambda r: r["abs_shap"], reverse=True)
    return {
        "ok": True,
        "model_version": model_version,
        "method": "kernel_shap",
        "features": features,
        "baseline_prediction": round(baseline_pred, 6),
        "target_prediction": round(target_pred, 6),
        "efficiency_diff": round(
            sum(f["shap_value"] for f in features)
            - (target_pred - baseline_pred),
            6,
        ),
        "sample": {
            "price": round(price, 6),
            "market_age_hours": round(age, 6),
        },
        "message": (
            f"KernelSHAP for {model_version} "
            f"at p={target_pred:.4f} "
            f"(baseline={baseline_pred:.4f})"
        ),
    }


def _err(model_version: str, msg: str) -> dict[str, Any]:
    return {
        "ok": False,
        "model_version": model_version,
        "method": "kernel_shap",
        "features": [],
        "baseline_prediction": None,
        "target_prediction": None,
        "efficiency_diff": None,
        "sample": None,
        "message": msg,
    }
