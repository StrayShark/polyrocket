//! ERNIE 百度千帆 (v0.111.1) —— Native AK/SK + Wenxinworkshop chat 协议。
//!
//! **为什么单独建一个客户端**（vs v0.111 使用的 CustomClient(OpenaiCompat)）：
//!   - 千帆提供两种鉴权方式：
//!     1. **Modern OpenAI 兼容 v2**（`qianfan.baidubce.com/v2/...`，使用
//!        `bce-v3/ALTAK-...` key）—— v0.111 走这条路（CustomClient）
//!     2. **Native AK/SK**（老的 `wenxinworkshop/chat/{model}` 协议）—— v0.111.1
//!        新增这条路给持有老 API key 的用户
//!   - Native 协议是两段式：
//!     a) AK + SK → `oauth/2.0/token` → access_token
//!     b) access_token → `wenxinworkshop/chat/{model}?access_token=...` → chat
//!   - 响应形状也不同：native 返回 `{"result": "..."}`，不是 OpenAI 的
//!     `{"choices": [{"message": {"content": "..."}}]}`。
//!
//! **实现要点**：
//!   - `ErnieNativeClient` 持有 `api_key`（AK）+ `secret_key`（SK）+ `model`
//!   - 调用前 lazy-fetch access_token（用 `tokio::sync::Mutex<Option<TokenCache>>`）
//!   - access_token 缓存 29 天（BCE 实际是 30 天 TTL）
//!   - Native 协议不支持 stream，一次性返回
//!
//! **Keyring secret 格式**：由于 OS keyring 一条 entry 存一个 secret，我们用
//! `concat!("{api_key}:{secret_key}")` 格式，parse 时用 splitn(2, ':')。
//! 这是 v0.111.1 ERNIE native 的约定，前端 keyring 写入时按这个格式。

use crate::domain::llm::{CallError, CallOutcome, CallRequest, CostRate, LlmClient, ProviderKind, err};
use crate::domain::llm::common;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex;

/// 千帆 AK/SK 鉴权的 OAuth2 端点。固定不变。
const TOKEN_URL: &str = "https://aip.baidubce.com/oauth/2.0/token";

/// 千帆 wenxinworkshop 聊天端点的 base。
/// 模型 ID 拼在最后: `https://aip.baidubce.com/rpc/2.0/ai_custom/v1/wenxinworkshop/chat/{model}`
const CHAT_BASE: &str = "https://aip.baidubce.com/rpc/2.0/ai_custom/v1/wenxinworkshop/chat";

/// Access token 缓存提前 1 天过期 (避免边界条件)。
const TOKEN_TTL_SAFETY_MARGIN_SECS: u64 = 24 * 3600;

/// 千帆 access_token 响应 (OAuth2 client_credentials grant).
#[derive(Debug, Clone, Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    /// 秒。千帆返回 30 天 = 2592000。
    #[serde(default = "default_expires_in")]
    pub expires_in: u64,
    /// scope / refresh_token / session_key — 暂时不用,千帆是 client_credentials grant 无 refresh。
    #[serde(default)]
    pub scope: Option<String>,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub session_key: Option<String>,
}

fn default_expires_in() -> u64 { 2592000 } // 30 天默认值

/// Access token 缓存条目。`fetched_at_unix` 是 `SystemTime::now()` 时的 unix 时间。
#[derive(Debug, Clone)]
pub struct TokenCache {
    pub access_token: String,
    pub fetched_at_unix: u64,
    pub expires_in: u64,
}

impl TokenCache {
    pub fn is_expired(&self, now_unix: u64) -> bool {
        let age = now_unix.saturating_sub(self.fetched_at_unix);
        age + TOKEN_TTL_SAFETY_MARGIN_SECS >= self.expires_in
    }
}

