//! M11 — Secret persistence (Polymarket CLOB + wallet private key)
//!
//! These commands are the *client-paste* path. The user enters the secret
//! in the Settings UI; the secret is written straight to the OS keyring
//! by these handlers and **never** stored in SQLite, in JSON files, or
//! in the .env file. Audit-log entries are written without the secret.
//!
//! See `keyring.rs` for the alias conventions.

use crate::AppResult;
use crate::state::AppState;
use serde::{Deserialize, Serialize};
use tauri::State;

// ---------- Polymarket CLOB ----------

#[derive(Debug, Deserialize)]
pub struct PmSetCredentialsArgs {
    pub api_key: String,
    pub api_secret: String,
    pub api_passphrase: String,
    pub host: Option<String>,
    pub chain_id: Option<i64>,
}

#[tauri::command]
pub async fn llm_pm_set_credentials(
    state: State<'_, AppState>,
    args: PmSetCredentialsArgs,
) -> AppResult<()> {
    if args.api_key.trim().is_empty()
        || args.api_secret.trim().is_empty()
        || args.api_passphrase.trim().is_empty()
    {
        return Err(crate::AppError::Invalid(
            "all three PM CLOB fields are required".into(),
        ));
    }
    crate::platform::keyring::set_key(crate::platform::keyring::pm_api_alias(), args.api_key.trim())?;
    crate::platform::keyring::set_key(crate::platform::keyring::pm_secret_alias(), args.api_secret.trim())?;
    crate::platform::keyring::set_key(
        crate::platform::keyring::pm_passphrase_alias(),
        args.api_passphrase.trim(),
    )?;

    // Persist host / chain_id in the wallets row (key 0) for convenience;
    // the secrets themselves stay in keyring.
    if let Some(host) = args.host.as_deref() {
        if !host.is_empty() {
            sqlx::query("INSERT OR REPLACE INTO _polyrocket_settings (k, v) VALUES ('pm_host', ?)")
                .bind(host)
                .execute(&state.db)
                .await?;
        }
    }
    if let Some(chain) = args.chain_id {
        sqlx::query("INSERT OR REPLACE INTO _polyrocket_settings (k, v) VALUES ('pm_chain_id', ?)")
            .bind(chain.to_string())
            .execute(&state.db)
            .await?;
    }

    sqlx::query(
        "INSERT INTO audit_log (actor, action, target, payload, result) VALUES ('user', 'pm.credentials.set', 'polyrocket/pm', ?, 'ok')",
    )
    .bind(serde_json::json!({
        "host": args.host,
        "chain_id": args.chain_id,
        "lengths": {
            "api_key": args.api_key.len(),
            "api_secret": args.api_secret.len(),
            "api_passphrase": args.api_passphrase.len(),
        }
    }))
    .execute(&state.db)
    .await?;
    Ok(())
}

#[tauri::command]
pub async fn llm_pm_clear_credentials(state: State<'_, AppState>) -> AppResult<()> {
    for alias in [
        crate::platform::keyring::pm_api_alias(),
        crate::platform::keyring::pm_secret_alias(),
        crate::platform::keyring::pm_passphrase_alias(),
    ] {
        let _ = crate::platform::keyring::delete_key(alias);
    }
    sqlx::query("DELETE FROM _polyrocket_settings WHERE k IN ('pm_host', 'pm_chain_id')")
        .execute(&state.db)
        .await?;
    sqlx::query(
        "INSERT INTO audit_log (actor, action, target, result) VALUES ('user', 'pm.credentials.clear', 'polyrocket/pm', 'ok')",
    )
    .execute(&state.db)
    .await?;
    Ok(())
}

// ---------- Wallet (mode B signed betting) ----------

#[derive(Debug, Deserialize)]
pub struct WalletSetPkArgs {
    pub private_key: String,
    pub address: String,
    pub alias: Option<String>,
}

#[tauri::command]
pub async fn polyrocket_wallet_set_pk(
    state: State<'_, AppState>,
    args: WalletSetPkArgs,
) -> AppResult<()> {
    let pk = args.private_key.trim();
    if pk.is_empty() {
        return Err(crate::AppError::Invalid("private_key is empty".into()));
    }
    // light validation: 64 hex chars, optionally prefixed 0x
    let stripped = pk.strip_prefix("0x").unwrap_or(pk);
    if stripped.len() != 64 || !stripped.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(crate::AppError::Invalid(
            "private_key must be 64 hex chars (with or without 0x prefix)".into(),
        ));
    }
    if args.address.trim().is_empty() {
        return Err(crate::AppError::Invalid("address is empty".into()));
    }
    let alias = args.alias.unwrap_or_else(|| "primary".to_string());
    let keyring_alias = crate::platform::keyring::wallet_alias(&alias);
    crate::platform::keyring::set_key(&keyring_alias, pk)?;

    // Update wallets row (create if absent) — only non-secret fields
    let now = chrono::Utc::now().timestamp_millis();
    sqlx::query(
        "INSERT INTO wallets (id, address, label, chain, keyring_alias, created_at, updated_at)
         VALUES (?, ?, ?, 'polygon', ?, ?, ?)
         ON CONFLICT(address) DO UPDATE SET
           label=COALESCE(wallets.label, excluded.label),
           keyring_alias=excluded.keyring_alias,
           updated_at=?",
    )
    .bind(&args.address)
    .bind(&args.address)
    .bind(Some(&alias))
    .bind(&keyring_alias)
    .bind(now)
    .bind(now)
    .bind(now)
    .execute(&state.db)
    .await?;

    sqlx::query(
        "INSERT INTO audit_log (actor, action, target, payload, result) VALUES ('user', 'wallet.pk.set', ?, ?, 'ok')",
    )
    .bind(&args.address)
    .bind(serde_json::json!({"alias": alias, "keyring_alias": keyring_alias, "pk_len": stripped.len()}))
    .execute(&state.db)
    .await?;
    Ok(())
}

