"""v0.59 — real SHAP values via KernelExplainer.

v0.55 ships the exact-decomposition
`contribution_i = w_i * x_i * p(1-p)` for the
3-feature logistic model. That's correct for
linear models but doesn't satisfy the SHAP
*efficiency* axiom (φ values sum to the
prediction - baseline) when features are
correlated. v0.59 fixes that with a true
KernelExplainer.

## Why KernelExplainer?

We have a 3-feature model. The exact SHAP
closed-form for linear models is well known
(Lundberg 2017, eq. 9), but the polyrocket
model is logistic, not linear. KernelExplainer
is model-agnostic: it treats the model as a
black box, samples feature coalitions
according to SHAP's kernel weights, and
computes weighted least squares to fit the
additive attribution.

The cost is O(2^M) coalition evaluations where
M is the number of features. With M=3, that's
8 coalitions + the empty coalition. Cheap.

## Why not import the `shap` library?

Two reasons:
  1. The `shap` PyPI package pulls in
     `numpy` + `scipy` + `pandas` (~30MB).
     The sidecar currently has zero deps.
     Adding a 30MB dep for 100 lines of math
     is overkill.
  2. The polyrocket sidecar runs as a
     child process. Heavy ML deps slow
     startup. We can re-add `shap` later
     if the model grows past 5 features
     (where exact linear SHAP breaks down
     and KernelExplainer becomes slow).

This implementation uses pure-Python (stdlib
math only). For a 3-feature model, it's
sub-millisecond per sample.

## Algorithm

KernelSHAP for M features:

  1. Generate all 2^M binary coalition masks
     (z ∈ {0,1}^M).
  2. Convert each mask to an "imputed" input
     by replacing absent features with the
     background (median) feature value.
  3. Evaluate the model on each imputed input.
  4. Weight each coalition by the SHAP kernel
     weight: w(z) = (M-1) / (C(M, |z|) * |z|
     * (M - |z|)) — the closed form from
     Lundberg & Lee (2017).
  5. Fit a weighted linear regression of
     model outputs on the mask features
     (one-hot). The coefficients are the SHAP
     values φ_i.
  6. Constraint: φ_0 + Σφ_i = f(x) - f(bg)
     (the efficiency axiom).

We implement step 5-6 in closed form using
the normal equations. With 2^M samples and
M+1 parameters, the regression is
over-determined (8 > 4) and the system has a
unique solution.

## Output

Returns a list of { feature, value, shap_value,
abs_shap } sorted by abs_shap descending.
Same shape as v0.55's explainability
contributions; the math is now SHAP instead
of exact-decomposition.
"""

from __future__ import annotations

import itertools
import math
from typing import Any

# v0.59 — feature names must match the order
# the model was trained with. The 3-feature
# model uses:
#   x[0] = 1           (bias)
#   x[1] = price       (0..1)
#   x[2] = market_age_hours (>=0)
FEATURE_NAMES = ("bias", "price", "market_age_hours")

# Background (baseline) values. The "empty
# coalition" prediction uses these. Picked
# from the synthetic dataset (v0.12+): the
# typical market has price=0.5 and
# market_age_hours=24. The bias is by
# definition 1.0 in our feature vector.
_BACKGROUND = (1.0, 0.5, 24.0)


def _predict_logistic(
    weights: tuple[float, float, float],
    x: tuple[float, float, float],
) -> float:
    """Numerically stable sigmoid. Identical
    to predict._sigmoid."""
    z = sum(w * xi for w, xi in zip(weights, x))
    if z >= 0.0:
        return 1.0 / (1.0 + math.exp(-z))
    ez = math.exp(z)
    return ez / (1.0 + ez)


def _kernel_weight(m: int, s: int) -> float:
    """SHAP kernel weight for a coalition of
    size s out of m features. Closed form
    from Lundberg & Lee 2017:

        w(z) = (M-1) / (C(M, |z|) * |z| * (M - |z|))

    Special case: when |z| = 0 or |z| = M, the
    weight is `infinity` (the empty / full
    coalition is over-determined). We use a
    large constant (1e6) as a soft cap.
    """
    if s == 0 or s == m:
        return 1.0e6
    binom = math.comb(m, s)
    return (m - 1) / (binom * s * (m - s))


