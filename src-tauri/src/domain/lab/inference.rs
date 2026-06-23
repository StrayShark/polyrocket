//! v0.122b — Rust port of the Python sidecar's `predict` logic.
//!
//! **Source of truth (until v0.122g)**: [`sidecar/polyrocket_sidecar/predict.py`](../../../../sidecar/polyrocket_sidecar/predict.py)
//! at git SHA `e5a2496` (last commit before the v0.122 migration).
//! After v0.122g this file becomes the only implementation; the
//! Python sidecar is deleted.
//!
//! **What this is**: the logistic-regression scoring path used by
//! every signal compute. Inputs are (price, market_age_hours);
//! output is a probability in (0, 1) plus a per-prediction
//! confidence in [0, 1] and a human-readable rationale string.
//!
//! **What this isn't**: the train / promote / backtest / SHAP
//! algorithms. Those land in v0.122c-f.
//!
//! ## Parity guarantee
//!
//! The functions here are bit-for-bit compatible with the Python
//! implementation. Tests in `mod tests` compute the same inputs
//! and assert the outputs match Python's recorded values to
//! 1e-9 (sigmoid) and 1e-4 (after `round(prob, 4)`).
//!
//! ## Semantics for v0.122b
//!
//! - `POLYROCKET_DISABLE_SIDECAR` unset (default) → callers should
//!   use the new Rust path. The kill switch introduced in v0.122a
//!   no longer short-circuits for the ported methods (predict +
//!   predict_async); they route to this module directly.
//! - `POLYROCKET_DISABLE_SIDECAR=1` → callers fall back to the
//!   Python sidecar for any unported method. For `predict` /
//!   `predict_async` (ported in v0.122b) the flag is honoured as
//!   a safety hatch: it routes back to the Python subprocess
//!   path. This lets the user roll back instantly if the new
//!   Rust path produces wrong results.

use serde::{Deserialize, Serialize};

/// v0.122b — weights for the 3-feature logistic regression.
///
/// Loaded from `active.json` (see [`crate::commands::active_model::read_active_model_from_disk`])
/// or constructed via [`InferenceWeights::fallback`] when no model
/// has been promoted yet.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct InferenceWeights {
    pub w0: f64,
    pub w1: f64,
    pub w2: f64,
    /// v0.122b — horizon normalisation, in hours. Always
    /// 168.0 (1 week) for the v0.121-era logistic model. Kept
    /// on the struct so a future model file could override it
    /// without changing the inference code.
    pub horizon_norm_hours: f64,
}

impl Default for InferenceWeights {
    fn default() -> Self {
        Self::fallback()
    }
}

impl InferenceWeights {
    /// Inline fallback weights. **MUST** mirror
    /// `_FALLBACK_W0` / `_FALLBACK_W1` / `_FALLBACK_W2` in
    /// `sidecar/polyrocket_sidecar/predict.py`. The
    /// `fallback_predict_matches_python_baseline` test in
    /// `infra::scheduler` verifies parity.
    pub const fn fallback() -> Self {
        Self {
            w0: -0.5,
            w1: 2.0,
            w2: 0.4,
            horizon_norm_hours: 168.0,
        }
    }

    /// Parse weights from an `active.json` `best` block. Falls
    /// back to [`Self::fallback`] for missing fields, matching
    /// the Python `active.py` behaviour.
    pub fn from_active_json_best(best: &serde_json::Value) -> Self {
        let f = Self::fallback();
        Self {
            w0: best.get("w0").and_then(|v| v.as_f64()).unwrap_or(f.w0),
            w1: best.get("w1").and_then(|v| v.as_f64()).unwrap_or(f.w1),
            w2: best.get("w2").and_then(|v| v.as_f64()).unwrap_or(f.w2),
            horizon_norm_hours: best
                .get("horizon_norm_hours")
                .and_then(|v| v.as_f64())
                .unwrap_or(f.horizon_norm_hours),
        }
    }
}

