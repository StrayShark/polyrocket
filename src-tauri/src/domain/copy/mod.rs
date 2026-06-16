//! L3 — Copy trading.
//!
//! Watches `copy_targets` (whale addresses) for on-chain trades and
//! records matches in `copy_events`. Real impl: depends on
//! `polymarket::fetch_trades_for_address` which is not yet implemented.
//!
//! **Status (v0.3c): stub.** M7 "Copy trading" milestone.

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_min_edge_defaults_to_zero() {
        let t = CopyTarget {
            id: "t1".into(),
            address: "0xabc".into(),
            label: None,
            enabled: true,
            allocation_cap: None,
            min_edge: 0.0,
            created_at: 0,
        };
        assert!(t.enabled);
        assert_eq!(t.min_edge, 0.0);
    }
}