/// 千帆 chat 响应 (native wenxinworkshop 协议)。
///
/// 关键字段:
///   - `result` — 模型返回的文本 (不是 `choices[0].message.content`)
///   - `usage` — token 计数 (千帆是 `prompt_tokens` / `completion_tokens` / `total_tokens`)
///   - `is_truncated` — 是否被截断 (output 超 max_output_tokens)
///   - `need_clear_history` — 敏感信息 flag (security)
#[derive(Debug, Clone, Deserialize)]
pub struct ErnieChatResponse {
    /// 回包 id
    pub id: Option<String>,
    /// 固定 "chat.completion"
    pub object: Option<String>,
    /// 时间戳 (秒)
    pub created: Option<u64>,
    /// 模型回答 — **native 用 `result` 不是 `choices`**
    pub result: String,
    /// 是否被截断
    #[serde(default)]
    pub is_truncated: bool,
    /// 是否需要清空历史 (sensitive content)
    #[serde(default)]
    pub need_clear_history: bool,
    #[serde(default)]
    pub usage: Option<ErnieUsage>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ErnieUsage {
    #[serde(default)]
    pub prompt_tokens: u32,
    #[serde(default)]
    pub completion_tokens: u32,
    #[serde(default)]
    pub total_tokens: u32,
}

/// 千帆 chat 请求 body。
#[derive(Debug, Serialize)]
pub struct ErnieChatRequest {
    pub messages: Vec<ErnieMessage>,
    /// 0.0-1.0, 默认 0.95
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    /// 0.0-1.0, 默认 0.7
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,
    /// 1.0-2.0, 默认 1.0
    #[serde(skip_serializing_if = "Option::is_none")]
    pub penalty_score: Option<f64>,
    /// system prompt (千帆独立字段)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    /// 用户 id (for monitoring)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    /// 最大输出 token 数,范围 2-2048
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ErnieMessage {
    pub role: String,
    pub content: String,
}

/// ERNIE native client. 持有 AK + SK + model, lazy 拿 access_token。
pub struct ErnieNativeClient {
    pub api_key: String,
    pub secret_key: String,
    pub model: String,
    token_cache: Arc<Mutex<Option<TokenCache>>>,
}

impl std::fmt::Debug for ErnieNativeClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // 不打印 secrets
        f.debug_struct("ErnieNativeClient")
            .field("api_key", &"<redacted>")
            .field("secret_key", &"<redacted>")
            .field("model", &self.model)
            .field("has_token_cache", &self.token_cache.try_lock().map(|g| g.is_some()).unwrap_or(false))
            .finish()
    }
}

impl ErnieNativeClient {
    pub fn new(api_key: impl Into<String>, secret_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            secret_key: secret_key.into(),
            model: model.into(),
            token_cache: Arc::new(Mutex::new(None)),
        }
    }

    /// 从 keyring secret (`api_key:secret_key`) 解析成 client。
    /// v0.111.1 协议: 用 `splitn(2, ':')` 分隔 AK 和 SK。SK 可包含 `:`（base64 padding）。
    pub fn from_secret(secret: &str, model: impl Into<String>) -> Result<Self, String> {
        let mut parts = secret.splitn(2, ':');
        let ak = parts.next().ok_or("missing api_key")?;
        let sk = parts.next().ok_or("missing secret_key")?;
        Ok(Self::new(ak.to_string(), sk.to_string(), model))
    }

    /// 通过 OAuth2 client_credentials grant 拉取 access_token。
    /// 公开方法供测试用,生产中通过 `ensure_token` 间接调。
    pub async fn fetch_token(
        &self,
        http: &reqwest::Client,
    ) -> Result<TokenCache, CallError> {
        let url = format!(
            "{}?grant_type=client_credentials&client_id={}&client_secret={}",
            TOKEN_URL,
            urlencoding(&self.api_key),
            urlencoding(&self.secret_key),
        );
        let resp = http.get(&url).send().await.map_err(|e| CallError {
            http_status: None,
            code: err::NETWORK,
            message: e.to_string(),
        })?;
        let status = resp.status();
        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return Err(CallError {
                http_status: Some(status.as_u16()),
                code: err::AUTH,
                message: format!("HTTP {}", status.as_u16()),
            });
        }
        if !status.is_success() {
            return Err(CallError {
                http_status: Some(status.as_u16()),
                code: err::UNKNOWN,
                message: format!("token fetch HTTP {}: {}", status.as_u16(), resp.text().await.unwrap_or_default()),
            });
        }
        let body: TokenResponse = resp.json().await.map_err(|e| CallError {
            http_status: Some(status.as_u16()),
            code: err::PARSE,
            message: e.to_string(),
        })?;
        let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        Ok(TokenCache {
            access_token: body.access_token,
            fetched_at_unix: now,
            expires_in: body.expires_in,
        })
    }

    /// 确保 token 仍有效;缺失或过期时重新拉取。
    async fn ensure_token(&self, http: &reqwest::Client) -> Result<String, CallError> {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        {
            let guard = self.token_cache.lock().await;
            if let Some(cached) = guard.as_ref() {
                if !cached.is_expired(now) {
                    return Ok(cached.access_token.clone());
                }
            }
        }
        let fresh = self.fetch_token(http).await?;
        let token = fresh.access_token.clone();
        *self.token_cache.lock().await = Some(fresh);
        Ok(token)
    }

    /// 根据 LLM `CallRequest` 构建聊天请求体。
    /// 千帆 body 不一样 — `messages` 字段直接 array of `{role, content}`,不是 OpenAI
    /// 的 `{role, content: string}` 格式 (千帆是相同格式,OAI 也一样;差异在
    /// `temperature` 用 0.0-1.0 不一样)。
    pub fn build_chat_request_body(req: &CallRequest) -> ErnieChatRequest {
        // 千帆 system 走独立字段,从 messages 第一个 system message 抽出来。
        let (system_msg, rest_messages): (Option<String>, Vec<_>) = if let Some(first) = req.messages.first() {
            if first.role == "system" {
                (Some(first.content.clone()), req.messages[1..].to_vec())
            } else {
                (None, req.messages.clone())
            }
        } else {
            (None, Vec::new())
        };
        ErnieChatRequest {
            messages: rest_messages.iter().map(|m| ErnieMessage {
                role: m.role.clone(),
                content: m.content.clone(),
            }).collect(),
            temperature: Some(req.temperature as f64),
            top_p: Some(0.7),  // 千帆 default
            penalty_score: Some(1.0), // 千帆 default
            system: system_msg,
            user_id: None,
            max_output_tokens: Some(req.max_tokens.min(2048)),
        }
    }
}

