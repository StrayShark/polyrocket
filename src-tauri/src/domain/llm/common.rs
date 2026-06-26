//! OpenAI 兼容 provider 的共享 wire 格式
//! (OpenAI, DeepSeek, OpenRouter、Azure、任何 `openai_compat` proxy)。

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

/// 解析非流式 chat/completions 响应。
/// 如果有 usage 数据则应用 `cost`。
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

    // 可选的 JSON mode 解析。v0.122c — 处理 LLM 将 JSON 包裹在
    // 思维链 block 中的输出(例如 MiniMax-M2.7 返回
    // `think.../think\n```json\n{...}\n````)（模型思维链包裹 JSON）。
    // 退回到第一个 {...} block,跟 `parse_recommendation` 一样,
    // 这样 CallLog 标志 `success=true`,下游消费者看到的是真实的预测。
    //（可解释性：保留 JSON 字面量,避免翻译后污染模板。）
    let (parse_ok, parsed, parse_error) = if json_mode {
        match serde_json::from_str::<Value>(&text) {
            Ok(v) => (true, Some(v), None),
            Err(_) => {
                // 剥离 think.../think block(部分模型会输出)。
                let cleaned = strip_think_block(&text);
                if let Ok(v) = serde_json::from_str::<Value>(&cleaned) {
                    (true, Some(v), None)
                } else if let Some(start) = cleaned.find('{') {
                    if let Some(end) = cleaned.rfind('}') {
                        if end > start {
                            let slice = &cleaned[start..=end];
                            match serde_json::from_str::<Value>(slice) {
                                Ok(v) => (true, Some(v), None),
                                Err(e) => (false, None, Some(format!(
                                    "json_mode parse: {e}; text={}",
                                    truncate(&cleaned, 200)
                                ))),
                            }
                        } else {
                            (false, None, Some(format!(
                                "json_mode parse: no closing brace; text={}",
                                truncate(&cleaned, 200)
                            )))
                        }
                    } else {
                        (false, None, Some(format!(
                            "json_mode parse: no opening brace; text={}",
                            truncate(&cleaned, 200)
                        )))
                    }
                } else {
                    (false, None, Some(format!(
                        "json_mode parse: no JSON object in text={}",
                        truncate(&cleaned, 200)
                    )))
                }
            }
        }
    } else {
        (true, None, None)
    };

    Ok(CallOutcome {
        http_status: status,
        latency_ms: 0, // 由调用方设置
        tokens_in,
        tokens_out,
        cost_cents,
        text,
        parsed,
        parse_ok,
        parse_error,
    })
}

/// 把 HTTP status (或 transport error) 归类到 8 个 stable error codes 之一。
///
/// **`body_hint` 用于辨识**：
///   - 401 with `{"error": {"code": "invalid_api_key"}}` → `AUTH`
///   - 429 with `Retry-After` header → `RATE_LIMIT`
///   - 400 with `model_not_found` → `MODEL_NOT_FOUND`
///
/// **为什么不只看 status**：401 跟 403 都在 4xx，但一个是 key 错一个是权限不够；
/// 401 with key 错也是 `AUTH`（不是 `INVALID_INPUT`）。
pub fn classify_status(status: u16, body_hint: &str) -> &'static str {
    // 首先,基于 status 分类
    let from_status = match status {
        401 | 403 => err::AUTH,
        404 if body_hint.to_lowercase().contains("model") => err::MODEL_NOT_FOUND,
        404 => err::PARSE,
        408 => err::TIMEOUT,
        429 => err::RATE_LIMIT,
        s if (500..600).contains(&s) => err::NETWORK,
        s if (200..300).contains(&s) => return err::UNKNOWN, // 成功;不应该被分类
        _ => err::UNKNOWN,
    };
    // 基于 body 的覆盖: 4xx 含鉴权相关措辞 → auth。
    // 部分 provider(如 Google)返回 400 "API key not valid" 而不是 401。
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

/// v0.122c — 从 LLM 响应中剥离前导 `think.../think` block。
/// 部分模型 (MiniMax-M2.7、DeepSeek R1、Qwen QwQ) 会将
/// 它们的 JSON 输出包裹在 chain-of-thought block 中,然后才是
/// 实际预测。block 是可选的、多行的,且可能包含嵌套的 `think`（罕见）。
/// 如果存在,返回最后一个 `think` 之后的文本;否则返回原文本。
fn strip_think_block(text: &str) -> String {
    // 查找最后一个 `think` 并取其后的所有内容。
    // 大多数模型只输出一个 block;使用 `rfind` 处理罕见的
    // 模型在响应中间再次写入 `think` 的情况(视为纯文本)。
    if let Some(end) = text.rfind("</think>") {
        let after = &text[end + "</think>".len()..];
        return after.trim().to_string();
    }
    text.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_think_block_basic() {
        let input = "<think>\nanalysis here\n</think>\n```json\n{\"a\":1}\n```";
        assert_eq!(strip_think_block(input), "```json\n{\"a\":1}\n```");
    }

    #[test]
    fn strip_think_block_passthrough() {
        assert_eq!(strip_think_block("{\"a\":1}"), "{\"a\":1}");
    }

    #[test]
    fn strip_think_block_last_of_multiple() {
        // 部分模型会在响应中间重新进入 think;取最后一个。
        let input = "<think>first</think>middle<think>second</think>{json}";
        assert_eq!(strip_think_block(input), "{json}");
    }

    #[test]
    fn parse_response_with_think_block_succeeds() {
        // v0.122c — json_mode 解析必须在 think.../think + json 上成功
        let body = r#"{"choices":[{"message":{"content":"<think>\nI think this is hard.\n</think>\n```json\n{\"x\":42}\n```"}}],"usage":{"prompt_tokens":10,"completion_tokens":5}}"#;
        let out = parse_response(200, body, CostRate { per_1k_in_cents: 0.4, per_1k_out_cents: 1.2 }, true).unwrap();
        assert!(out.parse_ok, "应该在 think 前缀下成功解析: {:?}", out.parse_error);
        assert_eq!(out.parsed.unwrap()["x"], 42);
    }
}
