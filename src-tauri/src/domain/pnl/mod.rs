//! L3 — PnL / dashboard KPIs.
//!
//! Aggregates `bets`, `model_performance`, and `signals` rows into
//! dashboard tiles: total equity, open PnL, 30d win rate, Brier score.
//!
//! **Status (v0.3c): stub.** Real implementation lives in
//! `commands::pnl::dashboard_kpis` (thin SQL). v0.3+ will lift it
//! here as part of the M8 "PnL dashboard" milestone.

use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct DashboardKpis {
    pub total_equity_usdc: String,
    pub open_pnl_usdc: String,
    pub win_rate_30d: f64,
    pub brier_score: f64,
    pub active_signals: i64,
    pub open_positions: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kpis_default_to_zero() {
        let k = DashboardKpis {
            total_equity_usdc: "0".into(),
            open_pnl_usdc: "0".into(),
            win_rate_30d: 0.0,
            brier_score: 0.0,
            active_signals: 0,
            open_positions: 0,
        };
        assert_eq!(k.win_rate_30d, 0.0);
        assert_eq!(k.active_signals, 0);
    }
}
