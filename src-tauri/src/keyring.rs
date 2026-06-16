//! macOS Keychain / Windows Credential Manager / Linux Secret Service
//!
//! polyrocket keeps ALL secrets out of SQLite and out of version control.
//! The .env file (if present) is dev-only and is consumed at startup to
//! seed the OS keyring; production / normal usage is "user pastes key in
//! Settings → LLM Management → Add key" which calls one of the IPC
//! commands in `commands/llm_mgmt` and `commands/secrets`.
//!
//! Alias conventions
//! -----------------
//! - LLM API key (per-provider, per-alias):  "llm/<provider_id>/<alias>"
//!   e.g.                                                  "llm/openai/prod-1"
//!                                                   or    "llm/anthropic/backup"
//! - Polymarket CLOB credential:               "polyrocket/pm/api"
//! - Polymarket CLOB secret:                   "polyrocket/pm/secret"
//! - Polymarket CLOB passphrase:                "polyrocket/pm/passphrase"
//! - Wallet private key (per alias):           "polyrocket/wallet/<alias>"
//!   e.g.                                       "polyrocket/wallet/primary"

use keyring::Entry;

const SERVICE: &str = "com.polyrocket.wallet";

// ---------- raw API ----------

pub fn set_key(alias: &str, secret: &str) -> crate::AppResult<()> {
    let entry = Entry::new(SERVICE, alias)?;
    entry.set_password(secret)?;
    Ok(())
}

pub fn get_key(alias: &str) -> crate::AppResult<String> {
    let entry = Entry::new(SERVICE, alias)?;
    Ok(entry.get_password()?)
}

pub fn delete_key(alias: &str) -> crate::AppResult<()> {
    let entry = Entry::new(SERVICE, alias)?;
    entry.delete_credential()?;
    Ok(())
}

pub fn has_key(alias: &str) -> bool {
    matches!(get_key(alias), Ok(s) if !s.is_empty())
}

// ---------- alias builders ----------

pub fn llm_alias(provider_id: &str, key_alias: &str) -> String {
    format!("llm/{provider_id}/{key_alias}")
}

pub fn pm_api_alias() -> &'static str {
    "polyrocket/pm/api"
}

pub fn pm_secret_alias() -> &'static str {
    "polyrocket/pm/secret"
}

pub fn pm_passphrase_alias() -> &'static str {
    "polyrocket/pm/passphrase"
}

pub fn wallet_alias(alias: &str) -> String {
    format!("polyrocket/wallet/{alias}")
}

// ---------- batched deletes (provider removal) ----------

/// Delete every LLM keychain entry matching `llm/<provider_id>/*`.
/// We can't enumerate the keyring, so the SQLite `llm_provider_keys`
/// table is the source of truth for "which aliases belong to provider X".
pub fn delete_provider_keys(provider_id: &str, keyring_aliases: &[String]) -> Vec<String> {
    let mut deleted = Vec::new();
    for alias in keyring_aliases {
        // Best-effort: only delete if the alias really is scoped to this provider.
        let expected_prefix = format!("llm/{provider_id}/");
        if alias.starts_with(&expected_prefix) {
            if delete_key(alias).is_ok() {
                deleted.push(alias.clone());
            }
        }
    }
    deleted
}

// ---------- .env parser (no extra crate dep) ----------
//
// Used at startup ONLY when `POLYROCKET_ENV=dev` and
// `POLYROCKET_KEYRING_ONLY=0`. Each recognised key is mirrored to the OS
// keyring if a corresponding entry is not already present. The function
// never panics, never logs the secret, and is a no-op in any other env.

pub fn parse_env_file(path: &std::path::Path) -> Vec<(String, String)> {
    let Ok(content) = std::fs::read_to_string(path) else {
        return vec![];
    };
    let mut out = Vec::new();
    for (lineno, raw) in content.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        // strip optional leading "export "
        let line = line.strip_prefix("export ").unwrap_or(line).trim();
        let Some((k, v)) = line.split_once('=') else {
            tracing::warn!(".env line {}: missing '=' (skipped)", lineno + 1);
            continue;
        };
        let key = k.trim().to_string();
        let val = v.trim();
        // strip surrounding quotes
        let val = if (val.starts_with('"') && val.ends_with('"') && val.len() >= 2)
            || (val.starts_with('\'') && val.ends_with('\'') && val.len() >= 2)
        {
            &val[1..val.len() - 1]
        } else {
            val
        };
        if val.is_empty() {
            continue;
        }
        out.push((key, val.to_string()));
    }
    out
}
