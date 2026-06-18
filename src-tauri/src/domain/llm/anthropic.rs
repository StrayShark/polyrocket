//! Anthropic Messages API client.
//! Spec: <https://docs.anthropic.com/en/api/messages>

use crate::domain::llm::{CallError, CallOutcome, CallRequest, CostRate, LlmClient, ProviderKind, err};
use serde_json::{Value, json};

// Anthropic Messages API 协议的 wire 差异（vs OpenAI）：
//   - 鉴权：x-api-key header (not Authorization: Bearer)
//   - 必填：anthropic-version: 2023-06-01
//   - system prompt 走独立 system 字段（非 messages[0]）
//   - response: content[0].text + usage.input_tokens/output_tokens
//   - cache 标记：cache_creation_input_tokens / cache_read_input_tokens
//   - 计费含 cache read（多 1.1x 或 1.25x）

/// Anthropic Messages API 客户端。Spec: <https://docs.anthropic.com/en/api/messages>
///
/// **与 OpenAI 的差异**：
///   - `x-api-key` header 鉴权（非 `Authorization: Bearer`）
///   - `anthropic-version` header 必须
///   - system prompt 走独立 `system` 字段，非 `messages[0]`
///   - token 计费含 cache_read（多 1.1x / 多 1.25x）
///
/// **`api_base`**：默认 `https://api.anthropic.com`。可指向 gateway proxy
/// （如 AWS Bedrock / GCP Vertex Anthropic）—— 协议兼容就行。
pub struct AnthropicClient {
    pub api_base: String, // e.g. "https://api.anthropic.com"
}

impl AnthropicClient {
    /// 默认 `api_base = https://api.anthropic.com`。
    pub fn new() -> Self {
        Self { api_base: "https://api.anthropic.com".into() }
    }
    /// 自定义 `api_base`（用于 Anthropic gateway / AWS Bedrock / GCP Vertex）。
    pub fn with_base(api_base: impl Into<String>) -> Self { Self { api_base: api_base.into() } }
}

impl Default for AnthropicClient {
    fn default() -> Self { Self::new() }
}

#[async_trait::async_trait]
impl LlmClient for AnthropicClient {
    fn kind(&self) -> ProviderKind { ProviderKind::Anthropic }

    /// 真实 Anthropic call。**业务流程**：
    ///   1. URL = `{api_base}/v1/messages`（Anthropic 固定 path）
    ///   2. `split_system` 把 system message 提到独立字段
    ///   3. `x-api-key` + `anthropic-version: 2023-06-01` headers
    ///   4. `parse_messages_response` 解 `content[0].text` + `usage.*_tokens`
    ///   5. 非 2xx → `classify_status` 归 stable code
    async fn call(
        &self,
        http: &reqwest::Client,
        secret: &str,
        req: &CallRequest,
        cost: CostRate,
    ) -> Result<CallOutcome, CallError> {
        let url = format!("{}/v1/messages", self.api_base.trim_end_matches('/'));
        let (system, messages) = split_system(&req.messages);
        let mut body = json!({
            "model": req.model,
            "max_tokens": req.max_tokens,
            "temperature": req.temperature,
            "messages": messages,
        });
        if let Some(s) = system {
            body["system"] = Value::String(s);
        }

        let started = std::time::Instant::now();
        let resp = http.post(&url)
            .header("x-api-key", secret)
            .header("anthropic-version", "2023-06-01")
            .json(&body)
            .send()
            .await
            .map_err(|e| CallError {
                http_status: None,
                code: if e.is_timeout() { err::TIMEOUT } else { err::NETWORK },
                message: e.to_string(),
            })?;
        let status = resp.status().as_u16();
        let text = resp.text().await.unwrap_or_default();

        let mut out = parse_messages_response(status, &text, cost)?;
        out.latency_ms = started.elapsed().as_millis() as u64;

        if !(200..300).contains(&status) {
            return Err(CallError {
                http_status: Some(status),
                code: crate::domain::llm::common::classify_status(status, &text),
                message: truncate(&text, 300).to_string(),
            });
        }
        Ok(out)
    }
}

/// 从 Anthropic Messages API 响应 JSON 抽 `text` / `tokens_in` / `tokens_out` / `cost_cents`。
///
/// **返回**：`CallOutcome.http_status = status`（即使非 2xx 也 OK；调用方决定
/// 怎么映射到 `CallError`）。
///
/// **为什么 pub**：`custom.rs::call_anthropic` 复用这个 fn（CustomClient 的
/// anthropic_compat variant 用同一 parser）。
pub fn parse_messages_response(
    status: u16,
    body_text: &str,
    cost: CostRate,
) -> Result<CallOutcome, CallError> {
    let body: Value = serde_json::from_str(body_text).map_err(|e| CallError {
        http_status: Some(status),
        code: err::PARSE,
        message: format!("Anthropic response not JSON: {e}; body={}", truncate(body_text, 200)),
    })?;
    // Text content is at content[0].text when type=="text"
    let text = body.pointer("/content/0/text")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let usage = body.get("usage");
    let tokens_in = usage
        .and_then(|u| u.get("input_tokens"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;
    let tokens_out = usage
        .and_then(|u| u.get("output_tokens"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;
    let cost_cents = cost.compute(tokens_in, tokens_out);
    Ok(CallOutcome {
        http_status: status,
        latency_ms: 0,
        tokens_in,
        tokens_out,
        cost_cents,
        text,
        parsed: None,
        parse_ok: true,
        parse_error: None,
    })
}

/// 把 OpenAI 风格 `Vec<ChatMessage>` 拆成 Anthropic 风格 `(system, messages)`。
/// **只取第一个 system message**。Anthropic 协议只支持 1 个 system field —— 多
/// system 走 LLM 时取 first 丢弃其余。
fn split_system(messages: &[crate::domain::llm::ChatMessage]) -> (Option<String>, Vec<Value>) {
    let mut system = None;
    let mut rest = Vec::new();
    for m in messages {
        if m.role == "system" && system.is_none() {
            system = Some(m.content.clone());
        } else {
            rest.push(json!({"role": m.role, "content": m.content}));
        }
    }
    (system, rest)
}

fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max { s } else { &s[..max] }
}