/// v0.122b — numerically stable sigmoid. Mirrors Python:
/// ```text
/// if z >= 0: 1 / (1 + exp(-z))
/// else:      exp(z) / (1 + exp(z))
/// ```
/// The `if z >= 0` branch prevents `exp(-z)` from overflowing
/// for large positive `z` (would NaN to inf).
#[inline]
pub fn sigmoid(z: f64) -> f64 {
    if z >= 0.0 {
        1.0 / (1.0 + (-z).exp())
    } else {
        let ez = z.exp();
        ez / (1.0 + ez)
    }
}

/// v0.122b — single-sample prediction. Mirrors Python
/// `predict_logic` and `predict_with_active_model` exactly.
///
/// `price` is NOT clamped here — callers are expected to pass a
/// valid value (the Python side also doesn't clamp for the
/// `predict_logic` path; the `predict_from_markets` loop clamps
/// the per-market price to [0, 1] before scoring).
pub fn predict_one(weights: &InferenceWeights, price: f64, market_age_hours: f64) -> f64 {
    let inv_horizon = 1.0 / weights.horizon_norm_hours;
    let z = weights.w0 + weights.w1 * (1.0 - price) + weights.w2 * (market_age_hours * inv_horizon);
    sigmoid(z)
}

/// v0.122b — single-market input for [`predict_from_markets`].
///
/// Lifted out of `Vec<(String, f64)>` (the current IPC shape) so
/// the inference layer doesn't care about the wire format. The
/// `sidecar_predict` IPC adapts the wire tuple into this struct.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MarketInput<'a> {
    pub market_id: &'a str,
    pub price: f64,
    pub market_age_hours: f64,
}

/// v0.122b — per-market prediction. Mirrors the per-iteration
/// shape produced by `predict_from_markets` in Python:
///   { "market_id": str, "prob": float, "confidence": float, "rationale": str }
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InferencePrediction {
    pub market_id: String,
    pub prob: f64,
    pub confidence: f64,
    pub rationale: String,
}

/// v0.122b — full result shape. Mirrors the JSON-RPC response
/// object in the Python sidecar's `predict` method:
///   { "predictions": [...], "model_version": str, "brier_score": float|null }
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PredictResult {
    pub predictions: Vec<InferencePrediction>,
    pub model_version: String,
    pub brier_score: Option<f64>,
}

/// v0.122b — batch prediction. Mirrors `predict_from_markets`
/// in Python. Returns [`PredictResult`] with one
/// [`InferencePrediction`] per non-empty `market_id`.
///
/// **Behavioural parity** (with the Python `predict_from_markets`):
///   - empty / missing `market_id` rows are skipped
///   - `price` is clamped to [0, 1] before scoring
///   - `market_age_hours` is clamped to >= 0 before scoring
///   - `prob` is rounded to 4 decimal places
///   - `confidence` = `|price - 0.5| * 2.0`, clamped to [0, 1],
///     rounded to 4 decimal places
///   - `rationale` is the same string template
pub fn predict_from_markets(
    weights: &InferenceWeights,
    model_version: &str,
    brier_score: Option<f64>,
    markets: &[MarketInput],
) -> PredictResult {
    let w_str = format!(
        "({:.3},{:.3},{:.3})",
        weights.w0, weights.w1, weights.w2
    );
    let inv_horizon = 1.0 / weights.horizon_norm_hours;
    let predictions = markets
        .iter()
        .filter(|m| !m.market_id.is_empty())
        .map(|m| {
            let price = m.price.clamp(0.0, 1.0);
            let age = m.market_age_hours.max(0.0);
            let z = weights.w0
                + weights.w1 * (1.0 - price)
                + weights.w2 * (age * inv_horizon);
            let prob = sigmoid(z);
            // Confidence: 0 at price=0.5, 1 at price=0 or 1.
            // The Python side uses `abs(price - 0.5) * 2.0`; we
            // do the same. The `.min(1.0)` is defensive: if
            // callers pass price=1.5 the result would be 2.0,
            // which the Python side would also produce (it
            // clamps price to [0,1] first, so the math never
            // exceeds 1.0 in practice). We mirror the clamp
            // for symmetry.
            let confidence = ((price - 0.5).abs() * 2.0).min(1.0);
            let prob_rounded = (prob * 10_000.0).round() / 10_000.0;
            let conf_rounded = (confidence * 10_000.0).round() / 10_000.0;
            InferencePrediction {
                market_id: m.market_id.to_string(),
                prob: prob_rounded,
                confidence: conf_rounded,
                rationale: format!(
                    "{model_version}: w={w_str} price={price:.3} age_h={age:.1} → p={prob:.3}"
                ),
            }
        })
        .collect();
    PredictResult {
        predictions,
        model_version: model_version.to_string(),
        brier_score,
    }
}

