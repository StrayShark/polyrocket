//! L3 — Markets domain (helpers used by L2 commands).
//!
//! Holds the typed [`Market`] DTO + filter / sort / classify helpers
//! that BOTH the L2 `commands::market` handler and the L1 `Markets`
//! route can share. No DB access here — that lives in L2 / L4.
//!
//! See docs/overview.md §1.2 — L3 is pure functions, no IO.

use serde::{Deserialize, Serialize};

/// All known market categories on Polymarket.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Category {
    Football,
    Cs2,
    Politics,
    Crypto,
    Tech,
    Science,
    PopCulture,
    Business,
    Other,
}

impl Category {
    pub fn as_str(self) -> &'static str {
        match self {
            Category::Football => "football",
            Category::Cs2 => "cs2",
            Category::Politics => "politics",
            Category::Crypto => "crypto",
            Category::Tech => "tech",
            Category::Science => "science",
            Category::PopCulture => "pop-culture",
            Category::Business => "business",
            Category::Other => "other",
        }
    }
    /// Best-effort classify a free-form question string into a category.
    /// Lowercased substring match. Order matters — first match wins.
    pub fn classify(question: &str) -> Self {
        let q = question.to_lowercase();
        if q.contains("fc ") || q.contains(" vs ") || q.contains("football") || q.contains("nba") || q.contains("nfl") || q.contains("premier league") {
            Category::Football
        } else if q.contains("cs2") || q.contains("counter-strike") || q.contains("esl") || q.contains("blast") {
            Category::Cs2
        } else if q.contains("trump") || q.contains("biden") || q.contains("election") || q.contains("president") || q.contains("senate") || q.contains("congress") {
            Category::Politics
        } else if q.contains("bitcoin") || q.contains("btc") || q.contains("eth") || q.contains("crypto") {
            Category::Crypto
        } else if q.contains("ai ") || q.contains("openai") || q.contains("gpt") || q.contains("anthropic") || q.contains("llm") {
            Category::Tech
        } else if q.contains("nasa") || q.contains("spacex") || q.contains("research") {
            Category::Science
        } else if q.contains("movie") || q.contains("oscar") || q.contains("grammy") || q.contains("celebrity") {
            Category::PopCulture
        } else if q.contains("stock") || q.contains("market cap") || q.contains("ipo") || q.contains("revenue") {
            Category::Business
        } else {
            Category::Other
        }
    }
}

/// Parsed numeric helpers for the string-encoded fields.
pub fn parse_liquidity(s: Option<&str>) -> f64 {
    s.and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0)
}

pub fn parse_volume_24h(s: Option<&str>) -> f64 {
    s.and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0)
}

/// Hours until close (negative = already closed).
pub fn hours_until_close(end_date_ms: i64, now_ms: i64) -> i64 {
    (end_date_ms - now_ms) / 3_600_000
}

/// Returns true if the market closes within the next `horizon_hours`.
pub fn closes_within(market_end_ms: i64, now_ms: i64, horizon_hours: i64) -> bool {
    let dt = market_end_ms - now_ms;
    dt > 0 && dt <= horizon_hours * 3_600_000
}

/// Bucket the remaining time into human labels.
pub fn closing_bucket(end_date_ms: i64, now_ms: i64) -> &'static str {
    let h = hours_until_close(end_date_ms, now_ms);
    if h < 0 { "closed" }
    else if h < 1 { "< 1h" }
    else if h < 24 { "today" }
    else if h < 24 * 7 { "this week" }
    else if h < 24 * 30 { "this month" }
    else { "later" }
}

// ---------------------------------------------------------------- I/O types
// (the original M1 wire types — preserved from the v0.2 module)

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

/// Fetch active markets from Polymarket Gamma API.
/// Layer rules: this function uses L4 `infra::http` for the shared
/// reqwest::Client (downward dependency, allowed).
pub async fn fetch_active_markets() -> crate::AppResult<Vec<MarketSummary>> {
    use crate::infra::http::new_http_client;
    let url = format!("{}/markets?active=true&limit=500", GAMMA_BASE);
    let resp = new_http_client()
        .get(&url)
        .send()
        .await?
        .json::<Vec<MarketSummary>>()
        .await?;
    Ok(resp)
}

/// Build the Jump-to-Polymarket URL for mode A (zero compliance risk).
pub fn build_jump_url(market_slug: &str, side: &str, price: f64) -> String {
    let side = side.to_uppercase();
    format!(
        "https://polymarket.com/event/{}?side={}&price={:.4}",
        market_slug, side, price
    )
}

/// Place a signed order (mode B). Stub — full impl needs `rs-clob-client`.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn category_classify() {
        assert_eq!(Category::classify("Will FC Barcelona win La Liga?"), Category::Football);
        assert_eq!(Category::classify("Trump 2024 election"), Category::Politics);
        assert_eq!(Category::classify("BTC > 100k by EOY?"), Category::Crypto);
        assert_eq!(Category::classify("Will OpenAI release GPT-5?"), Category::Tech);
        assert_eq!(Category::classify("Unknown thing"), Category::Other);
    }

    #[test]
    fn category_round_trip() {
        // Round-trip via canonical question samples (not the as_str slug
        // which classify() doesn't necessarily match).
        let samples: &[(Category, &str)] = &[
            (Category::Football, "Will Arsenal win the Premier League?"),
            (Category::Politics, "Will Trump win the 2024 election?"),
            (Category::Crypto, "BTC > 100k by EOY?"),
            (Category::Other, "Mystery box"),
        ];
        for (cat, q) in samples {
            assert_eq!(Category::classify(q), *cat, "sample: {q}");
        }
    }

    #[test]
    fn parse_liquidity_handles_none() {
        assert_eq!(parse_liquidity(None), 0.0);
        assert_eq!(parse_liquidity(Some("1234.5")), 1234.5);
    }

    #[test]
    fn hours_until_close_basic() {
        let now = 1_000_000_000_000;
        assert_eq!(hours_until_close(now + 3_600_000, now), 1);
        assert_eq!(hours_until_close(now - 3_600_000, now), -1);
    }

    #[test]
    fn closes_within_respects_horizon() {
        let now = 1_000_000_000_000;
        let h2 = now + 2 * 3_600_000;
        let h100 = now + 100 * 3_600_000;
        assert!(closes_within(h2, now, 24));
        assert!(!closes_within(h100, now, 24));
    }

    #[test]
    fn closing_bucket_labels() {
        let now = 1_000_000_000_000;
        let h = |h: i64| now + h * 3_600_000;
        assert_eq!(closing_bucket(h(-2), now), "closed");
        assert_eq!(closing_bucket(h(0), now), "< 1h");
        assert_eq!(closing_bucket(h(5), now), "today");
        assert_eq!(closing_bucket(h(72), now), "this week");
        assert_eq!(closing_bucket(h(24 * 14), now), "this month");
        assert_eq!(closing_bucket(h(24 * 90), now), "later");
    }
}
