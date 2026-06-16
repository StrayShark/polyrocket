//! L3 — Bet lifecycle.
//!
//! Owns the state machine for `bets` rows: `open` → `won` / `lost` /
//! `cancelled`. Provides a single `place()` entry that the L2 command
//! layer can call; signed order placement (mode B) and jump-link
//! construction (mode A) live here.
//!
//! **Status (v0.3c): stub.** The current `commands::bet` module has
//! the working SQL — it will be migrated into the helpers below as
//! M6 "Bet placement" milestone progresses.

use crate::AppError;
use crate::AppResult;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BetMode {
    /// Mode A: jump-link only. User signs on Polymarket UI.
    AJump,
    /// Mode B: signed order via OS keyring.
    BSigned,
}

impl BetMode {
    pub fn as_str(self) -> &'static str {
        match self {
            BetMode::AJump => "A_jump",
            BetMode::BSigned => "B_signed",
        }
    }
    pub fn parse(s: &str) -> AppResult<Self> {
        match s {
            "A_jump" => Ok(BetMode::AJump),
            "B_signed" => Ok(BetMode::BSigned),
            _ => Err(AppError::Invalid(format!("unknown bet mode: {s}"))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BetStatus {
    Open,
    Won,
    Lost,
    Cancelled,
}

impl BetStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            BetStatus::Open => "open",
            BetStatus::Won => "won",
            BetStatus::Lost => "lost",
            BetStatus::Cancelled => "cancelled",
        }
    }
    pub fn parse(s: &str) -> AppResult<Self> {
        match s {
            "open" => Ok(BetStatus::Open),
            "won" => Ok(BetStatus::Won),
            "lost" => Ok(BetStatus::Lost),
            "cancelled" => Ok(BetStatus::Cancelled),
            _ => Err(AppError::Invalid(format!("unknown bet status: {s}"))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BetSide {
    Yes,
    No,
}

impl BetSide {
    pub fn as_str(self) -> &'static str {
        match self {
            BetSide::Yes => "YES",
            BetSide::No => "NO",
        }
    }
    pub fn parse(s: &str) -> AppResult<Self> {
        match s.to_uppercase().as_str() {
            "YES" => Ok(BetSide::Yes),
            "NO" => Ok(BetSide::No),
            _ => Err(AppError::Invalid(format!("unknown side: {s}"))),
        }
    }
}

// ============================================================
// ============== PnL math (pure) ==============================
// ============================================================

/// Number of shares bought = USDC size / price.
/// Returns 0 if size or price is non-positive or unparseable.
pub fn shares_for_size(size_usdc: &str, price: f64) -> String {
    match size_usdc.parse::<f64>() {
        Ok(s) if s > 0.0 && price > 0.0 => (s / price).to_string(),
        _ => "0".to_string(),
    }
}

/// PnL when the market resolves YES.
/// For a YES bet: pnl = shares * (1 - price)  (you bought at `price`,
/// the share pays $1 on YES).
/// For a NO bet: pnl = -size  (NO share worthless).
pub fn pnl_on_yes(side: BetSide, shares: f64, price: f64, size_usdc: f64) -> f64 {
    match side {
        BetSide::Yes => shares * (1.0 - price),
        BetSide::No => -size_usdc,
    }
}

/// PnL when the market resolves NO.
/// For a YES bet: pnl = -size.
/// For a NO bet: pnl = shares * price  (NO share pays $1).
pub fn pnl_on_no(side: BetSide, shares: f64, price: f64, size_usdc: f64) -> f64 {
    match side {
        BetSide::Yes => -size_usdc,
        BetSide::No => shares * price,
    }
}

/// Compute PnL given the resolved outcome.
pub fn pnl(outcome: BetSide, side: BetSide, shares: f64, price: f64, size_usdc: f64) -> f64 {
    match outcome {
        BetSide::Yes => pnl_on_yes(side, shares, price, size_usdc),
        BetSide::No => pnl_on_no(side, shares, price, size_usdc),
    }
}

/// Whether a bet in this status is still "active" (excluded from PnL).
pub fn is_open(s: BetStatus) -> bool {
    matches!(s, BetStatus::Open)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_round_trip() {
        for m in [BetMode::AJump, BetMode::BSigned] {
            assert_eq!(BetMode::parse(m.as_str()).unwrap(), m);
        }
    }

    #[test]
    fn status_round_trip() {
        for s in [BetStatus::Open, BetStatus::Won, BetStatus::Lost, BetStatus::Cancelled] {
            assert_eq!(BetStatus::parse(s.as_str()).unwrap(), s);
        }
    }

    #[test]
    fn mode_rejects_unknown() {
        assert!(BetMode::parse("C_signed").is_err());
    }

    #[test]
    fn side_round_trip_case_insensitive() {
        assert_eq!(BetSide::parse("yes").unwrap(), BetSide::Yes);
        assert_eq!(BetSide::parse("NO").unwrap(), BetSide::No);
        assert!(BetSide::parse("maybe").is_err());
    }

    #[test]
    fn shares_for_size_basic() {
        let s = shares_for_size("100", 0.5);
        assert_eq!(s, "200");
    }

    #[test]
    fn shares_for_size_zero_price() {
        assert_eq!(shares_for_size("100", 0.0), "0");
    }

    #[test]
    fn pnl_yes_bet_wins() {
        // Buy 100 YES at 0.40 → if resolves YES: pnl = (1-0.40) * 250 = 150
        let p = pnl_on_yes(BetSide::Yes, 250.0, 0.40, 100.0);
        assert!((p - 150.0).abs() < 1e-6);
    }

    #[test]
    fn pnl_yes_bet_loses() {
        // If YES resolves NO, full size is lost
        let p = pnl_on_no(BetSide::Yes, 250.0, 0.40, 100.0);
        assert!((p - -100.0).abs() < 1e-6);
    }

    #[test]
    fn pnl_no_bet_wins() {
        // Buy 100 NO at 0.60 → if resolves NO: pnl = 100/0.60 * 0.60 = 100
        let shares = 100.0 / 0.60;
        let p = pnl_on_no(BetSide::No, shares, 0.60, 100.0);
        assert!((p - 100.0).abs() < 1e-6);
    }

    #[test]
    fn is_open_only_for_open_status() {
        assert!(is_open(BetStatus::Open));
        assert!(!is_open(BetStatus::Won));
        assert!(!is_open(BetStatus::Lost));
        assert!(!is_open(BetStatus::Cancelled));
    }
}