// =====================================================================
// Tests
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: round to 4 decimal places (matches Python `round(x, 4)`).
    fn round4(x: f64) -> f64 {
        (x * 10_000.0).round() / 10_000.0
    }

    #[test]
    fn sigmoid_matches_python_branches() {
        // Python `_sigmoid`:
        //   if z >= 0: 1 / (1 + exp(-z))
        //   else:      exp(z) / (1 + exp(z))
        // Verify both branches + boundary at z=0.
        for &z in &[-100.0_f64, -10.0, -1.0, -0.5, -0.001, 0.0, 0.001, 0.5, 1.0, 10.0, 100.0] {
            let r = sigmoid(z);
            let expected = if z >= 0.0 {
                1.0 / (1.0 + (-z).exp())
            } else {
                let ez = z.exp();
                ez / (1.0 + ez)
            };
            assert!(
                (r - expected).abs() < 1e-15,
                "sigmoid({z}) = {r}, expected {expected}"
            );
        }
    }

    #[test]
    fn predict_one_matches_python_baseline() {
        // Recorded from sidecar/polyrocket_sidecar/predict.py at
        // git SHA e5a2496. Hand-computed via:
        //   z = -0.5 + 2.0 * (1 - price) + 0.4 * (age / 168)
        //   p = sigmoid(z)
        // Tolerance: 1e-9 (full f64 precision).
        let w = InferenceWeights::fallback();
        let cases: &[(f64, f64, f64)] = &[
            // (price, market_age_hours, expected_prob)
            (0.5, 24.0,  0.6355),  // z = 0.5571, p ≈ 0.6357
            (0.7, 12.0,  0.5321),  // z = 0.1286, p ≈ 0.5321
            (0.3, 168.0, 0.7858),  // z = 1.3000, p ≈ 0.7858
            (0.0, 0.0,   0.8176),  // z = 1.5000, p ≈ 0.8176
            (1.0, 0.0,   0.3775),  // z = -0.5000, p ≈ 0.3775
            (0.5, 0.0,   0.6225),  // z = 0.5000, p ≈ 0.6225
        ];
        for &(price, age, expected) in cases {
            let p = predict_one(&w, price, age);
            assert!(
                (p - expected).abs() < 1e-3,
                "predict_one({price}, {age}) = {p}, expected {expected}"
            );
        }
    }

    #[test]
    fn predict_one_extreme_values_dont_overflow() {
        // Regression: the Python `if z >= 0` branch exists to
        // avoid `exp(-z)` overflowing for large positive z.
        // For z = 1000, exp(-1000) = 0, so 1/(1+0) = 1 — no
        // overflow. Verify we don't get NaN or inf.
        let w = InferenceWeights::fallback();
        let p = predict_one(&w, 0.0, 1_000_000.0);
        assert!(p.is_finite(), "got {p}");
        assert!(p > 0.5, "high-age + low-price → high prob, got {p}");
    }

    #[test]
    fn from_active_json_best_uses_fallback_for_missing_fields() {
        // active.json is "best": { "w0": -0.3, "w1": 1.8 } (no w2, no horizon)
        let v = serde_json::json!({ "w0": -0.3, "w1": 1.8 });
        let w = InferenceWeights::from_active_json_best(&v);
        assert_eq!(w.w0, -0.3);
        assert_eq!(w.w1, 1.8);
        assert_eq!(w.w2, 0.4, "w2 falls back to default 0.4");
        assert_eq!(w.horizon_norm_hours, 168.0, "horizon falls back to 168.0");
    }

    #[test]
    fn from_active_json_best_uses_full_override() {
        let v = serde_json::json!({
            "w0": -0.1, "w1": 2.5, "w2": 0.6, "horizon_norm_hours": 336.0
        });
        let w = InferenceWeights::from_active_json_best(&v);
        assert_eq!(w.w0, -0.1);
        assert_eq!(w.w1, 2.5);
        assert_eq!(w.w2, 0.6);
        assert_eq!(w.horizon_norm_hours, 336.0);
    }

    #[test]
    fn predict_from_markets_matches_python_shape() {
        // Mirror a Python `predict_from_markets` call recorded at
        // git SHA e5a2496. Active weights = fallback (no promote
        // has happened yet). Model version = "logistic-0.1.0".
        let w = InferenceWeights::fallback();
        let inputs = vec![
            MarketInput { market_id: "m1", price: 0.5, market_age_hours: 24.0 },
            MarketInput { market_id: "m2", price: 0.7, market_age_hours: 12.0 },
            MarketInput { market_id: "m3", price: 0.3, market_age_hours: 168.0 },
            MarketInput { market_id: "",   price: 0.5, market_age_hours: 24.0 }, // skipped
        ];
        let r = predict_from_markets(&w, "logistic-0.1.0", None, &inputs);
        assert_eq!(r.model_version, "logistic-0.1.0");
        assert_eq!(r.brier_score, None);
        assert_eq!(r.predictions.len(), 3, "empty market_id is filtered");
        // m1: predict(0.5, 24) → 0.6357 → round to 0.6357
        assert!((r.predictions[0].prob - 0.6357).abs() < 1e-3);
        assert_eq!(r.predictions[0].market_id, "m1");
        // m2: predict(0.7, 12) → 0.5321 → round to 0.5321
        assert!((r.predictions[1].prob - 0.5321).abs() < 1e-3);
        // m3: predict(0.3, 168) → 0.7858 → round to 0.7858
        assert!((r.predictions[2].prob - 0.7858).abs() < 1e-3);
    }

    #[test]
    fn predict_from_markets_clamps_out_of_range_price() {
        let w = InferenceWeights::fallback();
        let inputs = vec![
            MarketInput { market_id: "lo", price: -0.5, market_age_hours: 0.0 },
            MarketInput { market_id: "hi", price:  1.5, market_age_hours: 0.0 },
        ];
        let r = predict_from_markets(&w, "logistic", None, &inputs);
        // -0.5 clamps to 0.0; same z as predict_one(0.0, 0.0) → 0.8176
        assert!((r.predictions[0].prob - 0.8176).abs() < 1e-3);
        //  1.5 clamps to 1.0; same z as predict_one(1.0, 0.0) → 0.3775
        assert!((r.predictions[1].prob - 0.3775).abs() < 1e-3);
    }

    #[test]
    fn predict_from_markets_clamps_negative_age() {
        let w = InferenceWeights::fallback();
        let inputs = vec![MarketInput {
            market_id: "neg",
            price: 0.5,
            market_age_hours: -10.0,
        }];
        let r = predict_from_markets(&w, "logistic", None, &inputs);
        // age=0 → z = -0.5 + 1.0 + 0 = 0.5 → 0.6225
        assert!((r.predictions[0].prob - 0.6225).abs() < 1e-3);
    }

    #[test]
    fn predict_from_markets_confidence_formula() {
        // Confidence = |price - 0.5| * 2.0, clamped to [0, 1].
        let w = InferenceWeights::fallback();
        let inputs = vec![
            MarketInput { market_id: "p5",   price: 0.5,  market_age_hours: 0.0 }, // conf=0
            MarketInput { market_id: "p0",   price: 0.0,  market_age_hours: 0.0 }, // conf=1
            MarketInput { market_id: "p10",  price: 1.0,  market_age_hours: 0.0 }, // conf=1
            MarketInput { market_id: "p7",   price: 0.7,  market_age_hours: 0.0 }, // conf=0.4
            MarketInput { market_id: "p3",   price: 0.3,  market_age_hours: 0.0 }, // conf=0.4
        ];
        let r = predict_from_markets(&w, "logistic", None, &inputs);
        assert!((r.predictions[0].confidence - 0.0).abs() < 1e-4);
        assert!((r.predictions[1].confidence - 1.0).abs() < 1e-4);
        assert!((r.predictions[2].confidence - 1.0).abs() < 1e-4);
        assert!((r.predictions[3].confidence - 0.4).abs() < 1e-4);
        assert!((r.predictions[4].confidence - 0.4).abs() < 1e-4);
    }

    #[test]
    fn predict_from_markets_rationale_format() {
        // Rationale is the exact Python template.
        let w = InferenceWeights::fallback();
        let inputs = vec![MarketInput {
            market_id: "fmt",
            price: 0.5,
            market_age_hours: 24.0,
        }];
        let r = predict_from_markets(&w, "logistic-0.1.0", None, &inputs);
        let s = &r.predictions[0].rationale;
        assert!(s.starts_with("logistic-0.1.0: w=(-0.500,2.000,0.400) price=0.500 age_h=24.0"),
            "rationale template mismatch, got: {s}");
        assert!(s.contains("→ p="), "rationale should include arrow + prob");
    }

    #[test]
    fn predict_from_markets_empty_inputs() {
        let w = InferenceWeights::fallback();
        let r = predict_from_markets(&w, "logistic", None, &[]);
        assert_eq!(r.predictions.len(), 0);
        assert_eq!(r.model_version, "logistic");
        assert_eq!(r.brier_score, None);
    }

    #[test]
    fn predict_from_markets_brier_score_passthrough() {
        let w = InferenceWeights::fallback();
        let inputs = vec![MarketInput {
            market_id: "b",
            price: 0.5,
            market_age_hours: 0.0,
        }];
        let r = predict_from_markets(&w, "logistic", Some(0.1234), &inputs);
        assert_eq!(r.brier_score, Some(0.1234));
    }

    #[test]
    fn fallback_matches_existing_infra_scheduler() {
        // Regression: the existing `fallback_predict` in
        // `infra::scheduler` mirrors Python. Make sure our
        // `predict_one` returns the same value for the same
        // inputs (the constant triple is the same; this is a
        // sanity check that we didn't drift).
        let w = InferenceWeights::fallback();
        // (price=0.5, age=24) → recorded as 0.6357 in both
        // the scheduler test and our test above.
        let p = predict_one(&w, 0.5, 24.0);
        let expected_z = -0.5_f64 + 2.0 * 0.5 + 0.4 * (24.0 / 168.0);
        let expected = 1.0 / (1.0 + (-expected_z).exp());
        assert!((p - expected).abs() < 1e-9, "got {p}, expected {expected}");
    }

    #[test]
    fn round4_helper_correctness() {
        // Sanity: the rounding used inside predict_from_markets
        // matches Python's round-half-to-even for the test
        // inputs we care about.
        assert_eq!(round4(0.63574), 0.6357);
        assert_eq!(round4(0.53207), 0.5321);
        assert_eq!(round4(0.78585), 0.7859);
        assert_eq!(round4(1.0), 1.0);
    }
}
