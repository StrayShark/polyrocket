//! L3 — Wallet (UI-side helpers; signing is in L5 platform::keyring).
//!
//! Wallets live in the `wallets` table. The private key is NEVER stored
//! in SQLite — it's kept in the OS keyring under a `wallet_alias`
//! (see [`crate::platform::keyring::wallet_alias`]). This module owns
//! the metadata layer: list, register, and label wallets.
//!
//! **Status (v0.3c): stub.** L2 `commands::wallet` has the working SQL
//! — it will move here as part of the M3 "Wallet management" milestone.

use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Wallet {
    pub id: String,
    pub address: String,
    pub label: Option<String>,
    pub chain_id: i64,
    pub wallet_type: String,
    pub created_at: i64,
    pub last_synced_at: Option<i64>,
}

/// Polygon mainnet is the default chain for Polymarket.
pub const POLYGON_MAINNET: i64 = 137;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn polygon_mainnet_constant() {
        assert_eq!(POLYGON_MAINNET, 137);
    }
}
