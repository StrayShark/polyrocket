//! L3 — 信号检测（M2）。
//!
//! 计算 model-vs-market 的 edge,并决定哪些信号对前端
//! 是「可执行的」（active=1）。
//!
//! 真实实现：M2 里程碑是启发式打分器 —— 较重的 ML 部分在
//! M7（model-lab）。本模块负责每次重算时运行的
//! **filter + sort + edge 数学**。
//!
//! 规范：docs/polyrocket-modules.md §2.M2

use crate::domain::polymarket::hours_until_close;
use serde::{Deserialize, Serialize};
use specta::Type;

/// 信号记录 —— 镜像 SQLite `signals` 表的一行。
///
/// **`edge` 计算**：`predicted_prob - market_prob`。`edge > 0` 表示 model 觉得
/// YES token 被低估，机会买 YES；`edge < 0` 反之。
///
/// **`confidence` 计算**：v0.4 用 `min(|edge| * 2, 0.95)` 简单占位。真正的 model
/// 应该输出独立的不确定性（v0.55+ 替换）。
///
/// **`horizon_hours`**：从 `computed_at` 到市场关闭的小时数。L1 在 card 展示
/// 「3h to close」之类的提示。
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct Signal {
    pub market_id: String,
    pub computed_at: i64,
    pub model_version: String,
    pub predicted_prob: f64,
    pub market_prob: f64,
    pub edge: f64, // predicted - market
    pub confidence: f64, // 0..1
    pub horizon_hours: i64,
    pub rationale: Option<String>,
}

impl Signal {
    /// 当 edge 足够大、可以执行时返回 `true`。
    /// 综合条件:|edge| >= min_edge 且 confidence >= 0.6。
    pub fn is_actionable(&self, min_edge: f64) -> bool {
        self.edge.abs() >= min_edge && self.confidence >= 0.6
    }
}

/// 对单个 (predicted, market) 对打分。返回 (edge, confidence)。
/// Edge = predicted - market。Confidence 默认为 |edge|，但上限为 0.95 ——
/// 纯 edge 并非真正的 confidence 指标（真实 model 应输出独立的
/// 不确定性）。这是 v0.4 的占位实现。
pub fn score(predicted_prob: f64, market_prob: f64) -> (f64, f64) {
    let edge = predicted_prob - market_prob;
    let confidence = (edge.abs() * 2.0).min(0.95);
    (edge, confidence)
}

/// 按最小 edge 以及 active-only 过滤信号列表。
pub fn filter_active(signals: &[Signal], min_edge: f64) -> Vec<Signal> {
    signals
        .iter()
        .filter(|s| s.is_actionable(min_edge))
        .cloned()
        .collect()
}

/// 按 |edge| 降序对信号排序（最大 edge 排在前）。
pub fn sort_by_edge_abs(signals: &mut [Signal]) {
    signals.sort_by(|a, b| b.edge.abs().partial_cmp(&a.edge.abs()).unwrap_or(std::cmp::Ordering::Equal));
}

/// 为指定市场挑选最佳 edge。
pub fn best_for_market<'a>(signals: &'a [Signal], market_id: &str) -> Option<&'a Signal> {
    signals
        .iter()
        .filter(|s| s.market_id == market_id)
        .max_by(|a, b| a.edge.abs().partial_cmp(&b.edge.abs()).unwrap_or(std::cmp::Ordering::Equal))
}

/// 统计信息：信号切片的数量 + 平均 |edge|。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalStats {
    pub n: usize,
    pub avg_abs_edge: f64,
    pub max_abs_edge: f64,
    pub bullish: usize,  // edge > 0
    pub bearish: usize,  // edge < 0
}

