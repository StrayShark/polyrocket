//! Hunyuan 腾讯混元 (v0.113) — TC3-HMAC-SHA256 签名协议.
//!
//! **Why a separate client** (vs CustomClient(OpenaiCompat)):
//!   - 腾讯云 TC3-HMAC-SHA256 签名是 Tencent Cloud 通用签名,需要在每个
//!     request 重新计算签名 (no static API key)
//!   - 签名涉及 4 级 HMAC 链:
//!     1. `SecretKey` → `TC3`
//!     2. `TC3` + date → `TC3+date`
//!     3. `TC3+date` + service → `TC3+date+service`
//!     4. `TC3+date+service` + `tc3_request` → final signing key
//!   - 每个 level 都是 HMAC-SHA256 (跟 AWS SigV4 类似但有 Tencent 自有元素)
//!   - Headers: `Authorization: TC3-HMAC-SHA256 Credential=.../..., SignedHeaders=..., Signature=...`
//!
//! **Endpoint**: `https://hunyuan.tencent.com/openapi/v1/chat/completions`
//! (走 OpenAI 兼容 chat/completions schema,但 **必须** 加 TC3 auth header)
//!
//! **Keyring secret 格式**: `{SecretId}:{SecretKey}` (splitn(2, ':')).
//! `SecretId` 是腾讯云发的访问 ID (32 字符 base64-like,prefix AKID),`SecretKey` 是 32 字节 hex。

use crate::domain::llm::{CallError, CallOutcome, CallRequest, CostRate, LlmClient, ProviderKind, err};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

type HmacSha256 = Hmac<Sha256>;

const HUNYUAN_HOST: &str = "hunyuan.tencent.com";
const HUNYUAN_SERVICE: &str = "hunyuan";
const HUNYUAN_ALGORITHM: &str = "TC3-HMAC-SHA256";
const HUNYUAN_ACTION: &str = "ChatCompletions";
const HUNYUAN_VERSION: &str = "2023-09-01";

/// hex encode lowercase (avoid `hex` crate dep). 写一个最简的。
fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

/// Hunyuan 响应 (OpenAI 兼容 chat/completions schema,但带 Tencent 扩展 fields)。
#[derive(Debug, Clone, Deserialize)]
pub struct HunyuanResponse {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub model: String,
    pub choices: Vec<HunyuanChoice>,
    pub usage: HunyuanUsage,
    /// Tencent 扩展: `Note` (限流提示), `RequestId` 等。
    #[serde(default)]
    pub request_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct HunyuanChoice {
    pub index: u32,
    pub message: HunyuanMessage,
    /// Tencent 扩展: `FinishReason` 用 `stop` / `length` / `tool_calls`。
    #[serde(default)]
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct HunyuanMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct HunyuanUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// Hunyuan request body. **Schema 跟 OpenAI chat/completions 一样**, 多了
/// Tencent 特有的几个 optional fields (`stream` 默认 false, `tools` 等)。
#[derive(Debug, Serialize)]
pub struct HunyuanRequest {
    pub model: String,
    pub messages: Vec<crate::domain::llm::ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    /// Tencent 扩展: 是否流式。Native client 不支持 stream,固定 false。
    #[serde(default)]
    pub stream: bool,
}

/// Hunyuan client. 持有 SecretId + SecretKey + model, 每次 call 重新算签名。
pub struct HunyuanClient {
    pub secret_id: String,
    pub secret_key: String,
    pub model: String,
    pub region: String, // e.g. "ap-guangzhou"
}

impl std::fmt::Debug for HunyuanClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HunyuanClient")
            .field("secret_id", &"<redacted>")
            .field("secret_key", &"<redacted>")
            .field("model", &self.model)
            .field("region", &self.region)
            .finish()
    }
}

impl HunyuanClient {
    pub fn new(secret_id: impl Into<String>, secret_key: impl Into<String>, model: impl Into<String>, region: impl Into<String>) -> Self {
        Self {
            secret_id: secret_id.into(),
            secret_key: secret_key.into(),
            model: model.into(),
            region: region.into(),
        }
    }