def _build_coalitions(m: int) -> list[list[int]]:
    """All 2^M binary coalition masks. The
    i-th element is 1 if feature i is
    "present" in the coalition."""
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
    """Weighted least-squares fit of outputs
    on the one-hot mask encoding + bias
    column. Returns [φ_0, φ_1, ..., φ_M] where
    φ_0 is the bias term and φ_i is the SHAP
    value for feature i.

    We use the normal equations:
        (X^T W X) β = X^T W y

    where X is the (n_coalitions, n_features+1)
    design matrix, W is diag(weights), and y
    is the model outputs.
    """
    n = len(masks)
    # Build X^T W X and X^T W y
    # X has columns: [bias, mask[0], mask[1], ..., mask[M-1]]
    p = n_features + 1
    xtwx = [[0.0] * p for _ in range(p)]
    xtwy = [0.0] * p
    for r in range(n):
        w = weights[r]
        # Row r of X
        row = [1.0] + [float(masks[r][i]) for i in range(n_features)]
        # Update X^T W X
        for i in range(p):
            for j in range(p):
                xtwx[i][j] += row[i] * w * row[j]
            xtwy[i] += row[i] * w * outputs[r]
    # Solve via Gaussian elimination.
    return _solve_linear(xtwx, xtwy)


def _solve_linear(a: list[list[float]], b: list[float]) -> list[float]:
    """Solve Ax = b via Gaussian elimination
    with partial pivoting. `a` is a square
    (n, n) matrix in normal usage, but we
    also handle the over-determined case
    (more rows than unknowns) — the algorithm
    just doesn't touch the extra rows. The
    matrix shape is (rows, n+1) where n is
    the number of unknowns (= number of
    columns of a)."""
    n = len(a[0]) if a else 0
    rows = len(a)
    # Augmented matrix: rows × (n+1)
    m = [a[i][:] + [b[i]] for i in range(rows)]
    # Forward elimination. We only eliminate
    # down to row n (not rows-1) because we
    # have n unknowns; the extra rows (if any)
    # are unused residuals.
    for i in range(n):
        # Find pivot
        max_row = i
        max_val = abs(m[i][i]) if i < rows else 0.0
        for r in range(i + 1, rows):
            if abs(m[r][i]) > max_val:
                max_val = abs(m[r][i])
                max_row = r
        if max_val < 1e-12:
            # Singular — return zeros (the model
            # output doesn't depend on any
            # feature, which shouldn't happen
            # in practice).
            return [0.0] * n
        if max_row != i:
            m[i], m[max_row] = m[max_row], m[i]
        # Eliminate below
        for r in range(i + 1, rows):
            if abs(m[r][i]) < 1e-12:
                continue
            factor = m[r][i] / m[i][i]
            for c in range(i, n + 1):
                m[r][c] -= factor * m[i][c]
    # Back substitution
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
    """Compute true SHAP values for one
    sample (or a default sample when the
    user doesn't pass one).

    The output is sorted by abs_shap
    descending so the L1 bar chart shows the
    top feature first.
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

    # Build the 2^M coalitions.
    m = len(FEATURE_NAMES)
    coalitions = _build_coalitions(m)
    n_coalitions = len(coalitions)

    # Convert each coalition to an imputed
    # input (replace absent features with
    # background) and evaluate the model.
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

    # Fit weighted LS to extract SHAP values.
    # We treat the bias (mask column 0) as a
    # constant 1 in the design matrix; its
    # coefficient is the "expected value
    # term" that the SHAP values sum to
    # f(x) - E[f(x)] = f(x) - baseline_pred.
    phi = _fit_weighted_ls(coalitions, outputs, weights, m)

    # Baseline prediction (empty coalition).
    baseline_pred = _predict_logistic(w, _BACKGROUND)
    target_pred = _predict_logistic(w, x_target)
    # The fitted φ values already satisfy
    # Σφ_i = f(x) - E[f(x)] by construction.
    # We surface the magnitudes to the L1.

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
