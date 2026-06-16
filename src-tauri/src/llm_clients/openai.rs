//! OpenAI client — native (api.openai.com) and identical wire format.

use crate::llm_clients::{CallError, CallRequest, CostRate, LlmClient, ProviderKind, err};
use crate::llm_clients::common;

pub struct OpenAIClient {
    pub api_base: String, // e.g. "https://api.openai.com/v1"
}

impl OpenAIClient {
    pub fn new(api_base: impl Into<String>) -> Self { Self { api_base: api_base.into() } }
}

#[async_trait::async_trait]
impl LlmClient for OpenAIClient {
    fn kind(&self) -> ProviderKind { ProviderKind::Openai }

    async fn call(
        &self,
        http: &reqwest::Client,
        secret: &str,
        req: &CallRequest,
        cost: CostRate,
    ) -> Result<crate::llm_clients::CallOutcome, CallError> {
        let url = format!("{}/chat/completions", self.api_base.trim_end_matches('/'));
        let body = common::build_body(req);
        let started = std::time::Instant::now();

        let resp = http
            .post(&url)
            .bearer_auth(secret)
            .json(&body)
            .send()
            .await
            .map_err(|e| classify_transport(&e, started.elapsed().as_millis() as u64))?;

        let status = resp.status().as_u16();
        let body_text = resp.text().await.unwrap_or_default();
        let mut out = common::parse_response(status, &body_text, cost, req.response_format_json)?;
        out.latency_ms = started.elapsed().as_millis() as u64;

        if !(200..300).contains(&status) {
            let code = common::classify_status(status, &body_text);
            return Err(CallError {
                http_status: Some(status),
                code,
                message: truncate(&body_text, 300).to_string(),
            });
        }
        Ok(out)
    }
}

fn classify_transport(e: &reqwest::Error, elapsed_ms: u64) -> CallError {
    if e.is_timeout() || e.is_connect() && elapsed_ms > 5_000 {
        CallError {
            http_status: None,
            code: err::TIMEOUT,
            message: e.to_string(),
        }
    } else {
        CallError {
            http_status: None,
            code: err::NETWORK,
            message: e.to_string(),
        }
    }
}

fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max { s } else { &s[..max] }
}
