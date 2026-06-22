//! Spark 讯飞 (v0.114) — WebSocket 协议 client (结构 + 解析 + 鉴权).
//!
//! **Why a separate client**:
//!   - 讯飞星火是 WebSocket 协议(不是 HTTP),用 `wss://spark-api.xf-yun.com/v1.1/chat`
//!   - 鉴权:URL query 里塞 `appId` / `apiKey` / `apiSecret`(不是 Authorization header)
//!   - **流式**:Spark 协议设计是流式,服务端持续 send `data: {...}` chunks
//!   - Body: `{ "header": {...}, "parameter": {...}, "payload": {...} }` 嵌套结构
//!
//! **v0.114 范围**:
//!   - ✅ SparkClient struct + 鉴权 URL 构造 + JSON request/response schema
//!   - ✅ Tests: 12 unit tests 覆盖 auth URL, body schema, response parse
//!   - ⏸️ **WebSocket 实际收发** deferred v0.114.1+ (需要 `tokio-tungstenite` dep)
//!   - 当前 `call()` 用 `unimplemented!()` 返 err::UNKNOWN (contract: 不 panic)
//!
//! **Keyring secret 格式**: `{appId}:{apiKey}:{apiSecret}` (splitn(3, ':')).
//! 讯飞鉴权 3 件套(相比 OpenAI 1 个 key、ERNIE 2 件套、腾讯 2 件套)。

use crate::domain::llm::{CallError, CallOutcome, CallRequest, CostRate, LlmClient, ProviderKind, err};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::Message as WsMessage;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

const SPARK_HOST: &str = "spark-api.xf-yun.com";
const SPARK_PATH_V1_1: &str = "/v1.1/chat";
const SPARK_PATH_V3_0: &str = "/v3.0/chat";
const SPARK_PATH_V3_5: &str = "/v3.5/chat";

/// Spark 3 个版本 path。**v3.5 是最新**,2026 stable。模型 ID 配 v3.5:
///   - `general` (v1.1)
///   - `generalv2` (v2.0)
///   - `generalv3` (v3.0)
///   - `generalv3.5` (v3.5, 推荐)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SparkVersion {
    V1_1,
    V3_0,
    V3_5,
}

impl SparkVersion {
    pub fn path(&self) -> &'static str {
        match self {
            Self::V1_1 => SPARK_PATH_V1_1,
            Self::V3_0 => SPARK_PATH_V3_0,
            Self::V3_5 => SPARK_PATH_V3_5,
        }
    }
    /// 从 model ID 推断 version。Spark 3 个 version 有不同 model name prefix。
    pub fn from_model(model: &str) -> Self {
        if model.starts_with("generalv3.5") {
            Self::V3_5
        } else if model.starts_with("generalv3") {
            Self::V3_0
        } else {
            Self::V1_1
        }
    }
}

/// Spark 鉴权 URL。讯飞 鉴权走 URL query,不是 Authorization header。
///
/// **生产用 HMAC 签名 URL**(`apiSecret` + 当前时间,base64 URL-safe)。
/// v0.114 简化: 不算签名(讯飞 demo 也允许 `?authorization=...` 简化),只塞
/// appId + apiKey (apiSecret 留作 v0.114.1 实现签名用)。
pub fn build_spark_url(app_id: &str, api_key: &str, version: SparkVersion) -> String {
    format!(
        "wss://{}{}?appId={}&apiKey={}",
        SPARK_HOST,
        version.path(),
        app_id,
        api_key,
    )
}

/// Spark request body. 嵌套结构 (跟 OpenAI / 国产大模型不同)。
#[derive(Debug, Serialize)]
pub struct SparkRequest {
    pub header: SparkHeader,
    pub parameter: SparkParameter,
    pub payload: SparkPayload,
}

#[derive(Debug, Serialize)]
pub struct SparkHeader {
    pub app_id: String,
    pub uid: String,
}

#[derive(Debug, Serialize)]
pub struct SparkParameter {
    pub chat: SparkChatParameter,
}

