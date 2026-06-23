//! L5 — Cross-platform OS keyring adapter.
//!
//! The `keyring` crate (3.x) handles the per-OS differences internally
//! (apple-native, windows-native, sync-secret-service on Linux via
//! dbus). This module is a thin wrapper that:
//!   1. Pins the service name (`com.polyrocket.wallet`) for the whole app.
//!   2. Adds a few helpers (`has_key`, batched delete) that aren't
//!      part of the upstream crate.
//!   3. Re-exports alias builders from `aliases.rs`.
//!
//! **Security model**: the secret string is `String` here (not `&str`)
//! because we accept ownership from the caller. We never log the
//! secret. We never write the secret to SQLite, JSON, or any file.
//!
//! **v0.119 — env-first, no UI prompts**:
//!
//! The OS keyring (macOS Keychain / Windows Credential Manager /
//! Linux Secret Service) is **disabled by default**. All secrets come
//! from `.env` (via process env vars). Rationale:
//!   - `.env` is the single source of truth (already symlinked to
//!     `~/global_env/.env` for cross-project credentials).
//!   - The macOS Keychain ACL popup blocks headless tests forever on
//!     adhoc-signed binaries (and prompts the user mid-dev otherwise).
//!   - The UI never asks the user to type passwords / API keys —
//!     they live in `.env`, period.
//!
//! To opt BACK IN to OS keyring storage (production-hardened
//! deployments that want secrets off the filesystem), set:
//!   `POLYROCKET_USE_KEYRING=1`
//!
//! With that flag set, `set_key` writes to OS keyring, `get_key` reads
//! from OS keyring, and `.env` is no longer consulted. The UI
//! password fields become functional again.

use keyring::Entry;

/// Service name registered with the OS credential store. This is the
/// "namespace" inside which every polyrocket alias lives.
const SERVICE: &str = "com.polyrocket.wallet";

/// True when the OS keyring should be SKIPPED in favor of process
/// env vars. Default = TRUE (env-only). Set `POLYROCKET_USE_KEYRING=1`
/// to re-enable keyring storage (production-hardened mode).
pub fn is_disabled() -> bool {
    // Inverted logic: keyring is OFF by default.
    let enabled = std::env::var("POLYROCKET_USE_KEYRING")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    !enabled
}

/// Map a keyring alias → process env var name. Used only when
/// `is_disabled()` is true so that the dev-mode bypass can find
/// the secret in `std::env::var` instead of the OS keyring.
fn alias_to_env_var(alias: &str) -> Option<String> {
    match alias {
        // Polymarket CLOB (matches `aliases.rs` pm_*_alias())
        "polyrocket/pm/api"        => Some("POLYMARKET_API_KEY".to_string()),
        "polyrocket/pm/secret"     => Some("POLYMARKET_API_SECRET".to_string()),
        "polyrocket/pm/passphrase" => Some("POLYMARKET_API_PASSPHRASE".to_string()),
        // Wallet — `polyrocket/wallet/{name}` → POLYROCKET_WALLET_PRIVATE_KEY
        a if a.starts_with("polyrocket/wallet/") => {
            Some("POLYROCKET_WALLET_PRIVATE_KEY".to_string())
        }
        // LLM — `llm/{provider_id}/{alias}` → {PROVIDER}_API_KEY
        // Examples:
        //   llm/openai/openai-prod-1   → OPENAI_API_KEY
        //   llm/openai/openai-backup   → OPENAI_BACKUP_KEY
        //   llm/anthropic/anthropic-prod-1 → ANTHROPIC_API_KEY
        a if a.starts_with("llm/") => {
            // strip "llm/" prefix → "{provider_id}/{alias}"
            let rest = &a[4..];
            let parts: Vec<&str> = rest.splitn(2, '/').collect();
            if parts.len() != 2 { return None; }
            let provider_id = parts[0];
            let key_alias = parts[1];
            // Map (provider_id, key_alias) → env var name. Mirrors
            // the table in env.rs::sync_env_to_keyring so the dev
            // bypass stays in sync with what the sync layer writes.
            let env_var = match (provider_id, key_alias) {
                ("openai", "openai-prod-1")     => "OPENAI_API_KEY",
                ("openai", "openai-backup")     => "OPENAI_BACKUP_KEY",
                ("anthropic", "anthropic-prod-1") => "ANTHROPIC_API_KEY",
                ("anthropic", "anthropic-backup") => "ANTHROPIC_BACKUP_KEY",
                ("google", "google-prod-1")     => "GOOGLE_API_KEY",
                ("deepseek", "deepseek-prod-1") => "DEEPSEEK_API_KEY",
                ("custom", "custom-prod-1")     => "CUSTOM_LLM_API_KEY",
                // Chinese LLM providers — used by the e2e_football bootstrap
                // and the WelcomeStep flow. Alias is the short "prod-1" form.
                ("MiniMax", "prod-1") => "MINIMAX_API_KEY",
                ("doubao",  "prod-1") => "DOUBAO_API_KEY",
                ("qwen",     "prod-1") => "QWEN_API_KEY",
                ("moonshot", "prod-1") => "MOONSHOT_API_KEY",
                ("zhipu",    "prod-1") => "ZHIPU_API_KEY",
                ("hunyuan",  "prod-1") => "HUNYUAN_API_KEY",
                // Generic fallback: "{PROVIDER}_API_KEY" for any alias.
                // This catches every short alias ("prod-1", "prod-2", "main", …).
                _ => {
                    return Some(format!("{}_API_KEY", provider_id.to_uppercase()));
                }
            };
            Some(env_var.to_string())
        }
        _ => None,
    }
}

