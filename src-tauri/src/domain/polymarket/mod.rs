//! L3 — Markets domain (helpers used by L2 commands).
//!
//! Holds the typed `Market` DTO + filter / sort / classify helpers
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

/// v0.124 — wire shape for the `GET /markets/keyset` Gamma API
/// response (per market).
///
/// Polymarket's Gamma API returns each market as a JSON object with
/// many fields. We project only what we need and tolerate absent
/// fields via `Option` / `#[serde(default)]`. The shape was reverse-
/// engineered from a live call in 2026-06.
///
/// Notable quirks (re-checked against /markets/keyset in 2026-06-23):
///   - The wire format is **camelCase** (`endDate`, `volume24hr`,
///     `liquidity`, `closed`, `marketMakerAddress`...). v0.124
///     uses `#[serde(rename_all = "camelCase")]` to map to the
///     Rust snake_case field names below.
///   - `id` is the numeric PM market id as a string
///   - `endDate` is an ISO-8601 string, NOT a unix timestamp
///   - `closed` is the resolved flag; `active` is "orders accepted"
///   - `archived` excludes old markets
///   - `liquidity` is a STRING (e.g. `"16639.4255"`) and so is
///     `volume` (e.g. `"834874.4897460078"`). serde_json does NOT
///     auto-coerce string→number, so we type as String and parse
///     manually in post-processing.
///   - `volume24hr` IS a real number (e.g. `1150.4089619999997`)
///   - `category` and `tags` are null on most markets — fall back
///     to question-text classification
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketSummary {
    /// Numeric PM market id as a string (preserved form for FK).
    pub id: String,
    pub slug: String,
    pub question: String,
    #[serde(default)]
    pub description: Option<String>,
    /// ISO-8601 string from the API. Parsed to ms in post-processing.
    pub end_date: String,
    /// Parsed from `endDate` in post-processing.
    #[serde(skip)]
    pub end_date_ms: Option<i64>,
    pub active: bool,
    pub closed: bool,
    #[serde(default)]
    pub archived: bool,
    /// 24h volume in USDC (real field on keyset endpoint).
    #[serde(default)]
    pub volume_24hr: f64,
    /// Total volume. Gamma sends this as a STRING.
    #[serde(default)]
    pub volume: String,
    /// Same story as `volume`.
    #[serde(default)]
    pub liquidity: String,
    /// `category` is null on most markets.
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
}

/// v0.124 — wire shape for the `GET /markets/keyset` Gamma API
/// response wrapper.
///
/// The keyset endpoint returns:
/// ```json
/// { "markets": [ ... MarketSummary ... ], "next_cursor": "BCVp..." }
/// ```
/// rather than a bare array. We use the wrapper to surface the
/// cursor (not used yet — pagination is a v0.125+ feature).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeysetResponse {
    pub markets: Vec<MarketSummary>,
    #[serde(default)]
    pub next_cursor: Option<String>,
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

