//! Custom OpenAI-compat proxy (OpenRouter, Azure OpenAI, self-hosted).
//! `provider_kind` = `openai_compat` or `anthropic_compat`.
//!
//! 一个 client 走两种协议（chat/completions 或 messages），按 `provider_kind`
//! 字段 dispatch 到 `call_openai` / `call_anthropic` 私有方法。

use crate::domain::llm::{CallError, CallRequest, CostRate, LlmClient, ProviderKind, err};
use crate::domain::llm::common;
use serde_json::Value;

/// 自定义 OpenAI/Anthropic 兼容 proxy 客户端（OpenRouter / Azure / 自部署 llama-server）。
///
/// **`provider_kind` 必填**：
///   - `OpenaiCompat` — 走 chat/completions 协议
///   - `AnthropicCompat` — 走 messages 协议
///
/// **`api_base`** 必须以 `https://` 开头 + 不带尾 `/`。
pub struct CustomClient {
    pub provider_kind: ProviderKind, // OpenaiCompat or AnthropicCompat
    pub api_base: String,
    pub model: String,
}

impl CustomClient {
    /// 构造一个 OpenAI-compat client（OpenRouter / Azure / 自部署 llama-server）。
    pub fn new_openai_compat(api_base: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            provider_kind: ProviderKind::OpenaiCompat,
            api_base: api_base.into(),
            model: model.into(),
        }
    }
    /// 构造一个 Anthropic-compat client（Anthropic gateway / 自部署 Claude 兼容 proxy）。
    pub fn new_anthropic_compat(api_base: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            provider_kind: ProviderKind::AnthropicCompat,
            api_base: api_base.into(),
            model: model.into(),
        }
    }
}

#[async_trait::async_trait]
impl LlmClient for CustomClient {
    fn kind(&self) -> ProviderKind { self.provider_kind }

    /// Dispatch 到 `call_openai` / `call_anthropic`。
    /// **Misconfigured kind**（不是 `OpenaiCompat` / `AnthropicCompat`）→ `err::UNKNOWN`。
    /// 不 panic（dispatch contract）。
    async fn call(
        &self,
        http: &reqwest::Client,
        secret: &str,
        req: &CallRequest,
        cost: CostRate,
    ) -> Result<crate::domain::llm::CallOutcome, CallError> {
        match self.provider_kind {
            ProviderKind::OpenaiCompat => self.call_openai(http, secret, req, cost).await,
            ProviderKind::AnthropicCompat => self.call_anthropic(http, secret, req, cost).await,
            _ => Err(CallError {
                http_status: None,
                code: err::UNKNOWN,
                message: format!("CustomClient misconfigured with kind={:?}", self.provider_kind),
            }),
        }
    }
}

impl CustomClient {
    /// OpenAI-compat call。**业务流程**跟 `OpenAIClient::call` 一样（同样
    /// `chat/completions` 协议），但走 `CustomClient` 的 `api_base`。
    async fn call_openai(
        &self,
        http: &reqwest::Client,
        secret: &str,
        req: &CallRequest,
        cost: CostRate,
    ) -> Result<crate::domain::llm::CallOutcome, CallError> {
        let url = format!("{}/chat/completions", self.api_base.trim_end_matches('/'));
        let body = common::build_body(req);
        let started = std::time::Instant::now();
        let resp = http.post(&url).bearer_auth(secret).json(&body).send().await
            .map_err(|e| CallError {
                http_status: None,
                code: if e.is_timeout() { err::TIMEOUT } else { err::NETWORK },
                message: e.to_string(),
            })?;
        let status = resp.status().as_u16();
        let text = resp.text().await.unwrap_or_default();
        let mut out = common::parse_response(status, &text, cost, req.response_format_json)?;
        out.latency_ms = started.elapsed().as_millis() as u64;
        if !(200..300).contains(&status) {
            return Err(CallError {
                http_status: Some(status),
                code: common::classify_status(status, &text),
                message: truncate(&text, 300).to_string(),
            });
        }
        Ok(out)
    }

    /// Anthropic-compat call。**业务流程**：
    ///   1. URL = `{api_base}/v1/messages`（Anthropic 协议固定 path）
    ///   2. `x-api-key` header（**不**用 `Authorization: Bearer`）
    ///   3. `anthropic-version: 2023-06-01` header 必填
    ///   4. system prompt 走独立 `system` 字段（split via `split_system`）
    ///   5. 复用 `parse_messages_response`（跟 native Anthropic 同一函数）
    async fn call_anthropic(
        &self,
        http: &reqwest::Client,
        secret: &str,
        req: &CallRequest,
        cost: CostRate,
    ) -> Result<crate::domain::llm::CallOutcome, CallError> {
        // Same wire as native Anthropic, just different base URL.
        let url = format!("{}/v1/messages", self.api_base.trim_end_matches('/'));
        let (system, messages) = split_system(&req.messages);
        let mut body = serde_json::json!({
            "model": req.model,
            "max_tokens": req.max_tokens,
            "temperature": req.temperature,
            "messages": messages,
        });
        if let Some(s) = system { body["system"] = Value::String(s); }
        let started = std::time::Instant::now();
        let resp = http.post(&url)
            .header("x-api-key", secret)
            .header("anthropic-version", "2023-06-01")
            .json(&body).send().await
            .map_err(|e| CallError {
                http_status: None,
                code: if e.is_timeout() { err::TIMEOUT } else { err::NETWORK },
                message: e.to_string(),
            })?;
        let status = resp.status().as_u16();
        let text = resp.text().await.unwrap_or_default();
        let mut out = crate::domain::llm::anthropic::parse_messages_response(status, &text, cost)?;
        out.latency_ms = started.elapsed().as_millis() as u64;
        if !(200..300).contains(&status) {
            return Err(CallError {
                http_status: Some(status),
                code: common::classify_status(status, &text),
                message: truncate(&text, 300).to_string(),
            });
        }
        Ok(out)
    }
}

/// 把 OpenAI 风格的 `Vec<ChatMessage>` 拆成 Anthropic 风格的 `(system, messages)`。
/// **只取第一个 system 消息**（多 system message 取 first 丢弃其余 —— Anthropic
/// 协议只支持一个 system field）。
fn split_system(messages: &[crate::domain::llm::ChatMessage]) -> (Option<String>, Vec<Value>) {
    let mut system = None;
    let mut rest = Vec::new();
    for m in messages {
        if m.role == "system" && system.is_none() {
            system = Some(m.content.clone());
        } else {
            rest.push(serde_json::json!({"role": m.role, "content": m.content}));
        }
    }
    (system, rest)
}

/// 截断 body 字符串到 max 字节。**避免把 10MB 错误响应全存进 audit_log**。
/// 跟 `openai::truncate` 重复（每个 client 文件一个，**不**抽公共 helper，
/// 因为每个 client 可能有不同截断策略）。
fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max { s } else { &s[..max] }
}
