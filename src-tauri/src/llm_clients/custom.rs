//! Custom OpenAI-compat proxy (OpenRouter, Azure OpenAI, self-hosted).
//! `provider_kind` = `openai_compat` or `anthropic_compat`.

use crate::llm_clients::{CallError, CallRequest, CostRate, LlmClient, ProviderKind, err};
use crate::llm_clients::common;
use serde_json::Value;

pub struct CustomClient {
    pub provider_kind: ProviderKind, // OpenaiCompat or AnthropicCompat
    pub api_base: String,
    pub model: String,
}

impl CustomClient {
    pub fn new_openai_compat(api_base: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            provider_kind: ProviderKind::OpenaiCompat,
            api_base: api_base.into(),
            model: model.into(),
        }
    }
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

    async fn call(
        &self,
        http: &reqwest::Client,
        secret: &str,
        req: &CallRequest,
        cost: CostRate,
    ) -> Result<crate::llm_clients::CallOutcome, CallError> {
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
    async fn call_openai(
        &self,
        http: &reqwest::Client,
        secret: &str,
        req: &CallRequest,
        cost: CostRate,
    ) -> Result<crate::llm_clients::CallOutcome, CallError> {
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

    async fn call_anthropic(
        &self,
        http: &reqwest::Client,
        secret: &str,
        req: &CallRequest,
        cost: CostRate,
    ) -> Result<crate::llm_clients::CallOutcome, CallError> {
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
        let mut out = crate::llm_clients::anthropic::parse_messages_response(status, &text, cost)?;
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

fn split_system(messages: &[crate::llm_clients::ChatMessage]) -> (Option<String>, Vec<Value>) {
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

fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max { s } else { &s[..max] }
}
