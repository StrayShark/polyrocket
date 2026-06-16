//! Shared wire format for OpenAI-compatible providers
//! (OpenAI, DeepSeek, OpenRouter, Azure, any `openai_compat` proxy).

use crate::llm_clients::{CallError, CallOutcome, CallRequest, CostRate, err};
use serde_json::{Value, json};

/// Build the standard chat/completions JSON body.
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
pub fn classify_status(status: u16, body_hint: &str) -> &'static str {
    match status {
        401 | 403 => err::AUTH,
        404 if body_hint.to_lowercase().contains("model") => err::MODEL_NOT_FOUND,
        404 => err::PARSE,
        408 => err::TIMEOUT,
        429 => err::RATE_LIMIT,
        s if (500..600).contains(&s) => err::NETWORK,
        s if (200..300).contains(&s) => err::UNKNOWN, // success; should not be classified
        _ => err::UNKNOWN,
    }
}

fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max { s } else { &s[..max] }
}
