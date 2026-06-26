//! Hunyuan 腾讯混元（v0.113）—— TC3-HMAC-SHA256 签名协议。
//!
//! **为什么单独建一个客户端**（vs CustomClient(OpenaiCompat)）：
//!   - 腾讯云 TC3-HMAC-SHA256 签名是 Tencent Cloud 通用签名，需要在每个
//!     request 重新计算签名（无静态 API key）
//!   - 签名涉及 4 级 HMAC 链：
//!     1. `SecretKey` → `TC3`
//!     2. `TC3` + date → `TC3+date`
//!     3. `TC3+date` + service → `TC3+date+service`
//!     4. `TC3+date+service` + `tc3_request` → 最终 signing key
//!   - 每个 level 都是 HMAC-SHA256（与 AWS SigV4 类似但有 Tencent 自有元素）
//!   - Headers: `Authorization: TC3-HMAC-SHA256 Credential=.../..., SignedHeaders=..., Signature=...`
//!
//! **端点**：`https://hunyuan.tencent.com/openapi/v1/chat/completions`
//! （走 OpenAI 兼容 chat/completions schema，但 **必须** 加 TC3 auth header）
//!
//! **Keyring secret 格式**：`{SecretId}:{SecretKey}`（splitn(2, ':')）。
//! `SecretId` 是腾讯云发的访问 ID（32 字符 base64-like，prefix AKID），
//! `SecretKey` 是 32 字节 hex。

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

/// hex 编码（输出小写），避免引入 `hex` crate 依赖。写一个最简的实现。
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

/// Hunyuan 请求体。**Schema 跟 OpenAI chat/completions 一样**，多了
/// Tencent 特有的几个 optional 字段（`stream` 默认 false、`tools` 等）。
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

/// Hunyuan 客户端。持有 SecretId + SecretKey + model，每次调用时重新计算签名。
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

    /// 计算字节的 SHA256 十六进制字符串（小写）。
    pub fn sha256_hex(bytes: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        let result = hasher.finalize();
        hex_encode(result.as_slice())
    }

    /// HMAC-SHA256（返回原始字节）。
    fn hmac_sha256(key: &[u8], msg: &[u8]) -> Vec<u8> {
        let mut mac = HmacSha256::new_from_slice(key).expect("HMAC accepts any key length");
        mac.update(msg);
        mac.finalize().into_bytes().to_vec()
    }

    /// 4 级 HMAC 链 → 最终签名 key。
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

    /// 按 TC3 规范构建规范化请求。
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

    /// 构造待签名字符串（按 TC3 规范）。
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

    /// 将 unix 时间戳(秒)转换为 UTC 日期字符串 `YYYY-MM-DD`。
    /// v0.113.1 — 用算法实现避免 chrono feature dep。
    /// (年-月-日 Gregorian calendar,1970 epoch)
    pub fn date_string_unix(unix_secs: i64) -> String {
        // 自 1970-01-01 起的天数
        let days = unix_secs.div_euclid(86400);
        // Howard Hinnant 的 Civil-from-days 算法 (public domain)。
        // 参考: http://howardhinnant.github.io/date_algorithms.html
        let z = days + 719468;
        let era = if z >= 0 { z } else { z - 146096 } / 146097;
        let doe = (z - era * 146097) as u64;
        let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
        let y = yoe as i64 + era * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = doy - (153 * mp + 2) / 5 + 1;
        let m = if mp < 10 { mp + 3 } else { mp - 9 };
        let y = if m <= 2 { y + 1 } else { y };
        format!("{:04}-{:02}-{:02}", y, m, d)
    }

    /// 计算最终的 Authorization header 值。
    /// 这是 TC3 规范的核心 —— 4 步:
    ///   1. 拼 canonical request（拼装规范化请求）
    ///   2. hash canonical request (SHA256)（SHA256 哈希规范化请求）
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

        // Headers: host + content-type + action + timestamp（腾讯云规范要求）
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
        // v0.113.1 — 使用算法计算规范的日期格式 (YYYY-MM-DD, UTC)。
        // 生产环境: TC3 spec 要求 CredentialScope 中日期精确。
        let date = Self::date_string_unix(now.as_secs() as i64);

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
        // 6 段,以 \n 分隔
        // (canonical headers 自带嵌入的 \n,所以总 \n 数 > 5)
        let parts: Vec<_> = cr.split('\n').collect();
        // POST\n/\n\n<headers>\ncontent-type;host;x-tc-action\nabc123
        // （POST、空路径、空 query、规范化 headers、签名 headers、负载哈希）
        // 共 10 个 \n 分隔的部分,包括 \n\n 之间的空段。
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
        let c = HunyuanClient::new("sid-test", "sk-test", "hunyuan-pro", "ap-guangzhou");
        let body = br#"{"model":"hunyuan-pro","messages":[]}"#;
        let auth = c.sign_request("POST", "/", "1700000000", "2026-06-22", body);
        // Authorization header 格式
        assert!(auth.starts_with("TC3-HMAC-SHA256 Credential=sid-test/2026-06-22/hunyuan/tc3_request"));
        assert!(auth.contains("SignedHeaders=content-type;host;x-tc-action"));
        assert!(auth.contains("Signature="));
        // Signature 是 64 个十六进制字符 (SHA256)
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
        let c = HunyuanClient::new("sid-test", "sk-test", "hunyuan-pro", "ap-guangzhou");
        let s = format!("{:?}", c);
        assert!(!s.contains("sid-test"));
        assert!(!s.contains("sk-test"));
        assert!(s.contains("<redacted>"));
    }

    // v0.113.1 — date_string_unix 测试 (取代 chrono 占位实现)
    #[test]
    fn date_string_unix_epoch_is_1970_01_01() {
        assert_eq!(HunyuanClient::date_string_unix(0), "1970-01-01");
    }

    #[test]
    fn date_string_unix_one_day_later() {
        assert_eq!(HunyuanClient::date_string_unix(86400), "1970-01-02");
    }

    #[test]
    fn date_string_unix_year_2000() {
        // 2000-01-01 = 946684800 unix 秒（已知基准值）
        assert_eq!(HunyuanClient::date_string_unix(946684800), "2000-01-01");
    }

    #[test]
    fn date_string_unix_year_2024_leap_day() {
        // 2024-02-29 = 1709164800 unix 秒（闰年闰日）
        assert_eq!(HunyuanClient::date_string_unix(1709164800), "2024-02-29");
    }

    #[test]
    fn date_string_unix_year_2026_current() {
        // 2026-06-22 = 1782144000 unix 秒（当前日期约值）
        let d = HunyuanClient::date_string_unix(1782144000);
        assert!(d.starts_with("2026-"));
    }

    #[test]
    fn date_string_unix_negative_pre_epoch() {
        // 1969-12-31 = -86400 unix 秒（epoch 之前）
        assert_eq!(HunyuanClient::date_string_unix(-86400), "1969-12-31");
    }
}
