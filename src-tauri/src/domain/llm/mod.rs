//! LLM HTTP 客户端 — 每个 provider 一个 module,所有都实现 [`LlmClient`]。
//!
//! 设计说明
//! ------------
//! - 每个 client 返回相同的 [`CallOutcome`],这样上层
//!   (fan-out / 评分 / 持久化) 不需要按 provider 的怪癖特殊处理。
//! - 大多数 provider 使用 OpenAI 兼容的 wire 格式 (chat_completions)
//!   — OpenAI 自身、DeepSeek、OpenRouter 等。
//!   Anthropic 用 Messages API,Google 用 generateContent。我们保持
//!   差异小且显式。
//! - 所有 client 使用 `reqwest::Client` + rustls,无原生 TLS dep。
//! - HTTP client 在启动时创建一次并共享 (见 [`new_http_client`])
//!   — 连接池跨 provider 复用 socket。

use serde::{Deserialize, Serialize};

pub mod anthropic;
pub mod common;
pub mod deepseek;
pub mod dispatch;
pub mod google;
pub mod openai;
pub mod custom;
pub mod ernie_native;
pub mod hunyuan;
pub mod spark;
pub mod progress;
pub mod prompts;

pub use anthropic::AnthropicClient;
pub use deepseek::DeepSeekClient;
pub use google::GoogleClient;
pub use openai::OpenAIClient;
pub use custom::CustomClient;
pub use ernie_native::ErnieNativeClient;
pub use hunyuan::HunyuanClient;
pub use spark::SparkClient;
pub use dispatch::{CallLog, DispatchOutcome, KeyHandle, RetryPolicy, dispatch};
pub use progress::{
    AnalyzeFinishedEvent, AnalyzeStartedEvent, ConsensusDoneEvent, ProviderDoneEvent,
};
pub use prompts::{
    MarketContext, OrderbookTop, PeerView, SignalSummary,
    FootballMarketType, FootballMatchContext, FootballRecommendationPayload,
    FrameworkBreakdown,
    PROMPT_VERSION_MARKET_ANALYSIS, PROMPT_VERSION_QUICK_THESIS,
    PROMPT_VERSION_CONSENSUS_VOTE, PROMPT_VERSION_FOOTBALL_MATCH,
    build_consensus_request, build_market_analysis_request, build_quick_thesis_request,
    build_football_match_request,
    parse_recommendation, parse_football_recommendation,
};

// ---------- 对外类型 ----------

/// Provider 类型枚举。镜像 SQLite `llm_providers.provider_kind` 列的字符串值
/// （`"openai"` / `"anthropic"` / ...）。
///
/// **如何新增 provider**：
///   1. 在这个 enum 加 variant
///   2. 在 `as_str` / `parse` 加映射
///   3. 写新 client（`anthropic.rs` 风格）
///   4. 在 L1 `AddProviderModal` 暴露
///   5. 在 `LlmClient` trait impl 列表注册
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProviderKind {
    Openai,
    Anthropic,
    Google,
    Deepseek,
    OpenaiCompat,
    AnthropicCompat,
    /// v0.111.1 — ERNIE 百度千帆 native AK/SK 协议
    /// （`wenxinworkshop/chat/{model}` + OAuth2 access_token 鉴权）
    ErnieNative,
    /// v0.113 — Hunyuan 混元 TC3-HMAC-SHA256 协议
    Hunyuan,
    /// v0.114 — Spark 讯飞 WebSocket 协议
    Spark,
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
            Self::ErnieNative => "ernie_native",
            Self::Hunyuan => "hunyuan",
            Self::Spark => "spark",
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
            "ernie_native" => Some(Self::ErnieNative),
            "hunyuan" => Some(Self::Hunyuan),
            "spark" => Some(Self::Spark),
            _ => None,
        }
    }
}

/// 提示词中的单条消息。镜像 OpenAI/Anthropic 语义。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String, // "system" | "user" | "assistant"
    pub content: String,
}

impl ChatMessage {
    pub fn system(s: impl Into<String>) -> Self { Self { role: "system".into(), content: s.into() } }
    pub fn user(s: impl Into<String>) -> Self { Self { role: "user".into(), content: s.into() } }
}

