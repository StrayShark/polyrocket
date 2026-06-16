//! L3 — Copy trading.
//!
//! Watches `copy_targets` (whale addresses) for on-chain trades and
//! records matches in `copy_events`. Real impl: depends on
//! `polymarket::fetch_trades_for_address` which is not yet implemented.
//!
//! **Status (v0.3c): stub.** M7 "Copy trading" milestone.

use crate::domain::wallet::validate_address;
use crate::AppError;
use crate::AppResult;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopyTarget {
    pub id: String,
    pub address: String,
    pub label: Option<String>,
    pub enabled: bool,
    pub allocation_cap: Option<String>,
    pub min_edge: f64,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopyEvent {
    pub id: i64,
    pub target_id: String,
    pub market_id: String,
    pub detected_at: i64,
    pub side: String,
    pub size: String,
    pub price: f64,
    pub tx_hash: String,
    pub matched_bet_id: Option<String>,
}

// ============================================================
// ============== Pure helpers =================================
// ============================================================

/// Validate a copy target before insert. Re-uses the wallet address
/// validator (EVM 0x + 40 hex) since copy targets are wallet addrs.
pub fn validate_target_args(address: &str, min_edge: Option<f64>, allocation_cap: Option<&str>) -> AppResult<()> {
    validate_address(address)?;
    if let Some(e) = min_edge {
        if !(0.0..=1.0).contains(&e) {
            return Err(AppError::Invalid(format!("min_edge {e} out of [0, 1]")));
        }
    }
    if let Some(c) = allocation_cap {
        let n: f64 = c.parse().map_err(|_| AppError::Invalid(format!("allocation_cap not a number: {c}")))?;
        if n < 0.0 {
            return Err(AppError::Invalid("allocation_cap cannot be negative".into()));
        }
    }
    Ok(())
}

/// Decide whether a fill from a watched address should trigger
/// a mirror order. Returns the mirror side ("YES"/"NO") and size,
/// or None if the target is disabled or the edge is too small.
pub fn should_mirror(
    target: &CopyTarget,
    fill_side: &str,
    fill_size: &str,
    target_market_edge: f64,
) -> Option<MirrorDecision> {
    if !target.enabled {
        return None;
    }
    if target_market_edge.abs() < target.min_edge {
        return None;
    }
    let cap = target
        .allocation_cap
        .as_deref()
        .and_then(|c| c.parse::<f64>().ok())
        .unwrap_or(f64::INFINITY);
    let fill_size_n: f64 = fill_size.parse().unwrap_or(0.0);
    if fill_size_n <= 0.0 {
        return None;
    }
    let size = fill_size_n.min(cap);
    let mirror_side = if target_market_edge > 0.0 { "YES" } else { "NO" };
    Some(MirrorDecision {
        side: mirror_side.into(),
        size: size.to_string(),
        // Mirror is skipped if the whale's side disagrees with our edge
        // (i.e. the model is short while whale is long)
        flip: fill_side.to_uppercase() != mirror_side,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MirrorDecision {
    pub side: String,    // "YES" or "NO"
    pub size: String,    // decimal string
    pub flip: bool,      // true if the model disagrees with the whale's direction
}

/// Check if a `tx_hash` is already in the recent events list (dedup).
pub fn is_duplicate_tx(events: &[CopyEvent], tx_hash: &str) -> bool {
    events.iter().any(|e| e.tx_hash == tx_hash)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(enabled: bool, min_edge: f64, cap: Option<&str>) -> CopyTarget {
        CopyTarget {
            id: "t1".into(),
            address: "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045".into(),
            label: None,
            enabled,
            allocation_cap: cap.map(String::from),
            min_edge,
            created_at: 0,
        }
    }

    #[test]
    fn target_min_edge_defaults_to_zero() {
        let t = target(true, 0.0, None);
        assert!(t.enabled);
        assert_eq!(t.min_edge, 0.0);
    }

    #[test]
    fn validate_target_args_rejects_bad_address() {
        assert!(validate_target_args("not-an-address", None, None).is_err());
    }

    #[test]
    fn validate_target_args_rejects_bad_edge() {
        let addr = "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045";
        assert!(validate_target_args(addr, Some(1.5), None).is_err());
        assert!(validate_target_args(addr, Some(-0.1), None).is_err());
    }

    #[test]
    fn validate_target_args_rejects_bad_cap() {
        let addr = "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045";
        assert!(validate_target_args(addr, None, Some("not-a-number")).is_err());
        assert!(validate_target_args(addr, None, Some("-5")).is_err());
    }

    #[test]
    fn validate_target_args_ok() {
        let addr = "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045";
        assert!(validate_target_args(addr, Some(0.05), Some("100")).is_ok());
        assert!(validate_target_args(addr, None, None).is_ok());
    }

    #[test]
    fn mirror_disabled_returns_none() {
        let t = target(false, 0.05, None);
        assert!(should_mirror(&t, "YES", "100", 0.10).is_none());
    }

    #[test]
    fn mirror_edge_too_small() {
        let t = target(true, 0.05, None);
        assert!(should_mirror(&t, "YES", "100", 0.02).is_none());
    }

    #[test]
    fn mirror_respects_cap() {
        let t = target(true, 0.05, Some("50"));
        let m = should_mirror(&t, "YES", "100", 0.10).unwrap();
        assert_eq!(m.size, "50");
        assert!(!m.flip);
    }

    #[test]
    fn mirror_detects_direction_disagreement() {
        let t = target(true, 0.05, None);
        // Whale buys YES, but our edge is negative (we want NO)
        let m = should_mirror(&t, "YES", "100", -0.10).unwrap();
        assert_eq!(m.side, "NO");
        assert!(m.flip);
    }

    #[test]
    fn mirror_zero_size_returns_none() {
        let t = target(true, 0.0, None);
        assert!(should_mirror(&t, "YES", "0", 0.10).is_none());
    }

    #[test]
    fn is_duplicate_tx_detects_match() {
        let events = vec![CopyEvent {
            id: 1,
            target_id: "t1".into(),
            market_id: "m1".into(),
            detected_at: 0,
            side: "YES".into(),
            size: "100".into(),
            price: 0.5,
            tx_hash: "0xabc".into(),
            matched_bet_id: None,
        }];
        assert!(is_duplicate_tx(&events, "0xabc"));
        assert!(!is_duplicate_tx(&events, "0xdef"));
    }
}
