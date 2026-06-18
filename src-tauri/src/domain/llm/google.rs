//! Google Gemini — generateContent API.
//! Spec: https://ai.google.dev/api/generate-content
//!
//! Notes
//! -----
//! - API key is passed as a query param, NOT as Authorization header.
//! - System instructions have a separate field.
//! - Response uses `candidates[0].content.parts[0].text` for the text.
//! - Token counts come from `usageMetadata`.

use crate::domain::llm::{CallError, CallOutcome, CallRequest, CostRate, LlmClient, ProviderKind, err};
use serde_json::{Value, json};

/// Google Gemini `generateContent` 客户端。Spec: https://ai.google.dev/api/generate-content
///
/// **与 OpenAI 的差异**：
///   - API key 在 query string（`?key=...`），**不**在 `Authorization` header
///   - system instructions 有独立 `systemInstruction.parts` 字段
///   - 响应路径是 `candidates[0].content.parts[0].text`
///   - token 计数在 `usageMetadata.promptTokenCount` / `candidatesTokenCount`
///
/// **`api_base`**：默认 `https://generativelanguage.googleapis.com/v1beta`。
/// `request_path` 走 `models/{model}:generateContent`。
pub struct GoogleClient {
    pub api_base: String, // e.g. "https://generativelanguage.googleapis.com/v1beta"
}

impl GoogleClient {
    /// 默认 `api_base = https://generativelanguage.googleapis.com/v1beta`。
    pub fn new() -> Self {
        Self { api_base: "https://generativelanguage.googleapis.com/v1beta".into() }
    }
    /// 自定义 `api_base`（用于 Vertex AI 端点 / 自部署 Gemini 兼容 proxy）。
    pub fn with_base(api_base: impl Into<String>) -> Self { Self { api_base: api_base.into() } }
}

impl Default for GoogleClient {
    fn default() -> Self { Self::new() }
}

#[async_trait::async_trait]
impl LlmClient for GoogleClient {
    fn kind(&self) -> ProviderKind { ProviderKind::Google }

    /// 真实 Gemini call。**业务流程**：
    ///   1. URL = `{api_base}/models/{model}:generateContent?key={secret}`
    ///      （**API key 在 query string** —— Google 跟 OpenAI/Anthropic 不同）
    ///   2. `split_contents` 把 system 提到独立 `systemInstruction` 字段
    ///   3. assistant role 映射到 Gemini 的 `model` role
    ///   4. `parse_response` 解 `candidates[0].content.parts[0].text`
    ///   5. 非 2xx → `classify_status` 归 stable code
    async fn call(
        &self,
        http: &reqwest::Client,
        secret: &str,
        req: &CallRequest,
        cost: CostRate,
    ) -> Result<CallOutcome, CallError> {
        let url = format!(
            "{}/models/{}:generateContent?key={}",
            self.api_base.trim_end_matches('/'),
            req.model,
            secret
        );

        let (system, contents) = split_contents(&req.messages);
        let mut body = json!({
            "contents": contents,
            "generationConfig": {
                "maxOutputTokens": req.max_tokens,
                "temperature": req.temperature,
            },
        });
        if let Some(s) = system {
            body["systemInstruction"] = json!({
                "role": "system",
                "parts": [{"text": s}],
            });
        }

        let started = std::time::Instant::now();
        let resp = http.post(&url).json(&body).send().await.map_err(|e| CallError {
            http_status: None,
            code: if e.is_timeout() { err::TIMEOUT } else { err::NETWORK },
            message: e.to_string(),
        })?;
        let status = resp.status().as_u16();
        let text = resp.text().await.unwrap_or_default();
        let mut out = parse_response(status, &text, cost)?;
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

/// 从 Gemini `generateContent` 响应 JSON 抽 `text` / `tokens_in` / `tokens_out` / `cost_cents`。
///
/// **路径**：
///   - `candidates[0].content.parts[0].text` —— text
///   - `usageMetadata.promptTokenCount` / `candidatesTokenCount` —— tokens
///
/// **Pub**：`GoogleClient::call` + 未来可能的其他 Gemini-compatible client 共用。
pub fn parse_response(
    status: u16,
    body_text: &str,
    cost: CostRate,
) -> Result<CallOutcome, CallError> {
    let body: Value = serde_json::from_str(body_text).map_err(|e| CallError {
        http_status: Some(status),
        code: err::PARSE,
        message: format!("Gemini response not JSON: {e}; body={}", truncate(body_text, 200)),
    })?;
    let text = body.pointer("/candidates/0/content/parts/0/text")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let usage = body.get("usageMetadata");
    let tokens_in = usage
        .and_then(|u| u.get("promptTokenCount"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;
    let tokens_out = usage
        .and_then(|u| u.get("candidatesTokenCount"))
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

/// 把 OpenAI 风格 `Vec<ChatMessage>` 拆成 Gemini 风格 `(system, contents)`。
///
/// **差异**（vs `anthropic::split_system`）：
///   - Gemini role 只有 `user` / `model` —— `assistant` 映射到 `model`
///   - `parts` 是必填数组，每个 part 至少有 `text` field
///   - system 走 `systemInstruction.parts[0].text` 而非 `system` 字段
///
/// **只取第一个 system message**（同 anthropic 协议）。
fn split_contents(messages: &[crate::domain::llm::ChatMessage]) -> (Option<String>, Vec<Value>) {
    let mut system = None;
    let mut contents = Vec::new();
    for m in messages {
        if m.role == "system" && system.is_none() {
            system = Some(m.content.clone());
            continue;
        }
        // Gemini role is "user" | "model"; map assistant -> model
        let role = if m.role == "assistant" { "model" } else { "user" };
        contents.push(json!({
            "role": role,
            "parts": [{"text": m.content}],
        }));
    }
    (system, contents)
}

fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max { s } else { &s[..max] }
}
