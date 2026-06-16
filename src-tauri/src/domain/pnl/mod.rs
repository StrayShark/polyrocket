//! L3 — PnL / dashboard KPIs.
//!
//! Aggregates `bets`, `model_performance`, and `signals` rows into
//! dashboard tiles: total equity, open PnL, 30d win rate, Brier score.
//!
//! **Status (v0.3c): stub.** Real implementation lives in
//! `commands::pnl::dashboard_kpis` (thin SQL). v0.3+ will lift it
//! here as part of the M8 "PnL dashboard" milestone.

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct DashboardKpis {
    pub total_equity_usdc: String,
    pub open_pnl_usdc: String,
    pub win_rate_30d: f64,
    pub brier_score: f64,
    pub active_signals: i64,
    pub open_positions: i64,
}

// ============================================================
// ============== Pure aggregations ===========================
// ============================================================

/// Win rate = won / (won + lost). Returns 0.0 if no resolved bets.
pub fn win_rate(won: usize, lost: usize) -> f64 {
    let total = won + lost;
    if total == 0 {
        0.0
    } else {
        won as f64 / total as f64
    }
}

/// Brier score = mean over (predicted - actual)^2.
/// `actuals` are 0.0 or 1.0; `predicted` ∈ [0, 1].
/// Returns None if lists are empty or lengths differ.
pub fn brier_score(predicted: &[f64], actuals: &[f64]) -> Option<f64> {
    if predicted.is_empty() || predicted.len() != actuals.len() {
        return None;
    }
    let sum: f64 = predicted
        .iter()
        .zip(actuals.iter())
        .map(|(p, a)| (p - a).powi(2))
        .sum();
    Some(sum / predicted.len() as f64)
}

/// Realized PnL: sum of all pnl values. Strings parsed as f64, None → 0.
pub fn realized_pnl(pnls: &[Option<String>]) -> f64 {
    pnls.iter()
        .map(|p| p.as_deref().and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0))
        .sum()
}

/// Categorize a bet's PnL into win / loss / open.
pub fn categorize(pnl: Option<&str>, status: &str) -> BetCategory {
    if status == "open" {
        return BetCategory::Open;
    }
    match pnl.and_then(|s| s.parse::<f64>().ok()) {
        Some(n) if n > 0.0 => BetCategory::Win,
        Some(_) => BetCategory::Loss,
        None => BetCategory::Other, // cancelled, etc.
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BetCategory {
    Win,
    Loss,
    Open,
    Other,
}

impl BetCategory {
    pub fn as_str(self) -> &'static str {
        match self {
            BetCategory::Win => "win",
            BetCategory::Loss => "loss",
            BetCategory::Open => "open",
            BetCategory::Other => "other",
        }
    }
}

/// Aggregate stats: win_rate, total_pnl, brier (if data given).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PnLSummary {
    pub n_total: usize,
    pub n_won: usize,
    pub n_lost: usize,
    pub n_open: usize,
    pub win_rate: f64,
    pub total_pnl: f64,
    pub avg_win: f64,
    pub avg_loss: f64,
}

pub fn summarize(pnls: &[(Option<String>, String)]) -> PnLSummary {
    let mut n_won = 0;
    let mut n_lost = 0;
    let mut n_open = 0;
    let mut sum_win = 0.0;
    let mut sum_loss = 0.0;
    let mut total = 0.0;
    for (pnl, status) in pnls {
        let n = pnl.as_deref().and_then(|s| s.parse::<f64>().ok());
        let cat = categorize(pnl.as_deref(), status);
        match cat {
            BetCategory::Win => {
                n_won += 1;
                if let Some(v) = n {
                    sum_win += v;
                    total += v;
                }
            }
            BetCategory::Loss => {
                n_lost += 1;
                if let Some(v) = n {
                    sum_loss += v;
                    total += v;
                }
            }
            BetCategory::Open => n_open += 1,
            BetCategory::Other => {}
        }
    }
    PnLSummary {
        n_total: pnls.len(),
        n_won,
        n_lost,
        n_open,
        win_rate: win_rate(n_won, n_lost),
        total_pnl: total,
        avg_win: if n_won > 0 { sum_win / n_won as f64 } else { 0.0 },
        avg_loss: if n_lost > 0 { sum_loss / n_lost as f64 } else { 0.0 },
    }
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

    #[test]
    fn win_rate_basic() {
        assert!((win_rate(7, 3) - 0.7).abs() < 1e-9);
    }

    #[test]
    fn win_rate_no_data() {
        assert_eq!(win_rate(0, 0), 0.0);
    }

    #[test]
    fn brier_score_perfect() {
        let p = vec![1.0, 0.0, 1.0, 0.0];
        let a = vec![1.0, 0.0, 1.0, 0.0];
        assert_eq!(brier_score(&p, &a), Some(0.0));
    }

    #[test]
    fn brier_score_worst() {
        let p = vec![1.0, 0.0];
        let a = vec![0.0, 1.0];
        assert_eq!(brier_score(&p, &a), Some(1.0));
    }

    #[test]
    fn brier_score_empty() {
        assert_eq!(brier_score(&[], &[]), None);
    }

    #[test]
    fn brier_score_mismatched_lengths() {
        assert_eq!(brier_score(&[0.5], &[0.5, 1.0]), None);
    }

    #[test]
    fn realized_pnl_handles_none_and_strings() {
        let pnls = vec![
            Some("100".to_string()),
            Some("-50".to_string()),
            None,
            Some("abc".to_string()), // unparseable
        ];
        assert!((realized_pnl(&pnls) - 50.0).abs() < 1e-9);
    }

    #[test]
    fn categorize_status_open() {
        assert_eq!(categorize(Some("100"), "open"), BetCategory::Open);
        assert_eq!(categorize(None, "open"), BetCategory::Open);
    }

    #[test]
    fn categorize_win_loss_other() {
        assert_eq!(categorize(Some("100"), "won"), BetCategory::Win);
        assert_eq!(categorize(Some("-50"), "lost"), BetCategory::Loss);
        assert_eq!(categorize(None, "cancelled"), BetCategory::Other);
    }

    #[test]
    fn summarize_basic() {
        let pnls = vec![
            (Some("100".to_string()), "won".to_string()),
            (Some("-40".to_string()), "lost".to_string()),
            (Some("60".to_string()), "won".to_string()),
            (None, "open".to_string()),
        ];
        let s = summarize(&pnls);
        assert_eq!(s.n_total, 4);
        assert_eq!(s.n_won, 2);
        assert_eq!(s.n_lost, 1);
        assert_eq!(s.n_open, 1);
        assert!((s.win_rate - 2.0 / 3.0).abs() < 1e-9);
        assert!((s.total_pnl - 120.0).abs() < 1e-9);
        assert!((s.avg_win - 80.0).abs() < 1e-9);
        assert!((s.avg_loss - -40.0).abs() < 1e-9);
    }

    #[test]
    fn summarize_empty() {
        let s = summarize(&[]);
        assert_eq!(s.n_total, 0);
        assert_eq!(s.win_rate, 0.0);
        assert_eq!(s.total_pnl, 0.0);
    }
}