#[async_trait::async_trait]
impl LlmClient for ErnieNativeClient {
    fn kind(&self) -> ProviderKind { ProviderKind::ErnieNative }

    async fn call(
        &self,
        http: &reqwest::Client,
        _secret: &str, // 不用 secret — 我们用 api_key + secret_key 在 self 里
        req: &CallRequest,
        cost: CostRate,
    ) -> Result<CallOutcome, CallError> {
        let token = self.ensure_token(http).await?;
        let url = format!("{}/{}?access_token={}", CHAT_BASE, urlencoding(&self.model), urlencoding(&token));
        let body = Self::build_chat_request_body(req);
        let start = std::time::Instant::now();
        let resp = http
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| CallError {
                http_status: None,
                code: err::NETWORK,
                message: e.to_string(),
            })?;
        let status = resp.status();
        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            // Token 可能过期,清空缓存让下次重试
            *self.token_cache.lock().await = None;
            return Err(CallError {
                http_status: Some(status.as_u16()),
                code: err::AUTH,
                message: format!("HTTP {} (token 过期?)", status.as_u16()),
            });
        }
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(CallError {
                http_status: Some(status.as_u16()),
                code: err::RATE_LIMIT,
                message: "qianfan 429".into(),
            });
        }
        if !status.is_success() {
            return Err(CallError {
                http_status: Some(status.as_u16()),
                code: err::UNKNOWN,
                message: format!("chat HTTP {}: {}", status.as_u16(), resp.text().await.unwrap_or_default()),
            });
        }
        let body: ErnieChatResponse = resp.json().await.map_err(|e| CallError {
            http_status: Some(status.as_u16()),
            code: err::PARSE,
            message: e.to_string(),
        })?;
        let latency_ms = start.elapsed().as_millis() as u64;
        let usage = body.usage.unwrap_or(ErnieUsage { prompt_tokens: 0, completion_tokens: 0, total_tokens: 0 });
        let cost_cents = cost.compute(usage.prompt_tokens, usage.completion_tokens);
        Ok(CallOutcome {
            http_status: status.as_u16(),
            latency_ms,
            tokens_in: usage.prompt_tokens,
            tokens_out: usage.completion_tokens,
            cost_cents,
            text: body.result,
            parsed: None,
            parse_ok: true,
            parse_error: None,
        })
    }
}