/// 调用方提供的一次 LLM 调用参数。所有 client 都接受这个 shape（不需知道 provider
/// 是 OpenAI / Anthropic / Google）。
///
/// **构造方式**：用 `CallRequest::new(model).system(s).user(s).max_tokens(n)` 链式 API
/// 替代直接构造 4-5 个字段。
///
/// **`response_format_json = true`**：让 OpenAI / Gemini 走 JSON mode。
/// Anthropic 没有原生 JSON mode —— Anthropic client 走 system prompt 强约束。
#[derive(Debug, Clone)]
pub struct CallRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub max_tokens: u32,
    pub temperature: f32,
    /// 可选响应格式提示，例如 OpenAI / Gemini 的 JSON mode。
    /// `None` 表示默认值 (text)。
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

/// 单价（cents / 1k tokens）。**调用方**（`commands::llm_mgmt`）按 provider
/// + model 填。`compute()` 直接算钱。
///
/// **为什么按 1k token**：跟 OpenAI / Anthropic / Google 的定价单位一致。
/// **为什么 cents 不是美元**：避免浮点误差积累（cents 是整数）。
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

/// 8 个 stable LLM error codes。所有 client 的错误都归类到这 8 类之一，序列化到
/// `llm_call_logs.error_code` 字段，给 L1 展示 + 跨版本稳定。
///
/// **为什么不直接用 HTTP status**：401 跟 403 业务上不同（key 过期 vs 权限不够），
/// 但都在 4xx 范围。stable code 让 L1 可以基于 `error_code = 'auth'` 弹「更新 key」
/// toast，而不靠 fuzzy match HTTP 状态码。
///
/// **新增 code**：在 enum 加常量 → 在所有 client 的 `classify_status` 加映射。
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

/// 单次 LLM HTTP 调用的结果。`Ok(_)` 表示 request 到 provider + 鉴权 + 2xx 响应。
///
/// **为什么 `parse_ok = false` 也算 Ok**：HTTP 200 拿到响应但 JSON 不合法时，
/// 仍可能想用 `text` 字段（用户可以手抄）。`parsed` 是 best-effort 提取的 JSON。
///
/// **`cost_cents` 单位**：cents / 1k tokens。0.5 = 半个 cent。
/// **`latency_ms`**：从发请求到拿到完整响应的总时间（含 TLS + DNS + 上行 + 处理）。
#[derive(Debug, Clone)]
pub struct CallOutcome {
    pub http_status: u16,
    pub latency_ms: u64,
    pub tokens_in: u32,
    pub tokens_out: u32,
    pub cost_cents: f64,
    /// Provider 的自然语言回复 (text 或 `content[0].text`)。
    pub text: String,
    /// 如果 prompt 请求 JSON,这是解析后的值。即使 `text` 非空但 JSON 格式错误时
    /// 也可能为 `None`。
    pub parsed: Option<serde_json::Value>,
    /// 解析器是否成功? text-mode prompt 始终为 `true`。
    pub parse_ok: bool,
    /// 当 `parse_ok=false` 时设置,说明解析失败的原因。
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

/// 所有 LLM provider 必须实现的 trait。5 个 client（OpenAI / Anthropic / Google /
/// DeepSeek / Custom）+ 2 个 compat（OpenAI 兼容 / Anthropic 兼容）都实现。
///
/// **`Send + Sync`**：让 `dispatch()` 跨 await 持有 client 引用（dispatch 是 async）。
///
/// **`call()` 契约**：
///   - 必设 per-request timeout
///   - HTTP 错误归类到 `err::*` stable codes
///   - 2xx 返回 `CallOutcome` 含 tokens + text
///   - **绝不 panic**（dispatch 假设 call 总是返回 `CallResult`）
#[async_trait::async_trait]
pub trait LlmClient: Send + Sync {
    fn kind(&self) -> ProviderKind;

    /// Provider 特定的 call。实现应该：
    /// - 设置每次请求的超时
    /// - 把 HTTP 错误归类到 stable `err::*` codes
    /// - 在 2xx 时返回解析后的 tokens + text
    /// - 永不 panic
    async fn call(
        &self,
        http: &reqwest::Client,
        secret: &str,
        req: &CallRequest,
        cost: CostRate,
    ) -> CallResult;
}

// ---------- 共享辅助 ----------

/// 从 L4 infra 重新导出共享的 HTTP client 工厂。
/// 唯一真实来源在 [`crate::infra::http::new_http_client`]。
/// 这个重新导出保持 L3 API 表面稳定,供导入 `llm::new_http_client` 的调用方使用。
pub use crate::infra::http::new_http_client;
