//! Anthropic Messages API client.
//! Spec: https://docs.anthropic.com/en/api/messages

use crate::llm_clients::{CallError, CallOutcome, CallRequest, CostRate, LlmClient, ProviderKind, err};
use serde_json::{Value, json};

pub struct AnthropicClient {
    pub api_base: String, // e.g. "https://api.anthropic.com"
}

impl AnthropicClient {
    pub fn new() -> Self {
        Self { api_base: "https://api.anthropic.com".into() }
    }
    pub fn with_base(api_base: impl Into<String>) -> Self { Self { api_base: api_base.into() } }
}

impl Default for AnthropicClient {
    fn default() -> Self { Self::new() }
}

#[async_trait::async_trait]
impl LlmClient for AnthropicClient {
    fn kind(&self) -> ProviderKind { ProviderKind::Anthropic }

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
                code: crate::llm_clients::common::classify_status(status, &text),
                message: truncate(&text, 300).to_string(),
            });
        }
        Ok(out)
    }
}

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

fn split_system(messages: &[crate::llm_clients::ChatMessage]) -> (Option<String>, Vec<Value>) {
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