/// 简单 percent-encoding (千帆需要)。只 encode 字符集 `[A-Za-z0-9_.-~` 之外的。
/// 实际生产用 `percent-encoding` crate,但 spec 不让加新 dep,所以自己写。
pub fn urlencoding(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-' | '~') {
            out.push(c);
        } else {
            for b in c.to_string().as_bytes() {
                out.push_str(&format!("%{:02X}", b));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::llm::ChatMessage;

    #[test]
    fn parse_token_response_basic() {
        let json = r#"{"access_token":"abc.def.ghi","expires_in":2592000,"scope":"ai_custom_all"}"#;
        let r: TokenResponse = serde_json::from_str(json).unwrap();
        assert_eq!(r.access_token, "abc.def.ghi");
        assert_eq!(r.expires_in, 2592000);
        assert_eq!(r.scope, Some("ai_custom_all".into()));
    }

    #[test]
    fn parse_token_response_missing_expires_in_uses_default() {
        let json = r#"{"access_token":"x"}"#;
        let r: TokenResponse = serde_json::from_str(json).unwrap();
        assert_eq!(r.expires_in, 2592000); // 30 天 default
    }

    #[test]
    fn token_cache_is_expired_returns_true_past_ttl() {
        let cache = TokenCache {
            access_token: "x".into(),
            fetched_at_unix: 1000,
            expires_in: 100,
        };
        // age=200 + 86400 安全余量 = 86600 >= 100 → 过期
        assert!(cache.is_expired(1200));
    }

    #[test]
    fn token_cache_is_expired_returns_false_within_ttl() {
        let cache = TokenCache {
            access_token: "x".into(),
            fetched_at_unix: 1000,
            expires_in: 2592000,
        };
        // age=10 + 86400 安全余量 = 86410 < 2592000 → 未过期
        assert!(!cache.is_expired(1010));
    }

    #[test]
    fn parse_chat_response_with_result_field() {
        let json = r#"{"id":"abc","object":"chat.completion","created":1700000000,"result":"hello world","is_truncated":false,"usage":{"prompt_tokens":10,"completion_tokens":5,"total_tokens":15}}"#;
        let r: ErnieChatResponse = serde_json::from_str(json).unwrap();
        assert_eq!(r.result, "hello world");
        assert!(!r.is_truncated);
        let u = r.usage.unwrap();
        assert_eq!(u.prompt_tokens, 10);
        assert_eq!(u.completion_tokens, 5);
        assert_eq!(u.total_tokens, 15);
    }

    #[test]
    fn parse_chat_response_missing_usage_uses_zero() {
        let json = r#"{"result":"hi","is_truncated":true}"#;
        let r: ErnieChatResponse = serde_json::from_str(json).unwrap();
        assert_eq!(r.result, "hi");
        assert!(r.is_truncated);
        assert!(r.usage.is_none());
    }

    #[test]
    fn build_chat_request_body_basic() {
        let req = CallRequest::new("ernie-5.0")
            .user("hi")
            .max_tokens(512)
            .temperature(0.5);
        let body = ErnieNativeClient::build_chat_request_body(&req);
        assert_eq!(body.messages.len(), 1);
        assert_eq!(body.messages[0].role, "user");
        assert_eq!(body.messages[0].content, "hi");
        assert_eq!(body.temperature, Some(0.5));
        // max_tokens 被千帆 protocol 限制为 2048
        assert_eq!(body.max_output_tokens, Some(512));
    }

    #[test]
    fn build_chat_request_body_clamps_max_tokens_to_2048() {
        let req = CallRequest::new("ernie-5.0").max_tokens(10000);
        let body = ErnieNativeClient::build_chat_request_body(&req);
        // 千帆 protocol 上限为 2048
        assert_eq!(body.max_output_tokens, Some(2048));
    }

    #[test]
    fn build_chat_request_body_extracts_system_from_messages() {
        let req = CallRequest::new("ernie-5.0")
            .system("be polite")
            .user("hi");
        let body = ErnieNativeClient::build_chat_request_body(&req);
        assert_eq!(body.system, Some("be polite".into()));
        // system message 被移出 messages 列表
        assert_eq!(body.messages.len(), 1);
        assert_eq!(body.messages[0].role, "user");
    }

    #[test]
    fn urlencoding_basic() {
        assert_eq!(urlencoding("abc-123_~.~"), "abc-123_~.~");
        // 中文: 百度 → %E7%99%BE%E5%BA%A6
        assert_eq!(urlencoding("百度"), "%E7%99%BE%E5%BA%A6");
        // 空格 → %20
        assert_eq!(urlencoding("a b"), "a%20b");
    }

    #[test]
    fn from_secret_parses_ak_sk_format() {
        let c = ErnieNativeClient::from_secret("ak-123:sk-456", "ernie-5.0").unwrap();
        assert_eq!(c.api_key, "ak-123");
        assert_eq!(c.secret_key, "sk-456");
        assert_eq!(c.model, "ernie-5.0");
    }

    #[test]
    fn from_secret_rejects_missing_sk() {
        let r = ErnieNativeClient::from_secret("ak-123", "ernie-5.0");
        assert!(r.is_err());
        assert!(r.unwrap_err().contains("missing secret_key"));
    }

    #[test]
    fn from_secret_handles_sk_with_colon() {
        // splitn(2, ':') — SK 可以包含冒号(例如 base64 padding)
        let c = ErnieNativeClient::from_secret("ak:sk:abc:def", "m").unwrap();
        assert_eq!(c.api_key, "ak");
        assert_eq!(c.secret_key, "sk:abc:def");
    }

    #[test]
    fn debug_does_not_leak_secrets() {
        let c = ErnieNativeClient::new("ak-secret", "sk-secret", "ernie-5.0");
        let s = format!("{:?}", c);
        assert!(!s.contains("ak-secret"));
        assert!(!s.contains("sk-secret"));
        assert!(s.contains("<redacted>"));
    }
}
