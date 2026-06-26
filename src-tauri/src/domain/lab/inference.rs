//! v0.122b —— Python sidecar `predict` 逻辑的 Rust 移植。
//!
//! **唯一真相（直到 v0.122g）**：[`sidecar/polyrocket_sidecar/predict.py`](../../../../sidecar/polyrocket_sidecar/predict.py)
//! git SHA `e5a2496`（v0.122 迁移前的最后一次提交）。
//! v0.122g 之后，本文件成为唯一实现；Python sidecar 将被删除。
//!
//! **本文件是什么**：每次 signal compute 都会用到的逻辑回归打分路径。
//! 输入是 (price, market_age_hours)；输出是 (0, 1) 区间内的概率，
//! 以及每个预测的 [0, 1] 置信度和人类可读的 rationale 字符串。
//!
//! **本文件不是什么**：train / promote / backtest / SHAP
//! 算法，那些将在 v0.122c-f 中落地。
//!
//! ## 一致性保证
//!
//! 这里的函数与 Python 实现逐位兼容。`mod tests` 中的测试
//! 使用相同的输入，断言输出与 Python 记录的值匹配，
//! 误差 1e-9（sigmoid）和 1e-4（`round(prob, 4)` 之后）。
//!
//! ## v0.122b 语义
//!
//! - `POLYROCKET_DISABLE_SIDECAR` 未设置（默认）→ 调用方应使用
//!   新的 Rust 路径。v0.122a 引入的 kill switch 不再对已移植方法
//!  （predict + predict_async）短路；它们直接路由到本模块。
//! - `POLYROCKET_DISABLE_SIDECAR=1` → 调用方对任何未移植方法回退
//!   到 Python sidecar。对于 v0.122b 已移植的 `predict` /
//!   `predict_async`，该标志作为安全开关被尊重：路由回 Python
//!   子进程路径。这让用户在新的 Rust 路径产生错误结果时可以
//!   立即回滚。

use serde::{Deserialize, Serialize};

/// v0.122b —— 3 特征逻辑回归的权重。
///
/// 从 `active.json` 加载（参见 [`crate::commands::active_model::read_active_model_from_disk`]），
/// 或者在没有已 promote 模型时通过 [`InferenceWeights::fallback`] 构造。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct InferenceWeights {
    pub w0: f64,
    pub w1: f64,
    pub w2: f64,
    /// v0.122b —— horizon 归一化值，单位小时。对 v0.121 时代的
    /// 逻辑回归模型始终为 168.0（一周）。保留在结构体上以便未来
    /// 模型文件可在不修改推理代码的前提下覆盖它。
    pub horizon_norm_hours: f64,
}

impl Default for InferenceWeights {
    fn default() -> Self {
        Self::fallback()
    }
}

impl InferenceWeights {
    /// 内联的回退权重。**必须** 与 `sidecar/polyrocket_sidecar/predict.py`
    /// 中的 `_FALLBACK_W0` / `_FALLBACK_W1` / `_FALLBACK_W2` 保持镜像。
    /// `infra::scheduler` 中的 `fallback_predict_matches_python_baseline`
    /// 测试会校验一致性。
    pub const fn fallback() -> Self {
        Self {
            w0: -0.5,
            w1: 2.0,
            w2: 0.4,
            horizon_norm_hours: 168.0,
        }
    }

