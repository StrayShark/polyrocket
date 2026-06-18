//! OpenAI client — native (api.openai.com) and identical wire format.
//!
//! OpenAI 官方 API + OpenAI 兼容协议 baseline（DeepSeek / OpenRouter / Azure / 自部署）。
//! 协议同 chat/completions —— 鉴权 `Authorization: Bearer <key>` + body 走
//! `domain::llm::common::build_body`。

use crate::domain::llm::{CallError, CallRequest, CostRate, LlmClient, ProviderKind, err};
use crate::domain::llm::common;

/// OpenAI 官方 API 客户端（`api.openai.com/v1`）。**也是 compat 协议的 baseline**。
///
/// **`api_base`**：默认 `https://api.openai.com/v1`。可指向任何 OpenAI 兼容 endpoint
/// （如 OpenRouter / Azure OpenAI / 本地 llama-server）—— 协议同 chat/completions。
pub struct OpenAIClient {
    pub api_base: String, // e.g. "https://api.openai.com/v1"
}

impl OpenAIClient {
    /// 构造一个 OpenAIClient。`api_base` 末尾 `/` 自动 trim。
    pub fn new(api_base: impl Into<String>) -> Self { Self { api_base: api_base.into() } }
}

#[async_trait::async_trait]
impl LlmClient for OpenAIClient {
    fn kind(&self) -> ProviderKind { ProviderKind::Openai }

    /// 真实 LLM call。**业务流程**：
    ///   1. 构造 URL = `{api_base}/chat/completions`
    ///   2. `bearer_auth(secret)` 鉴权 + JSON body
    ///   3. send → 200..300 → `parse_response` 解出 tokens/text/cost
    ///   4. 非 2xx → `classify_status` 归类到 stable error code
    ///   5. transport error → `classify_transport` 分 timeout vs network
    async fn call(
        &self,
        http: &reqwest::Client,
        secret: &str,
        req: &CallRequest,
        cost: CostRate,
    ) -> Result<crate::domain::llm::CallOutcome, CallError> {
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

/// Transport-level error 分类。**两种 stable code**：
///   - `err::TIMEOUT` — `is_timeout()` 或 connect 失败且 elapsed > 5s
///   - `err::NETWORK` — 其他 transport error（DNS / TLS / connection refused）
///
/// **5s 启发式**：connect 失败 + elapsed 短 → 可能是 fast-fail (DNS 错误)，
/// 还是归 NETWORK；connect 失败 + elapsed 长 → 几乎肯定是 connect timeout。
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

/// 截断 body 字符串到 max 字节。**避免把 10MB 错误响应全存进 audit_log**。
/// **不是字符安全**：byte-level slice 在 UTF-8 边界可能切坏。**接受这点**——
/// CallError.message 只是用于显示，不参与反序列化。
fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max { s } else { &s[..max] }
}
