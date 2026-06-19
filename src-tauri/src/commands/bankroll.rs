//! L2 — Bankroll allocation IPC commands (M11, v0.78).
//!
//! v0.78b delivers the IPC surface for the bankroll allocator.
//! The pure-function algorithm lives in `crate::domain::bankroll`.
//! This file is the **thin IPC adapter** — no business logic.
//!
//! v0.78b scope:
//!   - `compute_allocation_preview` — pure compute, no DB writes
//!   - `validate_bankroll_config` — reject out-of-range values
//!
//! v0.78c+ will add DB-backed `get/set_bankroll_config` +
//! `apply_allocation` (writes to `bets` table).
//!
//! Spec: docs/bankroll-allocation-design.md §2.2.

use crate::AppResult;
use crate::domain::bankroll::{compute_allocation as compute, AllocationInput};
use crate::domain::signal::Signal;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::HashMap;

/// IPC request shape for `compute_allocation_preview`.
///
/// Mirrors `AllocationInput` but with owned data (the IPC layer can't
/// take references — Tauri deserializes into owned types).
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ComputeAllocationArgs {
    /// Total available USDC (string for DB TEXT compatibility).
    pub bankroll_usdc: String,
    /// Per-wallet config (or use default if None).
    pub config: Option<BankrollConfigDto>,
    /// Signals to consider.
    pub signals: Vec<Signal>,
    /// Optional market_id → max USDC allocatable (liquidity cap).
    pub market_liquidity: Option<HashMap<String, String>>,
}

/// IPC-friendly DTO for `BankrollConfig`. Same shape, separate type
/// so the L1↔L2 boundary stays clean (domain::BankrollConfig has no
/// `specta::Type` derive to keep it free of IPC-layer types).
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct BankrollConfigDto {
    pub kelly_multiplier: f64,
    pub max_per_signal_pct: f64,
    pub reserve_pct: f64,
    pub min_edge_pct: f64,
    pub max_total_exposure_pct: f64,
    pub min_confidence: f64,
}

impl From<&BankrollConfigDto> for crate::domain::bankroll::BankrollConfig {
    fn from(dto: &BankrollConfigDto) -> Self {
        Self {
            kelly_multiplier: dto.kelly_multiplier,
            max_per_signal_pct: dto.max_per_signal_pct,
            reserve_pct: dto.reserve_pct,
            min_edge_pct: dto.min_edge_pct,
            max_total_exposure_pct: dto.max_total_exposure_pct,
            min_confidence: dto.min_confidence,
        }
    }
}

/// Validate a `BankrollConfig`. Returns Err if any field is out of
/// range. Used by `set_bankroll_config` IPC to reject bad input
/// before persisting.
pub fn validate_config(config: &BankrollConfigDto) -> AppResult<()> {
    use crate::infra::error::AppError;
    let r = |lo, hi, name, v: f64| -> AppResult<()> {
        if !v.is_finite() || v < lo || v > hi {
            Err(AppError::Invalid(format!(
                "{} = {} out of range [{}, {}]",
                name, v, lo, hi
            )))
        } else {
            Ok(())
        }
    };
    r(0.0, 2.0, "kelly_multiplier", config.kelly_multiplier)?;
    r(0.0, 1.0, "max_per_signal_pct", config.max_per_signal_pct)?;
    r(0.0, 0.99, "reserve_pct", config.reserve_pct)?;
    r(0.0, 1.0, "min_edge_pct", config.min_edge_pct)?;
    r(0.0, 1.0, "max_total_exposure_pct", config.max_total_exposure_pct)?;
    r(0.0, 1.0, "min_confidence", config.min_confidence)?;
    // Cross-field: reserve + max_total <= 1.0
    if config.reserve_pct + config.max_total_exposure_pct > 1.0 {
        return Err(AppError::Invalid(format!(
            "reserve_pct ({}) + max_total_exposure_pct ({}) > 1.0",
            config.reserve_pct, config.max_total_exposure_pct
        )));
    }
    Ok(())
}

