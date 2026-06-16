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

use keyring::Entry;

/// Service name registered with the OS credential store. This is the
/// "namespace" inside which every polyrocket alias lives.
const SERVICE: &str = "com.polyrocket.wallet";

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
    let e = entry(alias)?;
    e.set_password(secret).map_err(|e| {
        crate::AppError::Internal(format!("keyring set_password({alias}): {e}"))
    })
}

/// Read the secret at `alias`. Returns `AppError::Internal` if the entry
/// does not exist OR the OS denies access (we deliberately don't
/// distinguish the two cases — the audit log doesn't need to leak
/// "this alias exists but is locked" vs "doesn't exist").
pub fn get_key(alias: &str) -> crate::AppResult<String> {
    let e = entry(alias)?;
    e.get_password().map_err(|err| {
        crate::AppError::Internal(format!("keyring get_password({alias}): {err}"))
    })
}

/// Delete the secret at `alias`. Idempotent — a missing entry is
/// silently treated as success.
pub fn delete_key(alias: &str) -> crate::AppResult<()> {
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
pub fn has_key(alias: &str) -> bool {
    matches!(get_key(alias), Ok(s) if !s.is_empty())
}

pub mod aliases;
pub use aliases::*;
