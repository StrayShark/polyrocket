//! Prompt template system — v1 of the polyrocket market-analysis prompt.
//!
//! All prompts ask the model to return a **strict JSON** shape so the
//! parser in [`crate::domain::llm::parse_recommendation`]
//! can extract a clean `(probability, side, confidence, reasoning)` tuple.
//!
//! JSON shape returned by every prompt:
//! ```json
//! {
//!   "probability": 0.68,            // 0..1
//!   "side": "YES",                  // "YES" | "NO" | "skip"
//!   "confidence": 0.72,             // 0..1
//!   "reasoning": "string",          // ≤ 800 chars
//!   "key_factors": ["...", "..."]   // ≤ 5 strings
//! }
//! ```

use crate::domain::llm::{CallRequest, ChatMessage};
use serde::{Deserialize, Serialize};

/// Bump this when the prompt text or shape changes — used to track
/// win-rate per prompt version (F13 stats).
pub const PROMPT_VERSION_MARKET_ANALYSIS: &str = "market.v1.0";
pub const PROMPT_VERSION_QUICK_THESIS: &str = "thesis.v1.0";
pub const PROMPT_VERSION_CONSENSUS_VOTE: &str = "consensus.v1.0";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketContext {
    pub market_id: String,
    pub question: String,
    pub category: String,
    pub yes_price_cents: u32,    // 0..100
    pub no_price_cents: u32,     // 0..100
    pub volume_24h_usdc: f64,
    pub liquidity_usdc: f64,
    pub closes_at_unix_ms: i64,
    pub resolution_source: String,
    pub recent_signals: Vec<SignalSummary>,
    pub orderbook_top: Option<OrderbookTop>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalSummary {
    pub name: String,
    pub value: String,
    pub polarity: String, // "bullish" | "bearish" | "neutral"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderbookTop {
    pub best_bid: u32,
    pub best_ask: u32,
    pub bid_depth: f64,
    pub ask_depth: f64,
}

const MARKET_SYSTEM: &str = "You are a precise prediction-market analyst. \
You must think carefully and return a strict JSON object with fields: \
probability (0..1), side (YES|NO|skip), confidence (0..1), reasoning (string, ≤ 800 chars), \
key_factors (array of ≤ 5 short strings). No prose before or after the JSON. \
The probability must reflect the model's estimate of the YES outcome. \
side = YES if probability > 0.5, NO if < 0.5, skip if confidence < 0.4.";

pub fn build_market_analysis_request(model: &str, ctx: &MarketContext) -> CallRequest {
    let user = serde_json::to_string_pretty(ctx).unwrap_or_default();
    let user = format!("Analyze this market. Return JSON only.\n\n```json\n{user}\n```");
    CallRequest::new(model)
        .max_tokens(1024)
        .temperature(0.2)
        .json_mode()
        .system(MARKET_SYSTEM)
        .user(user)
}

const THESIS_SYSTEM: &str = "You are a sharp markets commentator. \
Reply with a strict JSON object: {thesis: string ≤ 400 chars, action: YES|NO|skip, \
confidence: 0..1, edge_pct: number (your estimated edge vs market price, signed)}. \
No prose, JSON only.";

pub fn build_quick_thesis_request(model: &str, ctx: &MarketContext) -> CallRequest {
    let user = format!(
        "Question: {}\nYES market: ¢{}\nNO market: ¢{}\nVolume 24h: ${:.0}\nCloses: {}\n\n\
         Give your one-sentence thesis and action.",
        ctx.question, ctx.yes_price_cents, ctx.no_price_cents,
        ctx.volume_24h_usdc,
        chrono_format(ctx.closes_at_unix_ms)
    );
    CallRequest::new(model).max_tokens(512).temperature(0.3).json_mode()
        .system(THESIS_SYSTEM)
        .user(user)
}

const CONSENSUS_SYSTEM: &str = "You are a judge evaluating 4 LLM analyses of a prediction market. \
Each LLM gave a probability and reasoning. Return a strict JSON object: \
{final_probability: 0..1, side: YES|NO|skip, confidence: 0..1, dissent: string ≤ 200 chars}. \
Use the median unless two analyses strongly agree and the other two are clearly weaker.";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerView {
    pub provider_id: String,
    pub probability: f64,
    pub reasoning: String,
    pub confidence: f64,
}

pub fn build_consensus_request(model: &str, ctx: &MarketContext, peers: &[PeerView]) -> CallRequest {
    let user = serde_json::to_string_pretty(ctx).unwrap_or_default();
    let peers_json = serde_json::to_string_pretty(peers).unwrap_or_default();
    let user = format!(
        "Market:\n```json\n{user}\n```\n\nPeer analyses:\n```json\n{peers_json}\n```\n\n\
         Aggregate and return the final verdict as JSON."
    );
    CallRequest::new(model).max_tokens(800).temperature(0.1).json_mode()
        .system(CONSENSUS_SYSTEM)
        .user(user)
}

// ---- parse a recommendation from a model that returned JSON ----

#[derive(Debug, Clone, Deserialize)]
pub struct LlmRecommendationPayload {
    pub probability: Option<f64>,
    pub side: Option<String>,
    pub confidence: Option<f64>,
    pub reasoning: Option<String>,
}

pub fn parse_recommendation(text: &str) -> Result<(f64, String, f64, String), String> {
    // 1) try direct parse
    if let Ok(p) = serde_json::from_str::<LlmRecommendationPayload>(text) {
        return Ok((
            p.probability.unwrap_or(0.5).clamp(0.0, 1.0),
            normalize_side(p.side.as_deref().unwrap_or("skip")),
            p.confidence.unwrap_or(0.5).clamp(0.0, 1.0),
            p.reasoning.unwrap_or_default().chars().take(800).collect(),
        ));
    }
    // 2) try to find first {...} block (model may have wrapped JSON in code fences)
    if let Some(start) = text.find('{') {
        if let Some(end) = text.rfind('}') {
            if end > start {
                let slice = &text[start..=end];
                if let Ok(p) = serde_json::from_str::<LlmRecommendationPayload>(slice) {
                    return Ok((
                        p.probability.unwrap_or(0.5).clamp(0.0, 1.0),
                        normalize_side(p.side.as_deref().unwrap_or("skip")),
                        p.confidence.unwrap_or(0.5).clamp(0.0, 1.0),
                        p.reasoning.unwrap_or_default().chars().take(800).collect(),
                    ));
                }
            }
        }
    }
    Err(format!("could not extract recommendation JSON: {}", &text[..text.len().min(200)]))
}

fn normalize_side(s: &str) -> String {
    match s.to_uppercase().as_str() {
        "YES" | "Y" | "TRUE" | "1" => "YES".into(),
        "NO" | "N" | "FALSE" | "0" => "NO".into(),
        _ => "skip".into(),
    }
}

fn chrono_format(unix_ms: i64) -> String {
    use chrono::{DateTime, Utc};
    let dt: DateTime<Utc> = DateTime::<Utc>::from_timestamp_millis(unix_ms)
        .unwrap_or_else(|| Utc::now());
    dt.format("%Y-%m-%d %H:%M UTC").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_clean_json() {
        let txt = r#"{"probability":0.71,"side":"YES","confidence":0.68,"reasoning":"good"}"#;
        let (p, s, c, r) = parse_recommendation(txt).unwrap();
        assert!((p - 0.71).abs() < 1e-6);
        assert_eq!(s, "YES");
        assert!((c - 0.68).abs() < 1e-6);
        assert_eq!(r, "good");
    }

    #[test]
    fn parses_fenced_json() {
        let txt = "```json\n{\"probability\":0.4,\"side\":\"NO\",\"confidence\":0.6,\"reasoning\":\"x\"}\n```";
        let (p, s, _, _) = parse_recommendation(txt).unwrap();
        assert!((p - 0.4).abs() < 1e-6);
        assert_eq!(s, "NO");
    }

    #[test]
    fn normalizes_side_aliases() {
        assert_eq!(normalize_side("y"), "YES");
        assert_eq!(normalize_side("NO"), "NO");
        assert_eq!(normalize_side("true"), "YES");
        assert_eq!(normalize_side("false"), "NO");
        assert_eq!(normalize_side("maybe"), "skip");
    }
}