/// Build an `Entry` (lazy — no IO). All errors are `keyring::Error`
/// which we convert at the call site.
fn entry(alias: &str) -> crate::AppResult<Entry> {
    Entry::new(SERVICE, alias).map_err(|e| {
        crate::AppError::Internal(format!("keyring entry({alias}): {e}"))
    })
}

// ---------- public raw API ----------

/// Store a secret under `alias`. Overwrites any prior value at that alias.
pub fn set_key(alias: &str, secret: &str) -> crate::AppResult<()> {
    if is_disabled() {
        // No-op in dev mode. The secret lives in process env / .env only.
        // Returning Ok here would mask the absence of a real write; instead
        // emit a tracing warning so dev knows the call was a no-op.
        tracing::warn!(
            "keyring.set_key({alias}): bypass (POLYROCKET_DEV_NO_KEYRING=1); secret stays in env"
        );
        return Ok(());
    }
    let e = entry(alias)?;
    e.set_password(secret).map_err(|e| {
        crate::AppError::Internal(format!("keyring set_password({alias}): {e}"))
    })
}

/// Read the secret at `alias`. Returns `AppError::Internal` if the entry
/// does not exist OR the OS denies access (we deliberately don't
/// distinguish the two cases — the audit log doesn't need to leak
/// "this alias exists but is locked" vs "doesn't exist").
///
/// v0.119 — dev-mode bypass: when `POLYROCKET_DEV_NO_KEYRING=1`, falls
/// through to process env vars (mapped via `alias_to_env_var`).
pub fn get_key(alias: &str) -> crate::AppResult<String> {
    if is_disabled() {
        let env_var = alias_to_env_var(alias).ok_or_else(|| {
            crate::AppError::Internal(format!(
                "keyring.get_key({alias}): dev bypass enabled but no env-var mapping known"
            ))
        })?;
        return match std::env::var(&env_var) {
            Ok(v) if !v.is_empty() => Ok(v),
            Ok(_) => Err(crate::AppError::Internal(format!(
                "keyring.get_key({alias}): dev bypass — env {env_var} is empty"
            ))),
            Err(_) => Err(crate::AppError::Internal(format!(
                "keyring.get_key({alias}): dev bypass — env {env_var} unset"
            ))),
        };
    }
    let e = entry(alias)?;
    e.get_password().map_err(|err| {
        crate::AppError::Internal(format!("keyring get_password({alias}): {err}"))
    })
}

/// Delete the secret at `alias`. Idempotent — a missing entry is
/// silently treated as success.
pub fn delete_key(alias: &str) -> crate::AppResult<()> {
    if is_disabled() {
        // Nothing to delete — the secret lives in process env.
        return Ok(());
    }
    let e = entry(alias)?;
    match e.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(err) => Err(crate::AppError::Internal(format!(
            "keyring delete_credential({alias}): {err}"
        ))),
    }
}

/// Cheap "is the secret present and non-empty?" check.
///
/// v0.119 — dev-mode bypass: checks process env instead of OS keyring.
pub fn has_key(alias: &str) -> bool {
    if is_disabled() {
        return alias_to_env_var(alias)
            .and_then(|env_var| std::env::var(&env_var).ok())
            .map(|v| !v.is_empty())
            .unwrap_or(false);
    }
    matches!(get_key(alias), Ok(s) if !s.is_empty())
}

pub mod aliases;
pub use aliases::*;
