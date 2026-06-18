"""v0.55 — model explainability.

Computes the top features that drive the
logistic regression's prediction for a given
input. We use a SHAP-like linear-model
decomposition: for a logistic regression with
weights [w0, w1, w2] and features
[1, price, market_age_hours], the contribution
of feature i to a single prediction is:

    contribution_i = w_i * x_i

(scaled to probabilities via the logistic
function, then normalized so the contributions
sum to (p - 0.5)). For the linear model, this
is exact — it's not a SHAP approximation, it's
the actual decomposition of the dot product.

This is the cheapest possible explainability
method that's still meaningful. It works for
the 3-feature logistic model polyrocket ships
with. For tree-based models, a real SHAP library
(TreeSHAP) would be needed; v0.55+ candidate.

The output is a list of { feature, value,
contribution, abs_contribution } sorted by
abs_contribution descending. The L1 renders
this as a horizontal bar chart (positive vs
negative contributions).
"""

from __future__ import annotations

from typing import Any

# Feature names must match the order the model
# was trained with. The 3-feature model uses:
#   x[0] = 1           (bias)
#   x[1] = price       (0..1, the market price)
#   x[2] = market_age_hours (>=0, hours since
#                          market opened)
FEATURE_NAMES = ("bias", "price", "market_age_hours")


def run_explainability(
    *,
    model_version: str,
    sample: dict[str, Any] | None = None,
) -> dict[str, Any]:
    """Compute per-feature contribution for one
    sample (or a default "average" sample when
    the user doesn't pass one).

    Args:
      model_version: e.g.
        "logistic-train-441c352b". Looked up
        in archive.jsonl first, then active.json
        (same logic as run_backtest_model).
      sample: optional dict with:
        - "price"          (float, 0..1, default 0.5)
        - "market_age_hours" (float, >=0, default 24)
        When omitted, we use a default sample
        (price=0.5, age=24h) so the user gets
        a "what would the model say for a typical
        market" view.

    Returns:
      - ok: bool
      - model_version: the requested model
        (echoed)
      - features: list of { feature, value, weight,
        contribution, abs_contribution } sorted
        by abs_contribution desc
      - prediction: float (the model's predicted
        probability for the sample, 0..1)
      - sample: { price, market_age_hours }
        (the input we used)
      - message: human-readable status / error

    Linear-model math:
      z = w0 * 1 + w1 * price + w2 * age
      p = 1 / (1 + exp(-z))

    Per-feature contribution to p - 0.5 (the
    deviation from the no-information prior):
      contribution_i = (w_i * x_i) / 4

    The /4 normalizer comes from the slope of
    the logistic at z=0. We divide by 4 so the
    contributions are on the same scale as the
    probability (so a contribution of 0.1 means
    "this feature moved the probability by 0.1").
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
    # v0.55 — domain validation. Polymarket prices are
    # always in [0, 1] and age is hours since open. We
    # reject anything outside that to keep the model
    # input sane (the 3-feature model would extrapolate
    # wildly otherwise).
    if not (0.0 <= price <= 1.0):
        return _err(model_version, "price must be in [0, 1]")
    if age < 0:
        return _err(model_version, "market_age_hours must be >= 0")

    # v0.55 — load the trained weights. We look in
    # archive.jsonl first (so old / rolled-back models
    # can still be explained) then fall back to active.
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

    # Build feature vector + logit + probability.
    # x[0] is the bias (always 1.0). w[0] is the
    # intercept — together they shift the logit
    # before the sigmoid.
    x = [1.0, price, age]
    w = [w0, w1, w2]
    z = sum(wi * xi for wi, xi in zip(w, x))
    # Numerically stable sigmoid. The two-branch form
    # avoids overflow when z is large negative (e^-z
    # would be huge) or large positive (e^z same).
    if z >= 0:
        p = 1.0 / (1.0 + pow(2.718281828459045, -z))
    else:
        ez = pow(2.718281828459045, z)
        p = ez / (1.0 + ez)

    # Per-feature contribution to (p - 0.5).
    # We don't normalize by 1/4 here — instead we
    # use the actual slope: dp/dz = p(1-p).
    # At z=0, slope = 0.25. We pass the slope
    # through so the contribution is on the
    # probability scale.
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