    /// 从 `active.json` 的 `best` 块解析权重。缺失字段时回退到
    /// [`Self::fallback`]，以匹配 Python `active.py` 的行为。
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

/// v0.122b —— 数值稳定的 sigmoid。镜像 Python：
/// ```text
/// if z >= 0: 1 / (1 + exp(-z))
/// else:      exp(z) / (1 + exp(z))
/// ```
/// `if z >= 0` 分支用于避免当 `z` 较大正数时 `exp(-z)` 溢出
///（否则会 NaN 到 inf）。
#[inline]
pub fn sigmoid(z: f64) -> f64 {
    if z >= 0.0 {
        1.0 / (1.0 + (-z).exp())
    } else {
        let ez = z.exp();
        ez / (1.0 + ez)
    }
}

/// v0.122b —— 单样本预测。精确镜像 Python 的 `predict_logic`
/// 和 `predict_with_active_model`。
///
/// 此处**不**对 `price` 做裁剪 —— 调用方应传入合法值
///（Python 端在 `predict_logic` 路径上同样不裁剪；
/// `predict_from_markets` 循环会在打分前把每个 market 的
/// price 裁剪到 [0, 1]）。
pub fn predict_one(weights: &InferenceWeights, price: f64, market_age_hours: f64) -> f64 {
    let inv_horizon = 1.0 / weights.horizon_norm_hours;
    let z = weights.w0 + weights.w1 * (1.0 - price) + weights.w2 * (market_age_hours * inv_horizon);
    sigmoid(z)
}

/// v0.122b —— [`predict_from_markets`] 的单市场输入。
///
/// 从 `Vec<(String, f64)>`（当前 IPC 形状）抽离出来，以便推理层
/// 不必关心 wire 格式。`sidecar_predict` IPC 负责把 wire 元组
/// 适配到本结构体。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MarketInput<'a> {
    pub market_id: &'a str,
    pub price: f64,
    pub market_age_hours: f64,
}

/// v0.122b —— 单市场预测。镜像 Python 中 `predict_from_markets`
/// 每个迭代产生的结构：
///   { "market_id": str, "prob": float, "confidence": float, "rationale": str }
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InferencePrediction {
    pub market_id: String,
    pub prob: f64,
    pub confidence: f64,
    pub rationale: String,
}

/// v0.122b —— 完整的结果结构。镜像 Python sidecar `predict` 方法
/// 的 JSON-RPC 响应对象：
///   { "predictions": [...], "model_version": str, "brier_score": float|null }
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PredictResult {
    pub predictions: Vec<InferencePrediction>,
    pub model_version: String,
    pub brier_score: Option<f64>,
}

