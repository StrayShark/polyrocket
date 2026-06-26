//! L2 —— 钱包余额（v0.123）。
//!
//! IPC:get_wallet_balance —— 从 Polymarket CLOB API
//! 读取用户的 USDC（抵押品）余额。这是 v0.123 的简化：
//! 不再展示合成的 dashboard 小部件，而是直接呈现真实的
//! 链上余额（保留 2 位小数），便于用户确认 `.env`
//! 中的 L2 凭据是否正确接入。
//!
//! ## L2 HMAC 认证
//!
//! Polymarket CLOB 的 `GET /balance-allowance?asset_type=COLLATERAL`
//! 需要 L2（HMAC）认证，附带 4 个 header：
//!
//!   POLY_ADDRESS    —— L1 polygon 地址（EIP-55 校验和，不在 .env 中）
//!   POLY_API_KEY    —— UUID
//!   POLY_PASSPHRASE —— 钱包专属口令
//!   POLY_TIMESTAMP  —— 当前 unix 秒（须与签名时间一致）
//!   POLY_SIGNATURE  —— base64(HMAC-SHA256(secret, msg))
//!
//! 其中 `msg = timestamp + method + path + body`。
//!
//! `POLY_ADDRESS` 不在 `.env` 中（L1 私钥从不写入开发
//! 目录文件）。当它缺失时，我们回退到 `wallets.address`
//! 表中最新的一行 —— 也就是当初生成 L2 API key 时
//! 用到的那个地址。若此处也为空，则返回
//! `BalanceLookupError::NoAddress`，让 L1 可以展示
//! 友好的「请在 Settings 添加钱包地址」提示。

use crate::AppResult;
use crate::infra::http::new_http_client;
use serde::Serialize;
use tauri::State;

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

const CLOB_BASE: &str = "https://clob.polymarket.com";

/// v0.123 —— 余额查询结果。`ok=true` 表示 API 返回了余额；
/// `ok=false` 覆盖所有错误情形（凭据缺失、网络异常、
/// API 错误、L1 地址缺失等）。`reason` 字段是人类可读
/// 的字符串，便于 L1 直接展示给用户。
#[derive(Debug, Clone, Serialize)]
pub struct BalanceResult {
    pub ok: bool,
    /// USDC 抵押品余额，保留 2 位小数。出错时为 0。
    pub balance_usdc: f64,
    /// API 返回的原始余额字符串（供调试用）。
    /// 出错时为空。
    pub raw_balance: String,
    /// `ok=false` 时的自由文本原因。是稳定字符串，
    /// 便于 L1 进行匹配。
    pub reason: String,
    /// 当 .env 中的 L2 凭据存在时为 true（无论
    /// 实际的查询是否成功）。
    pub creds_present: bool,
}