/// v0.125 — fetch open markets from Polymarket Gamma, single
/// request, limit=20.
///
/// **Why single request**: the `/markets/keyset` `next_cursor`
/// pagination is broken at the API level (2026-06-23 verified:
/// passing the returned cursor as `&next_cursor=` returns the
/// SAME first page). The legacy `/markets?offset=N` works but
/// the endpoint is deprecated. The cleanest working approach
/// is one request with `limit=20` — returns ~14 football
/// markets in the top-volume ordering, plenty for the L1.
///
/// **Why volume24hr-DESC**: the default `endDate ASC` ordering
/// puts the FIFA World Cup 2026 markets (close Aug 2026) far
/// down the list — first 5 are Rihanna albums, GTA-VI bets,
/// elections. Volume-DESC surfaces football in the first 20.
///
/// **Why not >20**: the Gamma API has two bugs above 50 items
/// that the local HTTP proxy (POLYROCKET_PROXY=127.0.0.1:7897)
/// surfaces intermittently: (a) `Invalid control character` in
/// the JSON body, (b) reqwest body-decoder hangs on the
/// proxied HTTP/2 stream. 20 stays well under that threshold.
///
/// **Caller contract**: returns up to 20 markets (raw, not
/// pre-filtered). The football filter at the sync boundary
/// drops ~30% of them. L1 sees ~14 real football markets.
pub async fn fetch_active_markets() -> crate::AppResult<Vec<MarketSummary>> {
    use crate::infra::http::new_http_client;

    const LIMIT: usize = 20;

    let client = new_http_client();
    let url = format!(
        "{}/markets/keyset?active=true&closed=false&archived=false\
         &limit={}&order=volume24hr&ascending=false",
        GAMMA_BASE, LIMIT
    );

    let bytes = fetch_page(&client, &url).await?;
    let resp: KeysetResponse = serde_json::from_slice(&bytes).map_err(|e| {
        let preview: String = String::from_utf8_lossy(&bytes[..2000.min(bytes.len())])
            .chars().take(500).collect();
        tracing::warn!(
            "fetch_active_markets: JSON deser failed: {e}; preview={preview}"
        );
        crate::AppError::Internal(format!("Gamma keyset JSON deser failed: {e}"))
    })?;


    let mut all = resp.markets;
    for m in all.iter_mut() {
        m.end_date_ms = parse_iso_to_ms(&m.end_date);
    }
    tracing::info!(
        "fetch_active_markets: returned {} markets (football={})",
        all.len(), count_football(&all)
    );
    Ok(all)
}

/// One page fetch helper.
///
/// v0.125 — bumps per-request timeout from 10s to 30s. The
/// local HTTP proxy (127.0.0.1:7897) takes ~10-18s to
/// establish CONNECT to gamma-api.polymarket.com on cold
/// paths; 10s was below the floor and the body read surfaced
/// as `error decoding response body` (reqwest quirk: the
/// underlying transport timeout is masked as a decode error).
///
/// `read body as bytes()` (not `.json()`) preserves the
/// real error message in the case of an actual decode
/// failure (kept from v0.124).
///
/// 2 retries with 200ms / 500ms backoff — proxy is flaky
/// but a single retry almost always clears it.
async fn fetch_page(
    client: &reqwest::Client,
    url: &str,
) -> crate::AppResult<Vec<u8>> {
    use crate::AppError;
    let mut last_err: Option<String> = None;
    for attempt in 0..3 {
        let result = client
            .get(url)
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("Gamma send failed: {e}")))?
            .error_for_status()
            .map_err(|e| AppError::Internal(format!("Gamma HTTP error: {e}")))?
            .bytes()
            .await
            .map(|b| b.to_vec())
            .map_err(|e| AppError::Internal(format!("Gamma body read failed: {e}")));
        match result {
            Ok(b) => return Ok(b),
            Err(e) => {
                let msg = e.to_string();
                tracing::warn!("fetch_page attempt {} failed: {msg}", attempt + 1);
                last_err = Some(msg);
                // 200ms, then 500ms — fast enough that the user
                // doesn't notice unless every attempt fails
                tokio::time::sleep(std::time::Duration::from_millis(
                    if attempt == 0 { 200 } else { 500 },
                )).await;
            }
        }
    }
    Err(AppError::Internal(format!(
        "Gamma page fetch failed after 3 attempts: {}",
        last_err.unwrap_or_default()
    )))
}

/// Count football markets using the same question-text heuristic
/// the sync filter uses. Duplicates the filter heuristic so we
/// can saturate early (when the L1 already has enough football).
/// When the heuristic changes, both this and `is_football_market`
/// in `commands/market.rs` need updating.
fn count_football(markets: &[MarketSummary]) -> usize {
    let is_football_text = |s: &str| -> bool {
        let lower = s.to_lowercase();
        lower.contains("football") || lower.contains("soccer")
            || lower.contains("fifa") || lower.contains("uefa")
            || lower.contains("champions league")
            || lower.contains("premier league") || lower.contains("la liga")
            || lower.contains("bundesliga") || lower.contains("serie a")
            || lower.contains("ligue 1") || lower.contains("mls")
            || lower.contains(" fc ") || lower.contains(" united")
            || lower.contains("real madrid") || lower.contains("barcelona")
            || lower.contains("liverpool") || lower.contains("arsenal")
            || lower.contains("chelsea") || lower.contains("manchester")
            || lower.contains("bayern") || lower.contains("dortmund")
            || lower.contains("juventus") || lower.contains("psg")
            || lower.contains("atletico")
    };
    markets.iter().filter(|m| {
        if let Some(cat) = m.category.as_deref() {
            if is_football_text(cat) { return true; }
        }
        if let Some(tags) = m.tags.as_ref() {
            for t in tags { if is_football_text(t) { return true; } }
        }
        is_football_text(&m.question)
    }).count()
}

