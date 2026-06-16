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
}