/// IPC:get_wallet_balance —— 从 Polymarket CLOB
/// 通过 .env 中的 L2 凭据读取 USDC 余额。
///
/// **Returns**：`BalanceResult`（IPC 层面始终返回成功 —
/// 错误信息编码在 `ok` + `reason` 中）。
#[tauri::command]
pub async fn get_wallet_balance(state: State<'_, crate::infra::state::AppState>) -> AppResult<BalanceResult> {
    // 1) 从 .env 读取凭据（启动时已由
    //    `platform::env::maybe_load_dev_env` 注入）。
    let api_key = std::env::var("POLYMARKET_API_KEY")
        .ok()
        .filter(|v| !v.is_empty())
        .or_else(|| std::env::var("POLYROCKET_CLOB_API_KEY").ok().filter(|v| !v.is_empty()));
    let api_secret = std::env::var("POLYMARKET_API_SECRET")
        .ok()
        .filter(|v| !v.is_empty())
        .or_else(|| std::env::var("POLYROCKET_CLOB_API_SECRET").ok().filter(|v| !v.is_empty()));
    let passphrase = std::env::var("POLYMARKET_API_PASSPHRASE")
        .ok()
        .filter(|v| !v.is_empty())
        .or_else(|| std::env::var("POLYROCKET_CLOB_API_PASSPHRASE").ok().filter(|v| !v.is_empty()));

    let creds_present = matches!((&api_key, &api_secret, &passphrase),
        (Some(k), Some(s), Some(p)) if !k.is_empty() && !s.is_empty() && !p.is_empty());

    if !creds_present {
        return Ok(BalanceResult {
            ok: false,
            balance_usdc: 0.0,
            raw_balance: String::new(),
            reason: "POLYMARKET_API_KEY / _SECRET / _PASSPHRASE not set in .env".into(),
            creds_present: false,
        });
    }

    // 2) 从 wallets 表解析 L1 polygon 地址。
    //    引导行（id="wallet-bootstrap-pm"）的
    //    address 为空 —— 这是 v0.123 时的默认值。
    //    用户在 Settings 里填写后才能启用 Mode A
    //    （jump-to-PM）的下注。
    let address: Option<String> = sqlx::query_scalar(
        "SELECT address FROM wallets
         WHERE address IS NOT NULL AND address != ''
         ORDER BY created_at DESC
         LIMIT 1",
    )
    .fetch_optional(&state.db)
    .await?;

    let address = match address {
        Some(a) if !a.is_empty() => a,
        _ => {
            return Ok(BalanceResult {
                ok: false,
                balance_usdc: 0.0,
                raw_balance: String::new(),
                reason: "No L1 polygon address in wallets table — add one in Settings → Wallets".into(),
                creds_present: true,
            });
        }
    };

    // 3) 构造 L2 HMAC header。
    let path = "/balance-allowance";
    let query = "?asset_type=COLLATERAL";
    let ts = chrono::Utc::now().timestamp();
    let msg = format!("{ts}GET{path}{query}");
    let secret_bytes = match BASE64.decode(api_secret.as_deref().unwrap_or("")) {
        Ok(b) => b,
        Err(e) => {
            return Ok(BalanceResult {
                ok: false,
                balance_usdc: 0.0,
                raw_balance: String::new(),
                reason: format!("POLYMARKET_API_SECRET is not valid base64: {e}"),
                creds_present: true,
            });
        }
    };
    let mut mac = HmacSha256::new_from_slice(&secret_bytes)
        .expect("HMAC accepts any key length");
    mac.update(msg.as_bytes());
    let sig = BASE64.encode(mac.finalize().into_bytes());

    // 4) 请求 CLOB API。响应形态为
    //    `{ "balance": "1234.56", "allowance": "0" }`，
    //    balance 是人类单位的字符串。
    let url = format!("{CLOB_BASE}{path}{query}");
    let resp = match new_http_client()
        .get(&url)
        .header("POLY_ADDRESS", &address)
        .header("POLY_API_KEY", api_key.as_deref().unwrap_or(""))
        .header("POLY_PASSPHRASE", passphrase.as_deref().unwrap_or(""))
        .header("POLY_TIMESTAMP", ts.to_string())
        .header("POLY_SIGNATURE", sig)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            return Ok(BalanceResult {
                ok: false,
                balance_usdc: 0.0,
                raw_balance: String::new(),
                reason: format!("CLOB balance request failed: {e}"),
                creds_present: true,
            });
        }
    };

    if !resp.status().is_success() {
        let status = resp.status().as_u16();
        let body = resp.text().await.unwrap_or_default();
        return Ok(BalanceResult {
            ok: false,
            balance_usdc: 0.0,
            raw_balance: body,
            reason: format!("CLOB balance-allowance returned HTTP {status}"),
            creds_present: true,
        });
    }

    // 5) 解析 JSON。CLOB 返回的是人类可读单位的
    //    字符串余额，而不是原子单位。
    let body: serde_json::Value = match resp.json().await {
        Ok(v) => v,
        Err(e) => {
            return Ok(BalanceResult {
                ok: false,
                balance_usdc: 0.0,
                raw_balance: String::new(),
                reason: format!("CLOB balance response not JSON: {e}"),
                creds_present: true,
            });
        }
    };
    let raw = body
        .get("balance")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let balance: f64 = raw.parse().unwrap_or(0.0);
    // 四舍五入保留 2 位小数（USDC 链上有 6 位小数，
    // 但 PM 界面展示 2 位 —— 与 polyrocket 其他位置一致）。
    let balance_rounded = (balance * 100.0).round() / 100.0;

    Ok(BalanceResult {
        ok: true,
        balance_usdc: balance_rounded,
        raw_balance: raw,
        reason: String::new(),
        creds_present: true,
    })
}
