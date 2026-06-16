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

// ============================================================
// ============== Mirror queue state machine ====================
// ============================================================

/// A pending mirror order, derived from a CopyEvent + market edge.
/// Persisted in `copy_mirror_queue` table; the L2 scheduler picks
/// up `Pending` rows and submits them as bets.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MirrorStatus {
    /// Decided by should_mirror; waiting for executor to pick up
    Pending,
    /// Submitted as a bet; awaiting on-chain fill confirmation
    Submitted,
    /// Successfully filled and recorded in `bets` (matched_bet_id set)
    Filled,
    /// Rejected by executor (insufficient balance, network error, etc.)
    Rejected,
    /// Expired — market closed before mirror could be filled
    Expired,
}

impl MirrorStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            MirrorStatus::Pending => "pending",
            MirrorStatus::Submitted => "submitted",
            MirrorStatus::Filled => "filled",
            MirrorStatus::Rejected => "rejected",
            MirrorStatus::Expired => "expired",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(MirrorStatus::Pending),
            "submitted" => Some(MirrorStatus::Submitted),
            "filled" => Some(MirrorStatus::Filled),
            "rejected" => Some(MirrorStatus::Rejected),
            "expired" => Some(MirrorStatus::Expired),
            _ => None,
        }
    }
    /// Legal: Pending → (Submitted | Rejected | Expired)
    ///        Submitted → (Filled | Rejected)
    pub fn can_transition_to(self, next: MirrorStatus) -> bool {
        match (self, next) {
            (MirrorStatus::Pending, MirrorStatus::Submitted) => true,
            (MirrorStatus::Pending, MirrorStatus::Rejected) => true,
            (MirrorStatus::Pending, MirrorStatus::Expired) => true,
            (MirrorStatus::Submitted, MirrorStatus::Filled) => true,
            (MirrorStatus::Submitted, MirrorStatus::Rejected) => true,
            _ => false,
        }
    }
}

/// Build a MirrorOrder from a decision + the underlying event.
pub fn build_mirror(
    event: &CopyEvent,
    decision: &MirrorDecision,
    created_at: i64,
) -> MirrorOrder {
    MirrorOrder {
        id: format!("mir_{}_{}", event.tx_hash, event.market_id),
        event_id: event.id,
        target_id: event.target_id.clone(),
        market_id: event.market_id.clone(),
        side: decision.side.clone(),
        size: decision.size.clone(),
        flipped: decision.flip,
        status: MirrorStatus::Pending,
        created_at,
        submitted_at: None,
        filled_at: None,
        bet_id: None,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MirrorOrder {
    pub id: String,
    pub event_id: i64,
    pub target_id: String,
    pub market_id: String,
    pub side: String,
    pub size: String,
    pub flipped: bool,
    pub status: MirrorStatus,
    pub created_at: i64,
    pub submitted_at: Option<i64>,
    pub filled_at: Option<i64>,
    pub bet_id: Option<String>,
}

/// Stats over a set of mirror orders (for the L1 mirror panel).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MirrorStats {
    pub n_total: usize,
    pub n_pending: usize,
    pub n_submitted: usize,
    pub n_filled: usize,
    pub n_rejected: usize,
    pub n_expired: usize,
}

pub fn mirror_stats(orders: &[MirrorOrder]) -> MirrorStats {
    let mut s = MirrorStats {
        n_total: orders.len(),
        n_pending: 0,
        n_submitted: 0,
        n_filled: 0,
        n_rejected: 0,
        n_expired: 0,
    };
    for o in orders {
        match o.status {
            MirrorStatus::Pending => s.n_pending += 1,
            MirrorStatus::Submitted => s.n_submitted += 1,
            MirrorStatus::Filled => s.n_filled += 1,
            MirrorStatus::Rejected => s.n_rejected += 1,
            MirrorStatus::Expired => s.n_expired += 1,
        }
    }
    s
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

    fn ev(tx: &str) -> CopyEvent {
        CopyEvent {
            id: 42,
            target_id: "t1".into(),
            market_id: "m1".into(),
            detected_at: 0,
            side: "YES".into(),
            size: "100".into(),
            price: 0.5,
            tx_hash: tx.into(),
            matched_bet_id: None,
        }
    }

    #[test]
    fn mirror_status_round_trip() {
        for s in [
            MirrorStatus::Pending,
            MirrorStatus::Submitted,
            MirrorStatus::Filled,
            MirrorStatus::Rejected,
            MirrorStatus::Expired,
        ] {
            assert_eq!(MirrorStatus::parse(s.as_str()), Some(s));
        }
        assert_eq!(MirrorStatus::parse("bogus"), None);
    }

    #[test]
    fn mirror_status_legal_transitions() {
        assert!(MirrorStatus::Pending.can_transition_to(MirrorStatus::Submitted));
        assert!(MirrorStatus::Pending.can_transition_to(MirrorStatus::Rejected));
        assert!(MirrorStatus::Pending.can_transition_to(MirrorStatus::Expired));
        assert!(MirrorStatus::Submitted.can_transition_to(MirrorStatus::Filled));
        assert!(MirrorStatus::Submitted.can_transition_to(MirrorStatus::Rejected));
        // illegal
        assert!(!MirrorStatus::Filled.can_transition_to(MirrorStatus::Pending));
        assert!(!MirrorStatus::Expired.can_transition_to(MirrorStatus::Submitted));
        assert!(!MirrorStatus::Submitted.can_transition_to(MirrorStatus::Expired));
    }

    #[test]
    fn build_mirror_uses_event_data() {
        let d = MirrorDecision { side: "NO".into(), size: "50".into(), flip: true };
        let m = build_mirror(&ev("0xdeadbeef"), &d, 1700000000000);
        assert_eq!(m.id, "mir_0xdeadbeef_m1");
        assert_eq!(m.market_id, "m1");
        assert_eq!(m.side, "NO");
        assert_eq!(m.size, "50");
        assert!(m.flipped);
        assert_eq!(m.status, MirrorStatus::Pending);
        assert!(m.submitted_at.is_none());
        assert!(m.bet_id.is_none());
    }

    #[test]
    fn mirror_stats_aggregates() {
        let mut orders = Vec::new();
        for (i, s) in [
            MirrorStatus::Pending,
            MirrorStatus::Pending,
            MirrorStatus::Submitted,
            MirrorStatus::Filled,
            MirrorStatus::Filled,
            MirrorStatus::Filled,
            MirrorStatus::Rejected,
            MirrorStatus::Expired,
        ]
        .iter()
        .enumerate()
        {
            let d = MirrorDecision { side: "YES".into(), size: "10".into(), flip: false };
            let mut m = build_mirror(&ev(&format!("0x{:x}", i)), &d, 0);
            m.status = s.clone();
            orders.push(m);
        }
        let s = mirror_stats(&orders);
        assert_eq!(s.n_total, 8);
        assert_eq!(s.n_pending, 2);
        assert_eq!(s.n_submitted, 1);
        assert_eq!(s.n_filled, 3);
        assert_eq!(s.n_rejected, 1);
        assert_eq!(s.n_expired, 1);
    }
}