/// Parse a RFC-3339 / ISO-8601 string (e.g. "2025-10-31T00:00:00Z")
/// to unix milliseconds. Returns None on parse error so the caller
/// can decide to drop the market or keep it with end_date_ms=None.
fn parse_iso_to_ms(s: &str) -> Option<i64> {
    use chrono::DateTime;
    DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|dt| dt.timestamp_millis())
}

/// v0.124 — parse a numeric STRING (e.g. `"18465.6429"`) to f64.
/// Used for Gamma's `liquidity` and `volume` fields, which the
/// keyset endpoint serializes as strings (legacy from the days
/// when they held arbitrary-precision fractions).
fn parse_numeric_string(s: &str) -> f64 {
    s.parse::<f64>().unwrap_or(0.0)
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

// ============================================================
// ============== v0.51c — CLOB submit path =====================
// ============================================================
//
// v0.5d's `place_signed_order` was a stub returning
// an Internal error. v0.50 made `place_signed_order`
// in `commands::bet` ignore that error and use the
// deterministic sign stub. v0.51c adds a structured
// CLOB submit path that's wired to real Polymarket
// credentials when the env vars are set.
//
// The CLOB submit attempt is two phases:
//   1. Build the EIP-712 signed order payload (the
//      `Order` struct + signature). Today (no
//      `rs-clob-client`) we still synthesize a
//      deterministic placeholder; the shape matches
//      what `rs-clob-client` would produce.
//   2. POST the payload to the CLOB /order endpoint.
//      Real implementation needs the wallet signature
//      to pass the L2 auth header — captured below as
//      the `auth` field for forward-compat.
//
// `submit_signed_order_via_clob` returns a structured
// `ClobOrderResult` so the caller can record:
//   - filled_at + fill_price + fill_size + partial
//   - tx_hash (always present)
//   - the error message if the CLOB rejected
//
// In v0.51a/b (no CLOB feed wired yet) the
// `creds_present()` check returns false; callers
// fall back to the deterministic stub. v0.51c wires
// the HTTP call so when creds are present we
// actually attempt the submit; if the API isn't
// reachable we return a structured error rather
// than crashing.

/// v0.51c — true iff all three Polymarket CLOB credentials
/// are present in the env. Accepts both naming conventions:
///   - `POLYMARKET_API_KEY / _SECRET / _PASSPHRASE` (standard)
///   - `POLYROCKET_CLOB_API_KEY / _SECRET / _PASSPHRASE` (legacy)
/// v0.119 — `POLYMARKET_*` wins when both are set.
pub fn creds_present() -> bool {
    let k = std::env::var("POLYMARKET_API_KEY").ok().filter(|v| !v.is_empty())
        .or_else(|| std::env::var("POLYROCKET_CLOB_API_KEY").ok().filter(|v| !v.is_empty()));
    let s = std::env::var("POLYMARKET_API_SECRET").ok().filter(|v| !v.is_empty())
        .or_else(|| std::env::var("POLYROCKET_CLOB_API_SECRET").ok().filter(|v| !v.is_empty()));
    let p = std::env::var("POLYMARKET_API_PASSPHRASE").ok().filter(|v| !v.is_empty())
        .or_else(|| std::env::var("POLYROCKET_CLOB_API_PASSPHRASE").ok().filter(|v| !v.is_empty()));
    matches!((k, s, p), (Some(k), Some(s), Some(p))
        if !k.is_empty() && !s.is_empty() && !p.is_empty())
}

/// v0.51c — the structured outcome of a CLOB
/// submit attempt. `ok` distinguishes a successful
/// submit from a CLOB-level rejection; the `error`
/// field is non-empty when ok=false.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClobOrderResult {
    pub ok: bool,
    /// Transaction hash from the CLOB. For the
    /// deterministic stub this is the djb2 hash
    /// already produced by `sign_order`. For a real
    /// CLOB submit it's whatever the API returns.
    pub tx_hash: String,
    /// When the order was filled. For the stub
    /// (and v0.51c's HTTP attempt) this is `now_ms`.
    pub filled_at_ms: i64,
    /// Actual fill price. For the stub and v0.51c's
    /// best-effort HTTP path, this is the user's
    /// `price` (slippage = 0). Real CLOB responses
    /// would override.
    pub fill_price: f64,
    /// Actual fill size in shares (string for
    /// back-compat). For the stub and v0.51c HTTP
    /// path, this equals the user's desired size.
    pub fill_size: String,
    /// True when the fill was partial. Always false
    /// for the deterministic stub / v0.51c.
    pub partial: bool,
    /// Error message when ok=false. Empty on success.
    pub error: String,
    /// v0.51c — true when the call was made via
    /// the HTTP path (creds present) vs the stub
    /// (creds absent). Useful for the L1 to surface
    /// "live" vs "stub" in the UI.
    pub via_http: bool,
}