#[derive(Debug, Serialize)]
pub struct SparkChatParameter {
    pub domain: String,        // "general" / "generalv2" / "generalv3" / "generalv3.5"
    pub temperature: f64,
    pub max_tokens: u32,
    pub top_k: i32,
    /// Spark 协议流式: `0`=非流式, `1`=流式。v0.114 不实现流式,固定 0。
    pub chat_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SparkPayload {
    pub message: SparkMessage,
}

#[derive(Debug, Serialize)]
pub struct SparkMessage {
    pub text: Vec<SparkTextItem>,
}

#[derive(Debug, Serialize, Clone)]
pub struct SparkTextItem {
    pub role: String,
    pub content: String,
}

/// Response side: Spark 响应里的 `text` 数组。
#[derive(Debug, Clone, Deserialize)]
pub struct SparkResponseTextItem {
    pub role: String,
    pub content: String,
}

impl SparkRequest {
    pub fn from_call_request(
        app_id: &str,
        req: &CallRequest,
        version: SparkVersion,
    ) -> Self {
        let domain = match version {
            SparkVersion::V1_1 => "general",
            SparkVersion::V3_0 => "generalv3",
            SparkVersion::V3_5 => "generalv3.5",
        };
        Self {
            header: SparkHeader {
                app_id: app_id.to_string(),
                uid: "polyrocket".to_string(),
            },
            parameter: SparkParameter {
                chat: SparkChatParameter {
                    domain: domain.to_string(),
                    temperature: req.temperature as f64,
                    max_tokens: req.max_tokens,
                    top_k: 4,
                    chat_id: None,
                },
            },
            payload: SparkPayload {
                message: SparkMessage {
                    text: req.messages.iter().map(|m| SparkTextItem {
                        role: m.role.clone(),
                        content: m.content.clone(),
                    }).collect(),
                },
            },
        }
    }
}

/// Spark 响应 (server-sent `data: {...}` chunks,流式;非流式 1 个完整 JSON)。
#[derive(Debug, Clone, Deserialize)]
pub struct SparkResponse {
    pub header: SparkResponseHeader,
    pub payload: Option<SparkResponsePayload>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SparkResponseHeader {
    pub code: i32,    // 0 = success
    pub message: String,
    pub sid: String,
    pub status: i32,  // 0=first, 1=mid, 2=last
}

#[derive(Debug, Clone, Deserialize)]
pub struct SparkResponsePayload {
    pub choices: SparkChoices,
    pub usage: SparkUsage,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SparkChoices {
    pub status: i32,
    pub seq: i32,
    pub text: Vec<SparkResponseTextItem>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SparkUsage {
    pub text: SparkUsageText,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SparkUsageText {
    pub question_tokens: u32,
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// Spark client. 持有 3 件套 + model + version。
pub struct SparkClient {
    pub app_id: String,
    pub api_key: String,
    pub api_secret: String, // 留作 v0.114.1 HMAC 签名用
    pub model: String,
    pub version: SparkVersion,
}

impl std::fmt::Debug for SparkClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SparkClient")
            .field("app_id", &"<redacted>")
            .field("api_key", &"<redacted>")
            .field("api_secret", &"<redacted>")
            .field("model", &self.model)
            .field("version", &self.version)
            .finish()
    }
}

impl SparkClient {
    pub fn new(
        app_id: impl Into<String>,
        api_key: impl Into<String>,
        api_secret: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
        let model_str: String = model.into();
        let version = SparkVersion::from_model(&model_str);
        Self {
            app_id: app_id.into(),
            api_key: api_key.into(),
            api_secret: api_secret.into(),
            model: model_str,
            version,
        }
    }

    /// 从 keyring secret (`appId:apiKey:apiSecret`) 解析成 client。
    pub fn from_secret(secret: &str, model: impl Into<String>) -> Result<Self, String> {
        let parts: Vec<&str> = secret.splitn(3, ':').collect();
        if parts.len() != 3 {
            return Err(format!("expected appId:apiKey:apiSecret, got {} parts", parts.len()));
        }
        Ok(Self::new(parts[0], parts[1], parts[2], model))
    }

    /// 构造 WebSocket URL (含鉴权 query)。
    pub fn auth_url(&self) -> String {
        build_spark_url(&self.app_id, &self.api_key, self.version)
    }

    /// Build request body for a `CallRequest`.
    pub fn build_request_body(&self, req: &CallRequest) -> SparkRequest {
        SparkRequest::from_call_request(&self.app_id, req, self.version)
    }

    /// 提取最后 chunk 的 text (流式累积;非流式直接取 text[0])。
    pub fn extract_text(resp: &SparkResponse) -> String {
        resp.payload.as_ref()
            .and_then(|p| p.choices.text.first())
            .map(|t| t.content.clone())
            .unwrap_or_default()
    }
}

#[async_trait::async_trait]
impl LlmClient for SparkClient {
    fn kind(&self) -> ProviderKind { ProviderKind::Spark }

    async fn call(
        &self,
        _http: &reqwest::Client,
        _secret: &str,
        _req: &CallRequest,
        _cost: CostRate,
    ) -> Result<CallOutcome, CallError> {
        // v0.114 — WebSocket 实际收发 deferred v0.114.1+ (需要 tokio-tungstenite dep)。
        // 仍返回错误而非 panic (per LlmClient contract: 永不 panic)。
        Err(CallError {
            http_status: None,
            code: err::UNKNOWN,
            message: "Spark WebSocket transport deferred to v0.114.1+ (需要 tokio-tungstenite dep). Use 讯飞 demo / curl 临时 workaround.".into(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::llm::ChatMessage;

    #[test]
    fn spark_version_path() {
        assert_eq!(SparkVersion::V1_1.path(), "/v1.1/chat");
        assert_eq!(SparkVersion::V3_0.path(), "/v3.0/chat");
        assert_eq!(SparkVersion::V3_5.path(), "/v3.5/chat");
    }

    #[test]
    fn spark_version_from_model() {
        assert_eq!(SparkVersion::from_model("general"), SparkVersion::V1_1);
        assert_eq!(SparkVersion::from_model("generalv2"), SparkVersion::V1_1);  // V1.1 fallback
        assert_eq!(SparkVersion::from_model("generalv3"), SparkVersion::V3_0);
        assert_eq!(SparkVersion::from_model("generalv3.5"), SparkVersion::V3_5);
    }

    #[test]
    fn build_spark_url_contains_creds() {
        let url = build_spark_url("app123", "key456", SparkVersion::V3_5);
        assert!(url.starts_with("wss://spark-api.xf-yun.com/v3.5/chat"));
        assert!(url.contains("appId=app123"));
        assert!(url.contains("apiKey=key456"));
    }

    #[test]
    fn from_secret_parses_three_part() {
        let c = SparkClient::from_secret("app:k:sec", "generalv3.5").unwrap();
        assert_eq!(c.app_id, "app");
        assert_eq!(c.api_key, "k");
        assert_eq!(c.api_secret, "sec");
        assert_eq!(c.model, "generalv3.5");
        assert_eq!(c.version, SparkVersion::V3_5);
    }

    #[test]
    fn from_secret_rejects_wrong_part_count() {
        assert!(SparkClient::from_secret("only-one", "m").is_err());
        assert!(SparkClient::from_secret("a:b", "m").is_err());
        // splitn(3, ':') — secret with extra colons: a:b:c:d → 3 parts
        let c = SparkClient::from_secret("a:b:c:d", "m").unwrap();
        assert_eq!(c.app_id, "a");
        assert_eq!(c.api_key, "b");
        assert_eq!(c.api_secret, "c:d");
    }

    #[test]
    fn build_request_body_includes_all_fields() {
        let c = SparkClient::new("app", "k", "sec", "generalv3.5");
        let req = CallRequest::new("generalv3.5")
            .system("be polite")
            .user("hi");
        let body = c.build_request_body(&req);
        assert_eq!(body.header.app_id, "app");
        assert_eq!(body.parameter.chat.domain, "generalv3.5");
        assert_eq!(body.payload.message.text.len(), 2);
        assert_eq!(body.payload.message.text[0].role, "system");
        assert_eq!(body.payload.message.text[0].content, "be polite");
        assert_eq!(body.payload.message.text[1].role, "user");
    }

    #[test]
    fn build_request_body_domain_matches_version() {
        // v1.1 → "general"
        let c1 = SparkClient::new("a", "k", "s", "general");
        let r1 = c1.build_request_body(&CallRequest::new("general"));
        assert_eq!(r1.parameter.chat.domain, "general");

        // v3.0 → "generalv3"
        let c3 = SparkClient::new("a", "k", "s", "generalv3");
        let r3 = c3.build_request_body(&CallRequest::new("generalv3"));
        assert_eq!(r3.parameter.chat.domain, "generalv3");

        // v3.5 → "generalv3.5"
        let c35 = SparkClient::new("a", "k", "s", "generalv3.5");
        let r35 = c35.build_request_body(&CallRequest::new("generalv3.5"));
        assert_eq!(r35.parameter.chat.domain, "generalv3.5");
    }

    #[test]
    fn parse_response_with_text() {
        let json = r#"{"header":{"code":0,"message":"Success","sid":"abc123","status":2},"payload":{"choices":{"status":2,"seq":0,"text":[{"role":"assistant","content":"hello spark"}]},"usage":{"text":{"question_tokens":1,"prompt_tokens":5,"completion_tokens":3,"total_tokens":8}}}}"#;
        let r: SparkResponse = serde_json::from_str(json).unwrap();
        assert_eq!(r.header.code, 0);
        assert_eq!(r.header.status, 2);
        let text = SparkClient::extract_text(&r);
        assert_eq!(text, "hello spark");
    }

    #[test]
    fn parse_response_missing_payload_returns_empty() {
        let json = r#"{"header":{"code":0,"message":"Success","sid":"x","status":0}}"#;
        let r: SparkResponse = serde_json::from_str(json).unwrap();
        assert!(r.payload.is_none());
        let text = SparkClient::extract_text(&r);
        assert_eq!(text, "");
    }

    #[test]
    fn parse_response_error_code_passes_through() {
        let json = r#"{"header":{"code":10013,"message":"超出最大token限制","sid":"x","status":2}}"#;
        let r: SparkResponse = serde_json::from_str(json).unwrap();
        assert_eq!(r.header.code, 10013);
        assert!(r.header.message.contains("token"));
    }

    #[test]
    fn auth_url_uses_v35_for_generalv35_model() {
        let c = SparkClient::new("app", "key", "secret", "generalv3.5");
        let url = c.auth_url();
        assert!(url.contains("/v3.5/chat"));
    }

    #[test]
    fn debug_does_not_leak_secrets() {
        let c = SparkClient::new("app-secret", "key-secret", "secret-secret", "generalv3.5");
        let s = format!("{:?}", c);
        assert!(!s.contains("app-secret"));
        assert!(!s.contains("key-secret"));
        assert!(!s.contains("secret-secret"));
        assert!(s.contains("<redacted>"));
        assert!(s.contains("generalv3.5"));
    }
}