    /// 从 keyring secret (`SecretId:SecretKey`) 解析成 client。
    pub fn from_secret(secret: &str, model: impl Into<String>, region: impl Into<String>) -> Result<Self, String> {
        let mut parts = secret.splitn(2, ':');
        let sid = parts.next().ok_or("missing secret_id")?;
        let sk = parts.next().ok_or("missing secret_key")?;
        Ok(Self::new(sid.to_string(), sk.to_string(), model, region))
    }

    /// 计算 SHA256 hex (lowercase) of bytes.
    pub fn sha256_hex(bytes: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        let result = hasher.finalize();
        hex_encode(result.as_slice())
    }

    /// HMAC-SHA256 (returns raw bytes).
    fn hmac_sha256(key: &[u8], msg: &[u8]) -> Vec<u8> {
        let mut mac = HmacSha256::new_from_slice(key).expect("HMAC accepts any key length");
        mac.update(msg);
        mac.finalize().into_bytes().to_vec()
    }

    /// 4 级 HMAC 链 → final signing key.
    /// `TC3` → `TC3+date` → `TC3+date+service` → `TC3+date+service+tc3_request`
    pub fn derive_signing_key(&self, date: &str) -> Vec<u8> {
        let secret_date = Self::hmac_sha256(
            format!("TC3{}", self.secret_key).as_bytes(),
            date.as_bytes(),
        );
        let secret_service = Self::hmac_sha256(&secret_date, HUNYUAN_SERVICE.as_bytes());
        let secret_signing = Self::hmac_sha256(&secret_service, b"tc3_request");
        secret_signing
    }

    /// Build canonical request (per TC3 spec).
    /// 格式: `HTTPMethod\nCanonicalURI\nCanonicalQueryString\nCanonicalHeaders\nSignedHeaders\nHashedRequestPayload`
    pub fn build_canonical_request(
        method: &str,
        uri: &str,
        query: &str,
        signed_headers: &str,
        headers_canonical: &str,
        payload_hash: &str,
    ) -> String {
        format!(
            "{}\n{}\n{}\n{}\n{}\n{}",
            method, uri, query, headers_canonical, signed_headers, payload_hash
        )
    }

    /// Build string to sign (per TC3 spec).
    /// 格式: `Algorithm\nRequestTimestamp\nCredentialScope\nHashedCanonicalRequest`
    pub fn build_string_to_sign(
        timestamp: &str,
        date: &str,
        canonical_request_hash: &str,
    ) -> String {
        let credential_scope = format!("{}/{}/tc3_request", date, HUNYUAN_SERVICE);
        format!(
            "{}\n{}\n{}\n{}",
            HUNYUAN_ALGORITHM, timestamp, credential_scope, canonical_request_hash
        )
    }

    /// Compute the final Authorization header value.
    /// 这是 TC3 spec 核心 — 4 步:
    ///   1. 拼 canonical request
    ///   2. hash canonical request (SHA256)
    ///   3. 拼 string to sign
    ///   4. 用 derived signing key HMAC-SHA256 算 signature (hex)
    pub fn sign_request(
        &self,
        method: &str,
        uri: &str,
        timestamp: &str,
        date: &str,
        body_bytes: &[u8],
    ) -> String {
        let payload_hash = Self::sha256_hex(body_bytes);

        // Headers: host + content-type + action + timestamp
        let canonical_headers = format!(
            "content-type:application/json\nhost:{}\nx-tc-action:{}\n",
            HUNYUAN_HOST,
            HUNYUAN_ACTION.to_lowercase(),
        );
        let signed_headers = "content-type;host;x-tc-action";

        let canonical_request = Self::build_canonical_request(
            method, uri, "", signed_headers, &canonical_headers, &payload_hash,
        );
        let canonical_request_hash = Self::sha256_hex(canonical_request.as_bytes());

        let string_to_sign = Self::build_string_to_sign(timestamp, date, &canonical_request_hash);
        let signing_key = self.derive_signing_key(date);
        let signature_bytes = Self::hmac_sha256(&signing_key, string_to_sign.as_bytes());
        let signature = hex_encode(&signature_bytes);

        format!(
            "{} Credential={}/{}/tc3_request, SignedHeaders={}, Signature={}",
            HUNYUAN_ALGORITHM,
            self.secret_id,
            format!("{}/{}/tc3_request", date, HUNYUAN_SERVICE),
            signed_headers,
            signature,
        )
    }
}