/// IPC: compute allocation preview (no DB writes).
///
/// This is the **deterministic, side-effect-free** compute. Used by
/// the UI to show "what would the allocation look like?" before
/// the user clicks "Apply" (which would call `apply_allocation`
/// in v0.78c+).
#[tauri::command]
#[specta::specta]
pub fn compute_allocation_preview(
    args: ComputeAllocationArgs,
) -> AppResult<crate::domain::bankroll::AllocationResult> {
    // v0.78b — domain::bankroll::BankrollConfig doesn't derive
    // specta::Type, so we take an Option<DTO> and convert. Default
    // when None.
    let config: crate::domain::bankroll::BankrollConfig = match &args.config {
        Some(dto) => {
            validate_config(dto)?;
            dto.into()
        }
        None => crate::domain::bankroll::BankrollConfig::default(),
    };
    let input = AllocationInput {
        bankroll_usdc: &args.bankroll_usdc,
        config: &config,
        signals: &args.signals,
        market_liquidity: args.market_liquidity.as_ref(),
    };
    Ok(compute(&input))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::bankroll::BankrollConfig;

    fn default_dto() -> BankrollConfigDto {
        let c: BankrollConfig = BankrollConfig::default();
        BankrollConfigDto {
            kelly_multiplier: c.kelly_multiplier,
            max_per_signal_pct: c.max_per_signal_pct,
            reserve_pct: c.reserve_pct,
            min_edge_pct: c.min_edge_pct,
            max_total_exposure_pct: c.max_total_exposure_pct,
            min_confidence: c.min_confidence,
        }
    }

    fn sig(market: &str, edge: f64, conf: f64) -> Signal {
        Signal {
            market_id: market.to_string(),
            computed_at: 1700000000000,
            model_version: "m1".to_string(),
            predicted_prob: 0.5 + edge,
            market_prob: 0.5,
            edge,
            confidence: conf,
            horizon_hours: 24,
            rationale: None,
        }
    }

    #[test]
    fn validate_config_accepts_default() {
        let dto = default_dto();
        assert!(validate_config(&dto).is_ok());
    }

    #[test]
    fn validate_config_rejects_negative_kelly() {
        let mut dto = default_dto();
        dto.kelly_multiplier = -0.1;
        assert!(validate_config(&dto).is_err());
    }

    #[test]
    fn validate_config_rejects_kelly_above_2() {
        let mut dto = default_dto();
        dto.kelly_multiplier = 5.0;
        assert!(validate_config(&dto).is_err());
    }

    #[test]
    fn validate_config_rejects_max_per_signal_above_1() {
        let mut dto = default_dto();
        dto.max_per_signal_pct = 1.5;
        assert!(validate_config(&dto).is_err());
    }

    #[test]
    fn validate_config_rejects_nan() {
        let mut dto = default_dto();
        dto.kelly_multiplier = f64::NAN;
        assert!(validate_config(&dto).is_err());
    }

    #[test]
    fn validate_config_rejects_reserve_plus_total_above_1() {
        let mut dto = default_dto();
        dto.reserve_pct = 0.5;
        dto.max_total_exposure_pct = 0.6; // 0.5 + 0.6 = 1.1 > 1.0
        assert!(validate_config(&dto).is_err());
    }

    #[test]
    fn compute_allocation_preview_with_default_config() {
        let args = ComputeAllocationArgs {
            bankroll_usdc: "1000".to_string(),
            config: None, // default
            signals: vec![sig("m1", 0.10, 0.8)],
            market_liquidity: None,
        };
        let r = compute_allocation_preview(args).unwrap();
        // 1 signal, edge 0.10, conf 0.8, market 0.5
        // Kelly: b=1, p=0.6, q=0.4, f=0.2; multiplier 0.25 → 0.05
        // raw_alloc = 0.05 * 1000 = $50
        // max_per_signal = $100 → no cap
        // reserve = $200, total = $50 ≤ $800
        assert_eq!(r.per_market.len(), 1);
        assert_eq!(r.per_market[0].size_usdc, "50.00");
        assert_eq!(r.total_allocated_usdc, "50.00");
    }

    #[test]
    fn compute_allocation_preview_rejects_invalid_config() {
        let mut dto = default_dto();
        dto.kelly_multiplier = -0.5;
        let args = ComputeAllocationArgs {
            bankroll_usdc: "1000".to_string(),
            config: Some(dto),
            signals: vec![],
            market_liquidity: None,
        };
        assert!(compute_allocation_preview(args).is_err());
    }

    #[test]
    fn compute_allocation_preview_empty_signals() {
        let args = ComputeAllocationArgs {
            bankroll_usdc: "1000".to_string(),
            config: None,
            signals: vec![],
            market_liquidity: None,
        };
        let r = compute_allocation_preview(args).unwrap();
        assert_eq!(r.total_allocated_usdc, "0.00");
        assert_eq!(r.reserved_usdc, "200.00");
    }

    #[test]
    fn from_dto_preserves_all_fields() {
        let dto = BankrollConfigDto {
            kelly_multiplier: 0.5,
            max_per_signal_pct: 0.15,
            reserve_pct: 0.10,
            min_edge_pct: 0.03,
            max_total_exposure_pct: 0.85,
            min_confidence: 0.7,
        };
        let c: BankrollConfig = (&dto).into();
        assert_eq!(c.kelly_multiplier, 0.5);
        assert_eq!(c.max_per_signal_pct, 0.15);
        assert_eq!(c.reserve_pct, 0.10);
        assert_eq!(c.min_edge_pct, 0.03);
        assert_eq!(c.max_total_exposure_pct, 0.85);
        assert_eq!(c.min_confidence, 0.7);
    }
}