#[tauri::command]
pub async fn polyrocket_wallet_clear_pk(
    state: State<'_, AppState>,
    alias: Option<String>,
) -> AppResult<()> {
    let alias = alias.unwrap_or_else(|| "primary".to_string());
    let keyring_alias = crate::platform::keyring::wallet_alias(&alias);
    let _ = crate::platform::keyring::delete_key(&keyring_alias);
    sqlx::query(
        "UPDATE wallets SET keyring_alias = NULL, updated_at = unixepoch() * 1000 WHERE keyring_alias = ?",
    )
    .bind(&keyring_alias)
    .execute(&state.db)
    .await?;
    sqlx::query(
        "INSERT INTO audit_log (actor, action, target, result) VALUES ('user', 'wallet.pk.clear', ?, 'ok')",
    )
    .bind(&keyring_alias)
    .execute(&state.db)
    .await?;
    Ok(())
}

// ---------- Status (no secrets leaked) ----------

#[derive(Debug, Clone, Serialize)]
pub struct SecretStatus {
    pub kind: String,    // "llm_key" | "pm_api" | "pm_secret" | "pm_passphrase" | "wallet_pk"
    pub alias: String,   // keyring alias
    pub configured: bool,
    pub label: Option<String>, // human-friendly label for UI
}

#[derive(Debug, Clone, Serialize)]
pub struct SecretsStatus {
    pub llm_keys: Vec<SecretStatus>,
    pub polymarket: Vec<SecretStatus>,
    pub wallets: Vec<SecretStatus>,
}

/// Read-only status of which secrets are present in the OS keyring.
/// Never returns the secret itself — only a boolean `configured` flag.
#[tauri::command]
pub async fn secrets_status(state: State<'_, AppState>) -> AppResult<SecretsStatus> {
    // LLM keys — decode as Strings, compute `configured` from keyring.
    let llm_keys_db: Vec<(String, String, String, String)> = sqlx::query_as(
        "SELECT k.id, k.provider_id, k.alias, k.keyring_alias
         FROM llm_provider_keys k ORDER BY k.provider_id, k.priority",
    )
    .fetch_all(&state.db)
    .await?;

    let llm_keys = llm_keys_db
        .into_iter()
        .map(|(id, pid, alias, kra)| SecretStatus {
            kind: "llm_key".into(),
            alias: format!("{pid} / {alias}"),
            configured: crate::platform::keyring::has_key(&kra),
            label: Some(id),
        })
        .collect();

    // Polymarket
    let polymarket = vec![
        SecretStatus {
            kind: "pm_api".into(),
            alias: crate::platform::keyring::pm_api_alias().to_string(),
            configured: crate::platform::keyring::has_key(crate::platform::keyring::pm_api_alias()),
            label: Some("Polymarket API key".into()),
        },
        SecretStatus {
            kind: "pm_secret".into(),
            alias: crate::platform::keyring::pm_secret_alias().to_string(),
            configured: crate::platform::keyring::has_key(crate::platform::keyring::pm_secret_alias()),
            label: Some("Polymarket API secret".into()),
        },
        SecretStatus {
            kind: "pm_passphrase".into(),
            alias: crate::platform::keyring::pm_passphrase_alias().to_string(),
            configured: crate::platform::keyring::has_key(crate::platform::keyring::pm_passphrase_alias()),
            label: Some("Polymarket API passphrase".into()),
        },
    ];

    // Wallets
    let wallets_db: Vec<(String, String, Option<String>)> = sqlx::query_as(
        "SELECT address, COALESCE(label, address), keyring_alias FROM wallets ORDER BY created_at",
    )
    .fetch_all(&state.db)
    .await?;
    let wallets = wallets_db
        .into_iter()
        .map(|(address, label, kra)| {
            let configured = kra
                .as_deref()
                .map(crate::platform::keyring::has_key)
                .unwrap_or(false);
            SecretStatus {
                kind: "wallet_pk".into(),
                alias: address.clone(),
                configured,
                label: Some(label),
            }
        })
        .collect();

    Ok(SecretsStatus {
        llm_keys,
        polymarket,
        wallets,
    })
}