/// v0.51c — best-effort CLOB submit. When creds
/// are present, attempts a real HTTP POST to the
/// CLOB /order endpoint. When the API is
/// unreachable or returns an error, returns
/// `ClobOrderResult { ok: false, error: ... }`
/// rather than panicking — callers can decide to
/// fall back to the stub.
///
/// When creds are absent, returns
/// `ClobOrderResult { ok: true, ..., via_http: false }`
/// with deterministic stub values (slippage = 0,
/// no partials). The caller can persist this as a
/// "would-have-filled" record — exactly what v0.50
/// does today.
///
/// The HTTP path today is best-effort: we POST a
/// placeholder payload, parse the response if
/// possible, and degrade to the stub shape on any
/// error. The full EIP-712 + L2 auth wiring is
/// v0.51+ (requires `rs-clob-client`).
pub async fn submit_signed_order_via_clob(
    market_id: &str,
    side: &str,
    price: f64,
    size_shares: &str,
    key_alias: &str,
    order_type: &str,
    now_ms: i64,
) -> ClobOrderResult {
    use crate::infra::http::new_http_client;
    let tx_hash = format!(
        "0x{:016x}",
        djb2_stub(market_id, side, price, size_shares, key_alias, order_type, now_ms)
    );
    if !creds_present() {
        // No creds → deterministic stub. The caller
        // persists this; the user gets slippage=0.
        return ClobOrderResult {
            ok: true,
            tx_hash,
            filled_at_ms: now_ms,
            fill_price: price,
            fill_size: size_shares.to_string(),
            partial: false,
            error: String::new(),
            via_http: false,
        };
    }
    // v0.51c — best-effort HTTP path. We POST a
    // placeholder payload; on any failure we
    // surface a structured error.
    let url = format!("{}/order", CLOB_BASE);
    let payload = serde_json::json!({
        "market": market_id,
        "side": side.to_uppercase(),
        "price": price,
        "size": size_shares,
        "orderType": order_type,
        "keyAlias": key_alias,
        "ts_ms": now_ms,
    });
    let client = new_http_client();
    // v0.119 — accept either naming convention (POLYMARKET_* preferred)
    let api_key = std::env::var("POLYMARKET_API_KEY").ok().filter(|v| !v.is_empty())
        .or_else(|| std::env::var("POLYROCKET_CLOB_API_KEY").ok())
        .unwrap_or_default();
    let api_secret = std::env::var("POLYMARKET_API_SECRET").ok().filter(|v| !v.is_empty())
        .or_else(|| std::env::var("POLYROCKET_CLOB_API_SECRET").ok())
        .unwrap_or_default();
    let api_passphrase = std::env::var("POLYMARKET_API_PASSPHRASE").ok().filter(|v| !v.is_empty())
        .or_else(|| std::env::var("POLYROCKET_CLOB_API_PASSPHRASE").ok())
        .unwrap_or_default();
    let resp = client
        .post(&url)
        .header("POLYROCKET-API-KEY", &api_key)
        .header("POLYROCKET-API-SECRET", &api_secret)
        .header("POLYROCKET-API-PASSPHRASE", &api_passphrase)
        .json(&payload)
        .send()
        .await;
    match resp {
        Ok(r) if r.status().is_success() => {
            // Real CLOB response — best-effort parse.
            // If the shape differs (e.g. CLOB returns
            // a non-JSON 200), fall back to stub values
            // with via_http=true.
            match r.json::<serde_json::Value>().await {
                Ok(v) => {
                    let fill_price = v
                        .get("price")
                        .and_then(|x| x.as_f64())
                        .unwrap_or(price);
                    let fill_size = v
                        .get("sizeFilled")
                        .and_then(|x| x.as_str())
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| size_shares.to_string());
                    let partial = v
                        .get("partial")
                        .and_then(|x| x.as_bool())
                        .unwrap_or(false);
                    ClobOrderResult {
                        ok: true,
                        tx_hash,
                        filled_at_ms: now_ms,
                        fill_price,
                        fill_size,
                        partial,
                        error: String::new(),
                        via_http: true,
                    }
                }
                Err(_) => ClobOrderResult {
                    ok: true,
                    tx_hash,
                    filled_at_ms: now_ms,
                    fill_price: price,
                    fill_size: size_shares.to_string(),
                    partial: false,
                    error: "CLOB returned non-JSON response (stubbed)".into(),
                    via_http: true,
                },
            }
        }
        Ok(r) => ClobOrderResult {
            ok: false,
            tx_hash,
            filled_at_ms: now_ms,
            fill_price: price,
            fill_size: size_shares.to_string(),
            partial: false,
            error: format!("CLOB HTTP {}", r.status()),
            via_http: true,
        },
        Err(e) => ClobOrderResult {
            ok: false,
            tx_hash,
            filled_at_ms: now_ms,
            fill_price: price,
            fill_size: size_shares.to_string(),
            partial: false,
            error: format!("CLOB HTTP error: {e}"),
            via_http: true,
        },
    }
}

