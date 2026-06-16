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

// ============================================================
// ============== Args validation ==============================
// ============================================================

/// Maximum size in USDC (defensive cap to prevent typos like
/// "10000" instead of "100").
pub const MAX_BET_SIZE_USDC: f64 = 10_000.0;
/// Minimum price (1¢) and maximum (99¢) — anything outside is invalid
/// for a real CLOB market.
pub const MIN_PRICE: f64 = 0.01;
pub const MAX_PRICE: f64 = 0.99;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaceArgs {
    pub market_id: String,
    pub side: BetSide,
    pub size_usdc: String,
    pub price: f64,
    pub key_alias: Option<String>,
}

/// Validate a `place_*` args payload. Returns parsed size on success.
pub fn validate_place_args(args: &PlaceArgs) -> AppResult<f64> {
    if args.market_id.trim().is_empty() {
        return Err(AppError::Invalid("market_id is empty".into()));
    }
    if args.market_id.len() > 128 {
        return Err(AppError::Invalid("market_id > 128 chars".into()));
    }
    if !(args.price.is_finite() && (MIN_PRICE..=MAX_PRICE).contains(&args.price)) {
        return Err(AppError::Invalid(format!(
            "price {} out of [{}, {}]",
            args.price, MIN_PRICE, MAX_PRICE
        )));
    }
    let size: f64 = args.size_usdc.parse().map_err(|_| {
        AppError::Invalid(format!("size_usdc not a number: {}", args.size_usdc))
    })?;
    if !size.is_finite() || size <= 0.0 {
        return Err(AppError::Invalid(format!("size_usdc must be > 0 (got {size})")));
    }
    if size > MAX_BET_SIZE_USDC {
        return Err(AppError::Invalid(format!(
            "size_usdc {size} > max {MAX_BET_SIZE_USDC}"
        )));
    }
    Ok(size)
}

// ============================================================
// ============== Mode B signed-order simulation =================
// ============================================================

/// Outcome of the mode B signing path. Even without a real CLOB SDK,
/// we deterministically derive a fake `tx_hash` from the args so the
/// audit log and `bets.tx_hash` column are populated.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedOrderResult {
    pub tx_hash: String,
    pub signed_at_ms: i64,
    pub shares: String,
    /// Always "submitted" — we never get on-chain confirmation in v0.5.
    pub status: &'static str,
}

/// "Sign" a mode B order. Deterministic stub for v0.5 — when
/// `rs-clob-client` lands, replace the body of this function.
pub fn sign_order(args: &PlaceArgs, now_ms: i64) -> AppResult<SignedOrderResult> {
    let size = validate_place_args(args)?;
    if args.key_alias.as_deref().unwrap_or("").is_empty() {
        return Err(AppError::Invalid("key_alias required for mode B".into()));
    }
    // Deterministic pseudo-tx-hash from a hash of the canonical args.
    // We use a simple djb2 hash → hex. (Not cryptographic; just stable.)
    let canonical = format!(
        "{}|{}|{}|{}|{}",
        args.market_id,
        args.side.as_str(),
        args.size_usdc,
        args.price,
        args.key_alias.as_deref().unwrap_or(""),
    );
    let h = djb2(canonical.as_bytes());
    let tx_hash = format!("0x{:016x}{:016x}{:016x}{:016x}", h, h.rotate_left(13), h.rotate_left(26), h);
    let shares = shares_for_size(&args.size_usdc, args.price);
    Ok(SignedOrderResult {
        tx_hash,
        signed_at_ms: now_ms,
        shares,
        status: "submitted",
    })
}

fn djb2(bytes: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for b in bytes {
        h = h.wrapping_mul(33).wrapping_add(*b as u64);
    }
    h
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

    fn args(market: &str, size: &str, price: f64, key: Option<&str>) -> PlaceArgs {
        PlaceArgs {
            market_id: market.into(),
            side: BetSide::Yes,
            size_usdc: size.into(),
            price,
            key_alias: key.map(String::from),
        }
    }

    #[test]
    fn validate_place_args_ok() {
        let r = validate_place_args(&args("m1", "100", 0.5, Some("primary")));
        assert!(r.is_ok());
        assert!((r.unwrap() - 100.0).abs() < 1e-9);
    }

    #[test]
    fn validate_place_args_rejects_empty_market() {
        assert!(validate_place_args(&args("", "100", 0.5, Some("k"))).is_err());
    }

    #[test]
    fn validate_place_args_rejects_bad_price() {
        assert!(validate_place_args(&args("m", "100", 1.5, Some("k"))).is_err());
        assert!(validate_place_args(&args("m", "100", 0.0, Some("k"))).is_err());
        assert!(validate_place_args(&args("m", "100", f64::NAN, Some("k"))).is_err());
    }

    #[test]
    fn validate_place_args_rejects_bad_size() {
        assert!(validate_place_args(&args("m", "abc", 0.5, Some("k"))).is_err());
        assert!(validate_place_args(&args("m", "-5", 0.5, Some("k"))).is_err());
        assert!(validate_place_args(&args("m", "0", 0.5, Some("k"))).is_err());
    }

    #[test]
    fn validate_place_args_caps_max_size() {
        assert!(validate_place_args(&args("m", "50000", 0.5, Some("k"))).is_err());
    }

    #[test]
    fn sign_order_requires_key_alias() {
        let r = sign_order(&args("m1", "100", 0.5, None), 1_000_000);
        assert!(r.is_err());
    }

    #[test]
    fn sign_order_produces_stable_tx_hash() {
        let a = sign_order(&args("m1", "100", 0.5, Some("primary")), 1_000_000).unwrap();
        let b = sign_order(&args("m1", "100", 0.5, Some("primary")), 1_000_000).unwrap();
        assert_eq!(a.tx_hash, b.tx_hash);
        assert!(a.tx_hash.starts_with("0x"));
        assert_eq!(a.status, "submitted");
    }

    #[test]
    fn sign_order_different_args_produce_different_hash() {
        let a = sign_order(&args("m1", "100", 0.5, Some("primary")), 1_000_000).unwrap();
        let b = sign_order(&args("m2", "100", 0.5, Some("primary")), 1_000_000).unwrap();
        assert_ne!(a.tx_hash, b.tx_hash);
    }

    #[test]
    fn sign_order_computes_shares() {
        let r = sign_order(&args("m1", "100", 0.4, Some("k")), 0).unwrap();
        // shares = 100 / 0.4 = 250
        assert_eq!(r.shares, "250");
    }
}
