//! L3 — Polymarket CLOB v2 client (read + signed order stubs).
//! Spec: polyradar-blueprint-v2-client.md §4
//!
//! MVP scope: read-only market data + signed order submission (mode B).
//! Real `py-clob-client` parity is intentionally avoided in favor of a minimal
//! direct-Rust implementation to keep the binary small and self-contained.
//!
//! Layer rules: this module may use L4 (`infra::http::new_http_client`)
//! but not L2 / L1. L2 `commands::market` and `commands::bet` consume
//! the public functions below.

use serde::{Deserialize, Serialize};

const CLOB_BASE: &str = "https://clob.polymarket.com";
const GAMMA_BASE: &str = "https://gamma-api.polymarket.com";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketSummary {
    pub id: String,
    pub slug: String,
    pub question: String,
    pub category: String,
    pub end_date: i64,
    pub active: bool,
    pub resolved: bool,
    pub outcome: Option<String>,
    pub liquidity: Option<String>,
    pub volume_24h: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBookSnapshot {
    pub market_id: String,
    pub captured_at: i64,
    pub best_bid: f64,
    pub best_ask: f64,
    pub mid_price: f64,
    pub spread: f64,
}

/// Fetch active markets (cached; results stored locally in `markets` table).
pub async fn fetch_active_markets() -> crate::AppResult<Vec<MarketSummary>> {
    let url = format!("{}/markets?active=true&limit=500", GAMMA_BASE);
    let resp = reqwest::Client::new()
        .get(&url)
        .send()
        .await?
        .json::<Vec<MarketSummary>>()
        .await?;
    Ok(resp)
}

/// Build the Jump-to-Polymarket URL for mode A (zero compliance risk).
/// The user clicks → opens Polymarket's UI in their default browser, signs there.
pub fn build_jump_url(market_slug: &str, side: &str, price: f64) -> String {
    let side = side.to_uppercase();
    format!(
        "https://polymarket.com/event/{}?side={}&price={:.4}",
        market_slug, side, price
    )
}

/// Place a signed order (mode B). Requires a wallet whose private key is
/// stored in the OS keyring. Implementation will use the official
/// `rs-clob-client` once added as a dependency.
pub async fn place_signed_order(
    _market_id: &str,
    _side: &str,
    _price: f64,
    _size: &str,
    _key_alias: &str,
) -> crate::AppResult<String> {
    let _ = CLOB_BASE;
    Err(crate::AppError::Internal(
        "signed-order placement not yet wired (placeholder for mode B)".into(),
    ))
}