/// Tiny djb2 hash (mirrors `sign_order` in
/// `domain::bet`). Used by the stub path so the
/// tx_hash is stable across runs.
fn djb2_stub(
    market_id: &str,
    side: &str,
    price: f64,
    size: &str,
    key_alias: &str,
    order_type: &str,
    now_ms: i64,
) -> u64 {
    let s = format!(
        "{}|{}|{}|{}|{}|{}|{}",
        market_id, side, price, size, key_alias, order_type, now_ms
    );
    let mut h: u64 = 5381;
    for b in s.bytes() {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    /// v0.124 — deserialize a real Gamma /markets/keyset response
    /// (cached to /tmp/gamma_response.json by a curl) into the
    /// `KeysetResponse` DTO. If this test fails, the wire shape
    /// has drifted and the live sync will silently fail in prod.
    #[test]
    fn deser_real_gamma_keyset_response() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("..")
            .join("..")
            .join("..")
            .join("/tmp/gamma_response.json");
        let body = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(_) => {
                eprintln!(
                    "skip: {} not present (curl it with: \
                     curl -sS -x http://127.0.0.1:7897 \
                     'https://gamma-api.polymarket.com/markets/keyset?active=true&closed=false&limit=2' \
                     > {})",
                    path.display(),
                    path.display()
                );
                return;
            }
        };
        let r: KeysetResponse = serde_json::from_str(&body)
            .unwrap_or_else(|e| panic!("deser failed: {e}; body start: {}", &body[..200.min(body.len())]));
        assert!(!r.markets.is_empty(), "expected at least one market");
        let m = &r.markets[0];
        // sanity-check the field projection
        assert!(!m.id.is_empty());
        assert!(!m.question.is_empty());
        assert!(!m.end_date.is_empty());
        // end_date_ms gets populated by post-processing
        assert!(m.end_date_ms.is_none(), "end_date_ms is set by post-processor, not deser");
    }


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

    // ----- v0.51c — CLOB submit path -----

    // Process-global mutex for env var manipulation.
    // Tests in cargo run in parallel; std::env::set_var
    // is process-global, so we serialize the CLOB
    // cred tests behind a single mutex.
    use std::sync::Mutex;
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn clear_clob_env() {
        unsafe {
            std::env::remove_var("POLYROCKET_CLOB_API_KEY");
            std::env::remove_var("POLYROCKET_CLOB_API_SECRET");
            std::env::remove_var("POLYROCKET_CLOB_API_PASSPHRASE");
        }
    }

    /// v0.51c — creds_present is false when none
    /// of the three env vars are set.
    #[test]
    fn creds_absent_returns_false() {
        let _g = ENV_LOCK.lock().unwrap();
        clear_clob_env();
        assert!(!creds_present());
    }

    /// v0.51c — creds_present is true only when
    /// all three env vars are non-empty.
    #[test]
    fn creds_all_set_returns_true() {
        let _g = ENV_LOCK.lock().unwrap();
        clear_clob_env();
        unsafe {
            std::env::set_var("POLYROCKET_CLOB_API_KEY", "k");
            std::env::set_var("POLYROCKET_CLOB_API_SECRET", "s");
            std::env::set_var("POLYROCKET_CLOB_API_PASSPHRASE", "p");
        }
        let r = creds_present();
        clear_clob_env();
        assert!(r);
    }

    /// v0.51c — empty strings are treated as absent
    /// (the CLOB API rejects empty creds with a 401,
    /// so this is the conservative policy).
    #[test]
    fn creds_with_empty_strings_returns_false() {
        let _g = ENV_LOCK.lock().unwrap();
        clear_clob_env();
        unsafe {
            std::env::set_var("POLYROCKET_CLOB_API_KEY", "");
            std::env::set_var("POLYROCKET_CLOB_API_SECRET", "s");
            std::env::set_var("POLYROCKET_CLOB_API_PASSPHRASE", "p");
        }
        let r = creds_present();
        clear_clob_env();
        assert!(!r);
    }

    /// v0.51c — without creds, the CLOB submit
    /// returns the deterministic stub shape: ok=true,
    /// slippage=0, partial=false, via_http=false,
    /// and the tx_hash is the djb2 stub hash.
    #[tokio::test]
    async fn submit_without_creds_uses_stub() {
        let _g = ENV_LOCK.lock().unwrap();
        clear_clob_env();
        let r = submit_signed_order_via_clob(
            "m1", "YES", 0.5, "100", "primary", "market",
            1_700_000_000_000,
        )
        .await;
        clear_clob_env();
        assert!(r.ok);
        assert!(!r.via_http);
        assert!(!r.partial);
        assert!((r.fill_price - 0.5).abs() < 1e-9);
        assert_eq!(r.fill_size, "100");
        assert!(r.error.is_empty());
        assert!(r.tx_hash.starts_with("0x"));
    }

    /// v0.51c — same args produce the same hash
    /// (deterministic stub stability).
    #[tokio::test]
    async fn submit_stub_hash_is_deterministic() {
        let _g = ENV_LOCK.lock().unwrap();
        clear_clob_env();
        let r1 = submit_signed_order_via_clob(
            "m1", "YES", 0.5, "100", "primary", "market",
            1_700_000_000_000,
        )
        .await;
        let r2 = submit_signed_order_via_clob(
            "m1", "YES", 0.5, "100", "primary", "market",
            1_700_000_000_000,
        )
        .await;
        clear_clob_env();
        assert_eq!(r1.tx_hash, r2.tx_hash);
    }

    /// v0.51c — different args produce different
    /// hashes (hash actually depends on inputs).
    #[tokio::test]
    async fn submit_stub_hash_differs_with_inputs() {
        let _g = ENV_LOCK.lock().unwrap();
        clear_clob_env();
        let r1 = submit_signed_order_via_clob(
            "m1", "YES", 0.5, "100", "primary", "market",
            1_700_000_000_000,
        )
        .await;
        let r2 = submit_signed_order_via_clob(
            "m2", "YES", 0.5, "100", "primary", "market",
            1_700_000_000_000,
        )
        .await;
        clear_clob_env();
        assert_ne!(r1.tx_hash, r2.tx_hash);
    }
}
