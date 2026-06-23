//! L2 — Wallet balance (v0.123).
//!
//! IPC: `get_wallet_balance` —— read the user's USDC (collateral)
//! balance from Polymarket's CLOB API. This is the v0.123 simplification:
//! instead of a synthetic dashboard widget, surface the real on-chain
//! balance (rounded to 2dp) so the user can confirm `.env` L2 creds
//! are correctly wired.
//!
//! ## L2 HMAC auth
//!
//! Polymarket's CLOB `GET /balance-allowance?asset_type=COLLATERAL`
//! requires L2 (HMAC) authentication with 4 headers:
//!
//!   POLY_ADDRESS    — L1 polygon address (EIP-55 checksum, not in .env)
//!   POLY_API_KEY    — UUID
//!   POLY_PASSPHRASE — wallet-specific passphrase
//!   POLY_TIMESTAMP  — current unix seconds (must match signed time)
//!   POLY_SIGNATURE  — base64(HMAC-SHA256(secret, msg))
//!
//! where `msg = timestamp + method + path + body`.
//!
//! `POLY_ADDRESS` is not in `.env` (L1 keys never live in dev
//! files). When it's missing, we degrade to the most-recent
//! `wallets.address` row — the address that was used to
//! generate the L2 API key. If that's also empty, we return
//! `BalanceLookupError::NoAddress` so the L1 can show a
//! friendly "Add wallet address in Settings" prompt.

use crate::AppResult;
use crate::infra::http::new_http_client;
use serde::Serialize;
use tauri::State;

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

const CLOB_BASE: &str = "https://clob.polymarket.com";

/// v0.123 — balance lookup outcome. `ok=true` means the API
/// returned a balance; `ok=false` covers all error shapes
/// (creds missing, network down, API error, missing L1
/// address, etc.). The `reason` field is human-readable
/// for the L1 to surface.
#[derive(Debug, Clone, Serialize)]
pub struct BalanceResult {
    pub ok: bool,
    /// USDC collateral balance, rounded to 2dp. 0 on error.
    pub balance_usdc: f64,
    /// Original raw balance string from the API (for
    /// debugging). Empty on error.
    pub raw_balance: String,
    /// Free-form reason when ok=false. Stable copy so the
    /// L1 can match on it.
    pub reason: String,
    /// True when the .env L2 creds are present (regardless
    /// of whether the lookup succeeded).
    pub creds_present: bool,
}

/// IPC: `get_wallet_balance` —— read USDC balance from
/// Polymarket CLOB using the .env L2 credentials.
///
/// **Returns**: `BalanceResult` (always succeeds at the IPC
/// level — errors are encoded in `ok` + `reason`).
#[tauri::command]
pub async fn get_wallet_balance(state: State<'_, crate::infra::state::AppState>) -> AppResult<BalanceResult> {
    // 1) Read creds from .env (already injected by
    //    platform::env::maybe_load_dev_env on boot).
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

    // 2) Resolve the L1 polygon address from the wallets
    //    table. The bootstrap row (id="wallet-bootstrap-pm")
    //    has an empty address — that's the v0.123 default.
    //    The user can populate it via Settings once they want
    //    Mode A (jump-to-PM) betting.
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

    // 3) Build the L2 HMAC headers.
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

    // 4) Hit the CLOB API. The shape of the response is
    //    `{ "balance": "1234.56", "allowance": "0" }`
    //    where balance is a string in human units.
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

    // 5) Parse the JSON. CLOB returns the balance as a
    //    string in human units, not atomic units.
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
    // Round to 2dp (USDC has 6 decimals on-chain but PM UI
    // renders 2dp — same as the rest of polyrocket).
    let balance_rounded = (balance * 100.0).round() / 100.0;

    Ok(BalanceResult {
        ok: true,
        balance_usdc: balance_rounded,
        raw_balance: raw,
        reason: String::new(),
        creds_present: true,
    })
}
