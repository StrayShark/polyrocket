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

pub struct GoogleClient {
    pub api_base: String, // e.g. "https://generativelanguage.googleapis.com/v1beta"
}

impl GoogleClient {
    pub fn new() -> Self {
        Self { api_base: "https://generativelanguage.googleapis.com/v1beta".into() }
    }
    pub fn with_base(api_base: impl Into<String>) -> Self { Self { api_base: api_base.into() } }
}

impl Default for GoogleClient {
    fn default() -> Self { Self::new() }
}

#[async_trait::async_trait]
impl LlmClient for GoogleClient {
    fn kind(&self) -> ProviderKind { ProviderKind::Google }

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
