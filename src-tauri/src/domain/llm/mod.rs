//! LLM HTTP clients — one module per provider, all implement [`LlmClient`].
//!
//! Design notes
//! ------------
//! - Every client returns the same [`CallOutcome`] so the upper layer
//!   (fan-out / scoring / persistence) doesn't need to special-case
//!   per-provider quirks.
//! - The wire format is OpenAI-compatible (chat_completions) for the
//!   majority of providers — OpenAI itself, DeepSeek, OpenRouter, etc.
//!   Anthropic uses Messages API, Google uses generateContent. We keep
//!   the diff small and explicit.
//! - All clients use `reqwest::Client` with rustls. No native TLS dep.
//! - The HTTP client is created once at startup and shared (see
//!   [`new_http_client`]) — connection pool reuses sockets across
//!   providers.

use serde::{Deserialize, Serialize};

pub mod anthropic;
pub mod common;
pub mod deepseek;
pub mod dispatch;
pub mod google;
pub mod openai;
pub mod custom;
pub mod progress;
pub mod prompts;

pub use anthropic::AnthropicClient;
pub use deepseek::DeepSeekClient;
pub use google::GoogleClient;
pub use openai::OpenAIClient;
pub use custom::CustomClient;
pub use dispatch::{CallLog, DispatchOutcome, KeyHandle, RetryPolicy, dispatch};
pub use progress::{
    AnalyzeFinishedEvent, AnalyzeStartedEvent, ConsensusDoneEvent, ProviderDoneEvent,
};
pub use prompts::{
    MarketContext, OrderbookTop, PeerView, SignalSummary,
    PROMPT_VERSION_MARKET_ANALYSIS, PROMPT_VERSION_QUICK_THESIS, PROMPT_VERSION_CONSENSUS_VOTE,
    build_consensus_request, build_market_analysis_request, build_quick_thesis_request,
    parse_recommendation,
};

// ---------- public types ----------

/// Provider kind, mirroring `llm_providers.provider_kind` in SQLite.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProviderKind {
    Openai,
    Anthropic,
    Google,
    Deepseek,
    OpenaiCompat,
    AnthropicCompat,
}

impl ProviderKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Openai => "openai",
            Self::Anthropic => "anthropic",
            Self::Google => "google",
            Self::Deepseek => "deepseek",
            Self::OpenaiCompat => "openai_compat",
            Self::AnthropicCompat => "anthropic_compat",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "openai" => Some(Self::Openai),
            "anthropic" => Some(Self::Anthropic),
            "google" => Some(Self::Google),
            "deepseek" => Some(Self::Deepseek),
            "openai_compat" => Some(Self::OpenaiCompat),
            "anthropic_compat" => Some(Self::AnthropicCompat),
            _ => None,
        }
    }
}

/// A single message in the prompt. Mirrors OpenAI/Anthropic semantics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String, // "system" | "user" | "assistant"
    pub content: String,
}

impl ChatMessage {
    pub fn system(s: impl Into<String>) -> Self { Self { role: "system".into(), content: s.into() } }
    pub fn user(s: impl Into<String>) -> Self { Self { role: "user".into(), content: s.into() } }
}

/// Caller-supplied parameters for one LLM call.
#[derive(Debug, Clone)]
pub struct CallRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub max_tokens: u32,
    pub temperature: f32,
    /// Optional response format hint, e.g. JSON mode for OpenAI / Gemini.
    /// `None` means the default (text).
    pub response_format_json: bool,
}

impl CallRequest {
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            messages: Vec::new(),
            max_tokens: 1024,
            temperature: 0.2,
            response_format_json: false,
        }
    }
    pub fn system(mut self, s: impl Into<String>) -> Self { self.messages.push(ChatMessage::system(s)); self }
    pub fn user(mut self, s: impl Into<String>) -> Self { self.messages.push(ChatMessage::user(s)); self }
    pub fn max_tokens(mut self, n: u32) -> Self { self.max_tokens = n; self }
    pub fn temperature(mut self, t: f32) -> Self { self.temperature = t; self }
    pub fn json_mode(mut self) -> Self { self.response_format_json = true; self }
}

/// Caller-supplied pricing for cost accounting. Cost is recorded per call.
#[derive(Debug, Clone, Copy)]
pub struct CostRate {
    pub per_1k_in_cents: f64,
    pub per_1k_out_cents: f64,
}

impl Default for CostRate {
    fn default() -> Self { Self { per_1k_in_cents: 0.0, per_1k_out_cents: 0.0 } }
}

impl CostRate {
    pub fn compute(&self, tokens_in: u32, tokens_out: u32) -> f64 {
        (tokens_in as f64 / 1000.0) * self.per_1k_in_cents
            + (tokens_out as f64 / 1000.0) * self.per_1k_out_cents
    }
}

/// Standard error code strings — written to `llm_call_logs.error_code`
/// and surfaced to the frontend. Stable across versions.
pub mod err {
    pub const AUTH: &str = "auth";
    pub const RATE_LIMIT: &str = "rate_limit";
    pub const TIMEOUT: &str = "timeout";
    pub const NETWORK: &str = "network";
    pub const PARSE: &str = "parse";
    pub const MODEL_NOT_FOUND: &str = "model_not_found";
    pub const QUOTA: &str = "quota";
    pub const UNKNOWN: &str = "unknown";
}

/// Result of one LLM HTTP call. `Ok(_)` means the request reached the
/// provider, was authenticated, and returned 2xx. Even Ok can carry
/// `parse_ok=false` if the model output didn't match the expected
/// JSON shape — `parsed` is the "best effort" extracted value.
#[derive(Debug, Clone)]
pub struct CallOutcome {
    pub http_status: u16,
    pub latency_ms: u64,
    pub tokens_in: u32,
    pub tokens_out: u32,
    pub cost_cents: f64,
    /// Provider's natural-language reply (text or `content[0].text`).
    pub text: String,
    /// If the prompt asked for JSON, this is the parsed value. May be
    /// `None` even when `text` is non-empty if the JSON was malformed.
    pub parsed: Option<serde_json::Value>,
    /// Did the parser succeed? Always `true` for text-mode prompts.
    pub parse_ok: bool,
    /// Set when `parse_ok=false`. Explains why the parse failed.
    pub parse_error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CallError {
    pub http_status: Option<u16>,
    pub code: &'static str,
    pub message: String,
}

impl std::fmt::Display for CallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} (status={:?}): {}", self.code, self.http_status, self.message)
    }
}

impl std::error::Error for CallError {}

pub type CallResult = Result<CallOutcome, CallError>;

/// Trait every provider implements.
#[async_trait::async_trait]
pub trait LlmClient: Send + Sync {
    fn kind(&self) -> ProviderKind;

    /// Provider-specific call. Implementations should:
    /// - set a per-request timeout
    /// - classify HTTP errors into the stable `err::*` codes
    /// - return parsed tokens + text on 2xx
    /// - never panic
    async fn call(
        &self,
        http: &reqwest::Client,
        secret: &str,
        req: &CallRequest,
        cost: CostRate,
    ) -> CallResult;
}

// ---------- shared helpers ----------

/// Re-export the shared HTTP client factory from L4 infra.
/// Single source of truth lives in [`crate::infra::http::new_http_client`].
/// This re-export keeps the L3 API surface stable for callers that
/// import `llm::new_http_client`.
pub use crate::infra::http::new_http_client;
