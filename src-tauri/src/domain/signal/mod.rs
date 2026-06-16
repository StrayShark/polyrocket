//! L3 — Signal.
//!
//! Computes model-vs-market probability edges per market. Input is a
//! market + a model's predicted probability; output is a [`Signal`]
//! with `edge`, `confidence`, and a rationale string.
//!
//! **Status (v0.3c): stub.** M5 "Signal detection" milestone is the
//! active work — see overview.md §3.2. For now, commands surface the
//! raw `signals` table through L2 with no domain logic.

use serde::{Deserialize, Serialize};

/// Computed signal for a single market.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Signal {
    pub market_id: String,
    pub computed_at: i64,
    pub model_version: String,
    pub predicted_prob: f64,
    pub market_prob: f64,
    pub edge: f64, // predicted_prob - market_prob
    pub confidence: f64, // 0..1
    pub horizon_hours: i64,
    pub rationale: Option<String>,
}

impl Signal {
    /// `true` if the edge is large enough to be actionable.
    pub fn is_actionable(&self, min_edge: f64) -> bool {
        self.edge.abs() >= min_edge && self.confidence >= 0.6
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(edge: f64, conf: f64) -> Signal {
        Signal {
            market_id: "m1".into(),
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
    fn actionable_needs_both_edge_and_confidence() {
        assert!(!s(0.04, 0.9).is_actionable(0.05)); // edge too small
        assert!(!s(0.10, 0.5).is_actionable(0.05)); // confidence too low
        assert!( s(0.10, 0.6).is_actionable(0.05));
        assert!(!s(-0.10, 0.9).is_actionable(0.20)); // |edge| 0.10 < 0.20
    }
}
