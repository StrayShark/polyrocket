//! Shared wire format for OpenAI-compatible providers
//! (OpenAI, DeepSeek, OpenRouter, Azure, any `openai_compat` proxy).

use crate::domain::llm::{CallError, CallOutcome, CallRequest, CostRate, err};
use serde_json::{Value, json};

/// 构建标准 OpenAI chat/completions JSON 请求体。
///
/// **调用方**：`OpenAIClient::call` + `DeepSeekClient::call`（通过 OpenAIClient）
/// + `CustomClient::call`（`provider_kind = OpenaiCompat`）。
///
/// **为什么 Anthropic 不复用**：Anthropic 的 system 走独立字段 + 需要
/// `anthropic-version` header，不在 chat/completions 框架里。
pub fn build_body(req: &CallRequest) -> Value {
    let mut body = json!({
        "model": req.model,
        "messages": req.messages.iter().map(|m| json!({"role": m.role, "content": m.content})).collect::<Vec<_>>(),
        "max_tokens": req.max_tokens,
        "temperature": req.temperature,
    });
    if req.response_format_json {
        body["response_format"] = json!({"type": "json_object"});
    }
    body
}

/// Parse a non-streaming chat/completions response.
/// `cost` is applied if usage data is present.
/// 从 chat/completions 响应 JSON 抽 `text` / `tokens_in` / `tokens_out`。
///
/// **返回**：`CallOutcome` 即使 Ok 也可能 `parse_ok = false`（JSON 字段缺失
/// / 形状不匹配）。`text` 始终填（best-effort）。
pub fn parse_response(
    status: u16,
    body_text: &str,
    cost: CostRate,
    json_mode: bool,
) -> Result<CallOutcome, CallError> {
    let body: Value = match serde_json::from_str(body_text) {
        Ok(v) => v,
        Err(e) => {
            return Err(CallError {
                http_status: Some(status),
                code: err::PARSE,
                message: format!("response not JSON: {e}; body={}", truncate(body_text, 200)),
            });
        }
    };

    let text = body
        .pointer("/choices/0/message/content")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let usage = body.get("usage");
    let tokens_in = usage
        .and_then(|u| u.get("prompt_tokens"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;
    let tokens_out = usage
        .and_then(|u| u.get("completion_tokens"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;
    let cost_cents = cost.compute(tokens_in, tokens_out);

    // Optional JSON mode parsing
    let (parse_ok, parsed, parse_error) = if json_mode {
        match serde_json::from_str::<Value>(&text) {
            Ok(v) => (true, Some(v), None),
            Err(e) => (false, None, Some(format!("json_mode parse: {e}; text={}", truncate(&text, 200)))),
        }
    } else {
        (true, None, None)
    };

    Ok(CallOutcome {
        http_status: status,
        latency_ms: 0, // set by caller
        tokens_in,
        tokens_out,
        cost_cents,
        text,
        parsed,
        parse_ok,
        parse_error,
    })
}

/// Classify an HTTP status (or transport error) into a stable `err::*` code.
/// 把 HTTP status + body 提示归类到 8 个 stable error codes 之一。
///
/// **`body_hint` 用于辨识**：
///   - 401 with `{"error": {"code": "invalid_api_key"}}` → `AUTH`
///   - 429 with `Retry-After` header → `RATE_LIMIT`
///   - 400 with `model_not_found` → `MODEL_NOT_FOUND`
///
/// **为什么不只看 status**：401 跟 403 都在 4xx，但一个是 key 错一个是权限不够；
/// 401 with key 错也是 `AUTH`（不是 `INVALID_INPUT`）。
pub fn classify_status(status: u16, body_hint: &str) -> &'static str {
    // First, status-based classification
    let from_status = match status {
        401 | 403 => err::AUTH,
        404 if body_hint.to_lowercase().contains("model") => err::MODEL_NOT_FOUND,
        404 => err::PARSE,
        408 => err::TIMEOUT,
        429 => err::RATE_LIMIT,
        s if (500..600).contains(&s) => err::NETWORK,
        s if (200..300).contains(&s) => return err::UNKNOWN, // success; should not be classified
        _ => err::UNKNOWN,
    };
    // Body-based override: 4xx with auth-like wording → auth.
    // Some providers (e.g. Google) return 400 "API key not valid" instead of 401.
    if (400..500).contains(&status) {
        let lower = body_hint.to_lowercase();
        if lower.contains("api key")
            || lower.contains("auth")
            || lower.contains("credential")
            || lower.contains("permission")
        {
            return err::AUTH;
        }
    }
    from_status
}

fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max { s } else { &s[..max] }
}
