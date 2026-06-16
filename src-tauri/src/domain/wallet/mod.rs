//! L3 — Wallet (UI-side helpers; signing is in L5 platform::keyring).
//!
//! Wallets live in the `wallets` table. The private key is NEVER stored
//! in SQLite — it's kept in the OS keyring under a `wallet_alias`
//! (see [`crate::platform::keyring::wallet_alias`]). This module owns
//! the metadata layer: list, register, and label wallets.
//!
//! **Status (v0.3c): stub.** L2 `commands::wallet` has the working SQL
//! — it will move here as part of the M3 "Wallet management" milestone.

use crate::AppError;
use crate::AppResult;
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

/// All chain IDs the app currently supports.
pub const SUPPORTED_CHAINS: &[i64] = &[137, 80002]; // Polygon mainnet + Amoy testnet

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WalletType {
    Eoa,
    Smart,
}

impl WalletType {
    pub fn as_str(self) -> &'static str {
        match self {
            WalletType::Eoa => "eoa",
            WalletType::Smart => "smart",
        }
    }
    pub fn parse(s: &str) -> AppResult<Self> {
        match s {
            "eoa" => Ok(WalletType::Eoa),
            "smart" => Ok(WalletType::Smart),
            _ => Err(AppError::Invalid(format!("unknown wallet type: {s}"))),
        }
    }
}

// ============================================================
// ============== Validation (pure) ===========================
// ============================================================

/// Validate an EVM address (0x + 40 hex chars).
/// Returns `Err(AppError::Invalid)` if it doesn't match.
pub fn validate_address(addr: &str) -> AppResult<()> {
    if !addr.starts_with("0x") {
        return Err(AppError::Invalid("address must start with 0x".into()));
    }
    if addr.len() != 42 {
        return Err(AppError::Invalid(format!(
            "address must be 42 chars (got {})",
            addr.len()
        )));
    }
    if !addr[2..].chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(AppError::Invalid("address has non-hex chars".into()));
    }
    Ok(())
}

pub fn validate_chain(chain_id: i64) -> AppResult<()> {
    if SUPPORTED_CHAINS.contains(&chain_id) {
        Ok(())
    } else {
        Err(AppError::Invalid(format!(
            "unsupported chain_id: {chain_id}; supported: {SUPPORTED_CHAINS:?}"
        )))
    }
}

pub fn validate_label(label: Option<&str>) -> AppResult<()> {
    if let Some(l) = label {
        if l.is_empty() {
            return Err(AppError::Invalid("label cannot be empty".into()));
        }
        if l.len() > 64 {
            return Err(AppError::Invalid("label > 64 chars".into()));
        }
    }
    Ok(())
}

/// Format an address for display: 0x1234…abcd
pub fn short_address(addr: &str) -> String {
    if addr.len() < 10 {
        return addr.to_string();
    }
    format!("{}…{}", &addr[..6], &addr[addr.len() - 4..])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn polygon_mainnet_constant() {
        assert_eq!(POLYGON_MAINNET, 137);
    }

    #[test]
    fn wallet_type_round_trip() {
        for t in [WalletType::Eoa, WalletType::Smart] {
            assert_eq!(WalletType::parse(t.as_str()).unwrap(), t);
        }
    }

    #[test]
    fn wallet_type_rejects_unknown() {
        assert!(WalletType::parse("multisig").is_err());
    }

    #[test]
    fn validate_address_valid() {
        // Vitalik's address (well-known)
        let vitalik = "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045";
        assert!(validate_address(vitalik).is_ok());
    }

    #[test]
    fn validate_address_no_prefix() {
        assert!(validate_address("d8dA6BF26964aF9D7eEd9e03E53415D37aA96045").is_err());
    }

    #[test]
    fn validate_address_wrong_length() {
        assert!(validate_address("0x1234").is_err());
    }

    #[test]
    fn validate_address_non_hex() {
        assert!(validate_address("0xZZZZ6BF26964aF9D7eEd9e03E53415D37aA96045").is_err());
    }

    #[test]
    fn validate_chain_accepts_mainnet() {
        assert!(validate_chain(137).is_ok());
    }

    #[test]
    fn validate_chain_rejects_unknown() {
        assert!(validate_chain(1).is_err()); // Ethereum mainnet — not supported
    }

    #[test]
    fn validate_label_ok() {
        assert!(validate_label(Some("primary")).is_ok());
        assert!(validate_label(None).is_ok());
    }

    #[test]
    fn validate_label_rejects_empty() {
        assert!(validate_label(Some("")).is_err());
    }

    #[test]
    fn validate_label_rejects_too_long() {
        let long = "x".repeat(65);
        assert!(validate_label(Some(&long)).is_err());
    }

    #[test]
    fn short_address_basic() {
        let s = short_address("0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045");
        assert_eq!(s, "0xd8dA…6045");
    }

    #[test]
    fn short_address_short_input() {
        let s = short_address("0x123");
        assert_eq!(s, "0x123");
    }
}