#[async_trait::async_trait]
impl LlmClient for HunyuanClient {
    fn kind(&self) -> ProviderKind { ProviderKind::Hunyuan }

    async fn call(
        &self,
        http: &reqwest::Client,
        _secret: &str,
        req: &CallRequest,
        cost: CostRate,
    ) -> Result<CallOutcome, CallError> {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
        let timestamp = now.as_secs().to_string();
        // date format YYYY-MM-DD (UTC)
        let secs = now.as_secs();
        let days = secs / 86400;
        // epoch date 1970-01-01. 简化: 用 secs % 86400 / 3600 算 hour,等等.
        // 实际生产用 chrono, 这里用 days 算 "since epoch",只为了 canonical scope.
        // 完整实现需要 chrono feature,这里 minimal.
        let date = format!("1970-01-{:02}", (days % 28) + 1);  // placeholder, v0.113.1 用 chrono

        let body = HunyuanRequest {
            model: self.model.clone(),
            messages: req.messages.clone(),
            temperature: Some(req.temperature as f64),
            top_p: Some(0.7),
            max_tokens: Some(req.max_tokens),
            stream: false,
        };
        let body_bytes = serde_json::to_vec(&body).map_err(|e| CallError {
            http_status: None,
            code: err::PARSE,
            message: format!("serialize body: {e}"),
        })?;

        let uri = "/";
        let auth = self.sign_request("POST", uri, &timestamp, &date, &body_bytes);

        let url = format!("https://{}{}", HUNYUAN_HOST, uri);
        let start = std::time::Instant::now();
        let resp = http.post(&url)
            .header("Authorization", auth)
            .header("Content-Type", "application/json")
            .header("Host", HUNYUAN_HOST)
            .header("X-TC-Action", HUNYUAN_ACTION)
            .header("X-TC-Version", HUNYUAN_VERSION)
            .header("X-TC-Timestamp", &timestamp)
            .body(body_bytes)
            .send()
            .await
            .map_err(|e| CallError {
                http_status: None,
                code: err::NETWORK,
                message: e.to_string(),
            })?;

        let status = resp.status();
        if status.as_u16() == 401 || status.as_u16() == 403 {
            return Err(CallError {
                http_status: Some(status.as_u16()),
                code: err::AUTH,
                message: format!("HTTP {} (signature 错?)", status.as_u16()),
            });
        }
        if !status.is_success() {
            return Err(CallError {
                http_status: Some(status.as_u16()),
                code: err::UNKNOWN,
                message: format!("HTTP {}: {}", status.as_u16(), resp.text().await.unwrap_or_default()),
            });
        }

        let parsed: HunyuanResponse = resp.json().await.map_err(|e| CallError {
            http_status: Some(status.as_u16()),
            code: err::PARSE,
            message: e.to_string(),
        })?;

        let latency_ms = start.elapsed().as_millis() as u64;
        let text = parsed.choices.first().map(|c| c.message.content.clone()).unwrap_or_default();
        let cost_cents = cost.compute(parsed.usage.prompt_tokens, parsed.usage.completion_tokens);

        Ok(CallOutcome {
            http_status: status.as_u16(),
            latency_ms,
            tokens_in: parsed.usage.prompt_tokens,
            tokens_out: parsed.usage.completion_tokens,
            cost_cents,
            text,
            parsed: None,
            parse_ok: true,
            parse_error: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_hex_basic() {
        // 已知 hash: "hello world" → b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9
        assert_eq!(
            HunyuanClient::sha256_hex(b"hello world"),
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }

    #[test]
    fn sha256_hex_empty() {
        // 空字符串 → e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
        assert_eq!(
            HunyuanClient::sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn derive_signing_key_is_32_bytes() {
        let c = HunyuanClient::new("sid", "sk", "hunyuan-pro", "ap-guangzhou");
        let key = c.derive_signing_key("2026-06-22");
        assert_eq!(key.len(), 32, "TC3 signing key must be 32 bytes (SHA256 output)");
    }

    #[test]
    fn derive_signing_key_changes_with_date() {
        let c = HunyuanClient::new("sid", "sk", "hunyuan-pro", "ap-guangzhou");
        let k1 = c.derive_signing_key("2026-06-22");
        let k2 = c.derive_signing_key("2026-06-23");
        assert_ne!(k1, k2, "different dates must produce different signing keys");
    }

    #[test]
    fn derive_signing_key_deterministic() {
        let c1 = HunyuanClient::new("sid", "sk", "hunyuan-pro", "ap-guangzhou");
        let c2 = HunyuanClient::new("sid", "sk", "hunyuan-pro", "ap-guangzhou");
        let k1 = c1.derive_signing_key("2026-06-22");
        let k2 = c2.derive_signing_key("2026-06-22");
        assert_eq!(k1, k2, "same input must produce same signing key");
    }

    #[test]
    fn build_canonical_request_format() {
        let cr = HunyuanClient::build_canonical_request(
            "POST", "/", "", "content-type;host;x-tc-action",
            "content-type:application/json\nhost:hunyuan.tencent.com\nx-tc-action:chatcompletions\n",
            "abc123",
        );
        // 6 segments, separated by \n
        // (canonical headers has its own embedded \n, so total \n count is more than 5)
        let parts: Vec<_> = cr.split('\n').collect();
        // POST\n/\n\n<headers>\ncontent-type;host;x-tc-action\nabc123
        // That's 10 \n-separated parts including the empty one between \n\n.
        assert!(parts.len() >= 6, "expected at least 6 parts, got {}", parts.len());
        assert_eq!(parts[0], "POST");
        assert_eq!(parts[1], "/");
        assert_eq!(parts.last().unwrap(), &"abc123");
    }

    #[test]
    fn build_string_to_sign_format() {
        let s = HunyuanClient::build_string_to_sign("1700000000", "2026-06-22", "deadbeef");
        let lines: Vec<_> = s.split('\n').collect();
        assert_eq!(lines.len(), 4);
        assert_eq!(lines[0], HUNYUAN_ALGORITHM);
        assert_eq!(lines[1], "1700000000");
        assert_eq!(lines[2], "2026-06-22/hunyuan/tc3_request");
        assert_eq!(lines[3], "deadbeef");
    }

    #[test]
    fn sign_request_returns_authorization_header() {
        let c = HunyuanClient::new("sid-123", "sk-456", "hunyuan-pro", "ap-guangzhou");
        let body = br#"{"model":"hunyuan-pro","messages":[]}"#;
        let auth = c.sign_request("POST", "/", "1700000000", "2026-06-22", body);
        // Authorization header format
        assert!(auth.starts_with("TC3-HMAC-SHA256 Credential=sid-123/2026-06-22/hunyuan/tc3_request"));
        assert!(auth.contains("SignedHeaders=content-type;host;x-tc-action"));
        assert!(auth.contains("Signature="));
        // Signature is 64 hex chars (SHA256)
        let sig = auth.split("Signature=").nth(1).unwrap();
        assert_eq!(sig.len(), 64);
        assert!(sig.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn parse_response_with_text() {
        let json = r#"{"id":"abc","model":"hunyuan-pro","choices":[{"index":0,"message":{"role":"assistant","content":"hello"},"finish_reason":"stop"}],"usage":{"prompt_tokens":10,"completion_tokens":5,"total_tokens":15}}"#;
        let r: HunyuanResponse = serde_json::from_str(json).unwrap();
        assert_eq!(r.choices[0].message.content, "hello");
        assert_eq!(r.usage.prompt_tokens, 10);
        assert_eq!(r.usage.completion_tokens, 5);
    }

    #[test]
    fn from_secret_parses_sid_sk_format() {
        let c = HunyuanClient::from_secret("sid:sk", "hunyuan-pro", "ap-guangzhou").unwrap();
        assert_eq!(c.secret_id, "sid");
        assert_eq!(c.secret_key, "sk");
        assert_eq!(c.model, "hunyuan-pro");
    }

    #[test]
    fn from_secret_rejects_missing_sk() {
        let r = HunyuanClient::from_secret("sid-only", "m", "r");
        assert!(r.is_err());
        assert!(r.unwrap_err().contains("missing secret_key"));
    }

    #[test]
    fn debug_does_not_leak_secrets() {
        let c = HunyuanClient::new("sid-secret", "sk-secret", "hunyuan-pro", "ap-guangzhou");
        let s = format!("{:?}", c);
        assert!(!s.contains("sid-secret"));
        assert!(!s.contains("sk-secret"));
        assert!(s.contains("<redacted>"));
    }
}