pub fn stats(signals: &[Signal]) -> SignalStats {
    let n = signals.len();
    if n == 0 {
        return SignalStats { n: 0, avg_abs_edge: 0.0, max_abs_edge: 0.0, bullish: 0, bearish: 0 };
    }
    let mut sum = 0.0;
    let mut max = 0.0;
    let mut bull = 0;
    let mut bear = 0;
    for s in signals {
        let a = s.edge.abs();
        sum += a;
        if a > max { max = a; }
        if s.edge > 0.0 { bull += 1; }
        if s.edge < 0.0 { bear += 1; }
    }
    SignalStats {
        n,
        avg_abs_edge: sum / n as f64,
        max_abs_edge: max,
        bullish: bull,
        bearish: bear,
    }
}

/// 自动过期所属市场已关闭的信号。
/// 返回应标记为不活跃的信号 ID 列表。
pub fn expire_for_closed_markets(signals: &[Signal], now_ms: i64) -> Vec<String> {
    signals
        .iter()
        .filter(|s| hours_until_close(s.horizon_hours * 3_600_000 + s.computed_at, now_ms) < 0)
        .map(|s| s.market_id.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sig(market_id: &str, edge: f64, conf: f64) -> Signal {
        Signal {
            market_id: market_id.into(),
            computed_at: 0,
            model_version: "v0".into(),
            predicted_prob: 0.5 + edge,
            market_prob: 0.5,
            edge,
            confidence: conf,
            horizon_hours: 24,
            rationale: None,
        }
    }

    #[test]
    fn score_basic() {
        let (e, c) = score(0.7, 0.5);
        assert!((e - 0.2).abs() < 1e-9);
        assert!((c - 0.4).abs() < 1e-9);
    }

    #[test]
    fn score_confidence_capped() {
        let (_, c) = score(1.0, 0.0);
        assert!(c <= 0.95);
    }

    #[test]
    fn actionable_needs_both_edge_and_confidence() {
        assert!(!sig("m", 0.04, 0.9).is_actionable(0.05));
        assert!(!sig("m", 0.10, 0.5).is_actionable(0.05));
        assert!( sig("m", 0.10, 0.6).is_actionable(0.05));
    }

    #[test]
    fn filter_active_respects_threshold() {
        let s = vec![sig("a", 0.03, 0.9), sig("b", 0.10, 0.6), sig("c", -0.20, 0.7)];
        let active = filter_active(&s, 0.05);
        assert_eq!(active.len(), 2);
    }

    #[test]
    fn sort_by_edge_abs_descending() {
        let mut s = vec![sig("a", 0.05, 0.7), sig("b", -0.20, 0.8), sig("c", 0.10, 0.7)];
        sort_by_edge_abs(&mut s);
        assert_eq!(s[0].market_id, "b"); // |0.20| biggest
        assert_eq!(s[1].market_id, "c");
    }

    #[test]
    fn best_for_market_picks_max_abs() {
        let s = vec![sig("a", 0.05, 0.7), sig("a", 0.20, 0.7), sig("a", 0.10, 0.7)];
        let best = best_for_market(&s, "a").unwrap();
        assert!((best.edge - 0.20).abs() < 1e-9);
    }

    #[test]
    fn stats_aggregates() {
        let s = vec![sig("a", 0.10, 0.7), sig("b", -0.20, 0.7), sig("c", 0.30, 0.7)];
        let st = stats(&s);
        assert_eq!(st.n, 3);
        assert_eq!(st.bullish, 2);
        assert_eq!(st.bearish, 1);
        assert!((st.avg_abs_edge - 0.20).abs() < 1e-9);
        assert!((st.max_abs_edge - 0.30).abs() < 1e-9);
    }

    #[test]
    fn stats_empty() {
        let st = stats(&[]);
        assert_eq!(st.n, 0);
        assert_eq!(st.avg_abs_edge, 0.0);
    }

    #[test]
    fn expire_returns_past_due() {
        // computed_at = 0, horizon = 24h → 截止时间为 now_24h_ago
        let s = vec![sig("a", 0.05, 0.7)];
        let expired = expire_for_closed_markets(&s, 100 * 3_600_000);
        assert_eq!(expired, vec!["a".to_string()]);
    }
}