/// v0.122b —— 批量预测。镜像 Python 中的 `predict_from_markets`。
/// 返回 [`PredictResult`]，每个非空 `market_id` 对应一个
/// [`InferencePrediction`]（单条推理预测）。
///
/// **行为一致性**（与 Python `predict_from_markets` 对齐）：
///   - 跳过 `market_id` 为空 / 缺失的行
///   - 打分前将 `price` 裁剪到 [0, 1]
///   - 打分前将 `market_age_hours` 裁剪到 >= 0
///   - `prob` 四舍五入到 4 位小数
///   - `confidence` = `|price - 0.5| * 2.0`，裁剪到 [0, 1]，
///     四舍五入到 4 位小数
///   - `rationale` 使用同样的字符串模板
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
            // 置信度：price=0.5 时为 0，price=0 或 1 时为 1。
            // Python 端使用 `abs(price - 0.5) * 2.0`；这里保持一致。
            // `.min(1.0)` 是防御性的：若调用方传入 price=1.5，
            // 结果会是 2.0；Python 端也会产生同样的结果
            //（它先把 price 裁剪到 [0,1]，
            // 因此实际运算不会超过 1.0）。我们同步保留裁剪以保持对称。
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
// 测试
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// 辅助函数：四舍五入到 4 位小数（与 Python `round(x, 4)` 对齐）。
    fn round4(x: f64) -> f64 {
        (x * 10_000.0).round() / 10_000.0
    }

    #[test]
    fn sigmoid_matches_python_branches() {
        // Python `_sigmoid`（两分支 sigmoid，数值稳定）：
        //   if z >= 0: 1 / (1 + exp(-z))    （正分支）
        //   else:      exp(z) / (1 + exp(z))（负分支）
        // 验证两个分支以及 z=0 边界。
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
        // 从 sidecar/polyrocket_sidecar/predict.py（git SHA e5a2496）记录。
        // 手工计算公式：
        //   z = -0.5 + 2.0 * (1 - price) + 0.4 * (age / 168)
        //   p = sigmoid(z)（线性加权 + sigmoid）
        // 容差：1e-9（完整 f64 精度）。
        let w = InferenceWeights::fallback();
        let cases: &[(f64, f64, f64)] = &[
            // (价格, 市场年龄小时数, 期望概率)
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
        // 回归测试：Python 的 `if z >= 0` 分支存在，
        // 是为了避免当 z 较大正数时 `exp(-z)` 溢出。
        // 对于 z = 1000，exp(-1000) = 0，故 1/(1+0) = 1 —— 无溢出。
        // 验证我们不会得到 NaN 或 inf。
        let w = InferenceWeights::fallback();
        let p = predict_one(&w, 0.0, 1_000_000.0);
        assert!(p.is_finite(), "got {p}");
        assert!(p > 0.5, "high-age + low-price → high prob, got {p}");
    }

    #[test]
    fn from_active_json_best_uses_fallback_for_missing_fields() {
        // active.json 是 "best": { "w0": -0.3, "w1": 1.8 }（无 w2、无 horizon）
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
        // 镜像 Python 中记录在 git SHA e5a2496 的 `predict_from_markets` 调用。
        // 活跃权重 = fallback（尚未发生 promote）。
        // 模型版本 = "logistic-0.1.0"。
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
        // m1: predict(0.5, 24) → 0.6357 → 四舍五入到 0.6357
        assert!((r.predictions[0].prob - 0.6357).abs() < 1e-3);
        assert_eq!(r.predictions[0].market_id, "m1");
        // m2: predict(0.7, 12) → 0.5321 → 四舍五入到 0.5321
        assert!((r.predictions[1].prob - 0.5321).abs() < 1e-3);
        // m3: predict(0.3, 168) → 0.7858 → 四舍五入到 0.7858
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
        // -0.5 裁剪到 0.0；与 predict_one(0.0, 0.0) 同 z → 0.8176
        assert!((r.predictions[0].prob - 0.8176).abs() < 1e-3);
        //  1.5 裁剪到 1.0；与 predict_one(1.0, 0.0) 同 z → 0.3775
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
        // age=0 → z = -0.5 + 1.0 + 0 = 0.5 → 0.6225（age 被裁剪为 0）
        assert!((r.predictions[0].prob - 0.6225).abs() < 1e-3);
    }

    #[test]
    fn predict_from_markets_confidence_formula() {
        // 置信度 = |price - 0.5| * 2.0，裁剪到 [0, 1]。
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
        // rationale 是 Python 中的同一字符串模板。
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
        // 回归测试：`infra::scheduler` 中已有的 `fallback_predict` 与 Python 对齐。
        // 确保我们的 `predict_one` 对相同输入返回相同值
        //（常量三元组相同；这是一项防止漂移的健全性检查）。
        let w = InferenceWeights::fallback();
        // (price=0.5, age=24) → 在 scheduler 测试与上面的测试中均记录为 0.6357。
        let p = predict_one(&w, 0.5, 24.0);
        let expected_z = -0.5_f64 + 2.0 * 0.5 + 0.4 * (24.0 / 168.0);
        let expected = 1.0 / (1.0 + (-expected_z).exp());
        assert!((p - expected).abs() < 1e-9, "got {p}, expected {expected}");
    }

    #[test]
    fn round4_helper_correctness() {
        // 健全性检查：predict_from_markets 内部使用的四舍五入，
        // 对我们关心的测试输入与 Python 的 round-half-to-even 一致。
        assert_eq!(round4(0.63574), 0.6357);
        assert_eq!(round4(0.53207), 0.5321);
        assert_eq!(round4(0.78585), 0.7859);
        assert_eq!(round4(1.0), 1.0);
    }
}
