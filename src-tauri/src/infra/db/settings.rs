//! L4 — App-level non-secret key/value settings.
//!
//! Why a side-table? The Drizzle schema is owned by the webview layer
//! and only contains user-facing data. Things like the Polymarket
//! `host` URL, the active `chain_id`, or feature flags
//! (`POLYROCKET_TELEMETRY=1`) are mutated from Rust commands but never
//! read by the webview — they don't belong in Drizzle.
//!
//! Schema:
//! ```sql
//! CREATE TABLE _polyrocket_settings (
//!     k TEXT PRIMARY KEY,
//!     v TEXT NOT NULL,
//!     updated_at INTEGER NOT NULL DEFAULT (unixepoch() * 1000)
//! )
//! ```
//!
//! All values are stored as text. Use the typed getters
//! ([`get_u64`], [`get_u32`], [`get_i32`]) to parse with a default.

use sqlx::SqlitePool;

/// Create the side-table if it doesn't exist. Idempotent.
pub async fn ensure_table(pool: &SqlitePool) -> sqlx::Result<()> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS _polyrocket_settings (
            k TEXT PRIMARY KEY,
            v TEXT NOT NULL,
            updated_at INTEGER NOT NULL DEFAULT (unixepoch() * 1000)
        )",
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// Upsert a string value.
pub async fn set(pool: &SqlitePool, k: &str, v: &str) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO _polyrocket_settings (k, v) VALUES (?, ?)
         ON CONFLICT(k) DO UPDATE SET v = excluded.v, updated_at = unixepoch() * 1000",
    )
    .bind(k)
    .bind(v)
    .execute(pool)
    .await?;
    Ok(())
}

/// Read a string value (or None if key is missing).
pub async fn get(pool: &SqlitePool, k: &str) -> sqlx::Result<Option<String>> {
    let v: Option<String> = sqlx::query_scalar(
        "SELECT v FROM _polyrocket_settings WHERE k = ?",
    )
    .bind(k)
    .fetch_optional(pool)
    .await?;
    Ok(v)
}

/// Read a string value, returning `default` if missing.
pub async fn get_or(pool: &SqlitePool, k: &str, default: &str) -> sqlx::Result<String> {
    Ok(get(pool, k).await?.unwrap_or_else(|| default.to_string()))
}

/// Read and parse as u64; return `default` on missing or unparseable.
pub async fn get_u64(pool: &SqlitePool, k: &str, default: u64) -> sqlx::Result<u64> {
    Ok(get(pool, k)
        .await?
        .and_then(|v| v.parse().ok())
        .unwrap_or(default))
}

/// Read and parse as u32; return `default` on missing or unparseable.
pub async fn get_u32(pool: &SqlitePool, k: &str, default: u32) -> sqlx::Result<u32> {
    Ok(get(pool, k)
        .await?
        .and_then(|v| v.parse().ok())
        .unwrap_or(default))
}

/// Read and parse as i32; return `default` on missing or unparseable.
pub async fn get_i32(pool: &SqlitePool, k: &str, default: i32) -> sqlx::Result<i32> {
    Ok(get(pool, k)
        .await?
        .and_then(|v| v.parse().ok())
        .unwrap_or(default))
}

/// Delete a key. No-op if missing.
pub async fn delete(pool: &SqlitePool, k: &str) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM _polyrocket_settings WHERE k = ?")
        .bind(k)
        .execute(pool)
        .await?;
    Ok(())
}

// =================================================================
// ============== v0.13c — Audit retention user policy ==============
// =================================================================
//
// The default `RetentionPolicy` (90d / 50k / 1k) lives in
// `domain::audit`. Users can override it from Settings. The
// overrides are stored in the same `_polyrocket_settings` table
// under three separate keys, so a partial override still merges
// with the defaults. `delete_user_retention()` clears overrides
// and falls back to defaults.

const K_RETAIN_MS: &str = "audit.retain_recent_ms";
const K_MAX_ROWS: &str = "audit.max_rows";
const K_MIN_KEEP: &str = "audit.min_keep_rows";

/// Read the user-overridden retention policy. Missing keys fall
/// back to `RetentionPolicy::default()`.
pub async fn read_audit_retention(
    pool: &SqlitePool,
) -> sqlx::Result<crate::domain::audit::RetentionPolicy> {
    use crate::domain::audit::RetentionPolicy;
    let def = RetentionPolicy::default();
    let retain = get_i64_or(pool, K_RETAIN_MS, def.retain_recent_ms).await?;
    let max = get_i64_or(pool, K_MAX_ROWS, def.max_rows).await?;
    let min = get_i64_or(pool, K_MIN_KEEP, def.min_keep_rows).await?;
    Ok(RetentionPolicy {
        retain_recent_ms: retain,
        max_rows: max,
        min_keep_rows: min,
    })
}

/// Write a user retention policy. Only non-default values are
/// persisted (saves table churn). Use `delete_audit_retention()`
/// to clear all overrides and revert to defaults.
pub async fn write_audit_retention(
    pool: &SqlitePool,
    policy: &crate::domain::audit::RetentionPolicy,
) -> sqlx::Result<()> {
    let def = crate::domain::audit::RetentionPolicy::default();
    if policy.retain_recent_ms != def.retain_recent_ms {
        set(pool, K_RETAIN_MS, &policy.retain_recent_ms.to_string()).await?;
    } else {
        delete(pool, K_RETAIN_MS).await.ok();
    }
    if policy.max_rows != def.max_rows {
        set(pool, K_MAX_ROWS, &policy.max_rows.to_string()).await?;
    } else {
        delete(pool, K_MAX_ROWS).await.ok();
    }
    if policy.min_keep_rows != def.min_keep_rows {
        set(pool, K_MIN_KEEP, &policy.min_keep_rows.to_string()).await?;
    } else {
        delete(pool, K_MIN_KEEP).await.ok();
    }
    Ok(())
}

/// Delete all retention overrides. The scheduler will then fall
/// back to `RetentionPolicy::default()` on the next tick.
pub async fn delete_audit_retention(pool: &SqlitePool) -> sqlx::Result<()> {
    delete(pool, K_RETAIN_MS).await.ok();
    delete(pool, K_MAX_ROWS).await.ok();
    delete(pool, K_MIN_KEEP).await.ok();
    Ok(())
}

/// Read and parse as i64; return `default` on missing or unparseable.
async fn get_i64_or(pool: &SqlitePool, k: &str, default: i64) -> sqlx::Result<i64> {
    Ok(get(pool, k)
        .await?
        .and_then(|v| v.parse().ok())
        .unwrap_or(default))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use std::str::FromStr;

    async fn fresh_pool() -> SqlitePool {
        let opts = SqliteConnectOptions::from_str("sqlite::memory:")
            .unwrap()
            .create_if_missing(true);
        let pool = SqlitePoolOptions::new().max_connections(1).connect_with(opts).await.unwrap();
        ensure_table(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn round_trip_string() {
        let p = fresh_pool().await;
        assert_eq!(get(&p, "missing").await.unwrap(), None);
        set(&p, "host", "https://clob.polymarket.com").await.unwrap();
        assert_eq!(
            get(&p, "host").await.unwrap().as_deref(),
            Some("https://clob.polymarket.com")
        );
    }

    #[tokio::test]
    async fn upsert_overwrites() {
        let p = fresh_pool().await;
        set(&p, "k", "v1").await.unwrap();
        set(&p, "k", "v2").await.unwrap();
        assert_eq!(get(&p, "k").await.unwrap().as_deref(), Some("v2"));
    }

    #[tokio::test]
    async fn typed_getters_apply_default() {
        let p = fresh_pool().await;
        assert_eq!(get_u64(&p, "n", 42).await.unwrap(), 42);
        assert_eq!(get_u32(&p, "n", 7).await.unwrap(), 7);
        assert_eq!(get_i32(&p, "n", -1).await.unwrap(), -1);
        set(&p, "n", "100").await.unwrap();
        assert_eq!(get_u64(&p, "n", 42).await.unwrap(), 100);
    }

    #[tokio::test]
    async fn typed_getters_fall_back_on_unparseable() {
        let p = fresh_pool().await;
        set(&p, "junk", "not-a-number").await.unwrap();
        assert_eq!(get_u64(&p, "junk", 9).await.unwrap(), 9);
    }

    #[tokio::test]
    async fn delete_is_idempotent() {
        let p = fresh_pool().await;
        set(&p, "k", "v").await.unwrap();
        delete(&p, "k").await.unwrap();
        delete(&p, "k").await.unwrap();  // missing → no error
        assert_eq!(get(&p, "k").await.unwrap(), None);
    }

    #[tokio::test]
    async fn get_or_returns_default_for_missing() {
        let p = fresh_pool().await;
        assert_eq!(get_or(&p, "absent", "fallback").await.unwrap(), "fallback");
    }

    // v0.13c — audit retention overrides

    #[tokio::test]
    async fn audit_retention_defaults_when_no_overrides() {
        let p = fresh_pool().await;
        let policy = read_audit_retention(&p).await.unwrap();
        let def = crate::domain::audit::RetentionPolicy::default();
        assert_eq!(policy.retain_recent_ms, def.retain_recent_ms);
        assert_eq!(policy.max_rows, def.max_rows);
        assert_eq!(policy.min_keep_rows, def.min_keep_rows);
    }

    #[tokio::test]
    async fn audit_retention_round_trip() {
        let p = fresh_pool().await;
        let custom = crate::domain::audit::RetentionPolicy {
            retain_recent_ms: 30 * 86_400_000,
            max_rows: 10_000,
            min_keep_rows: 500,
        };
        write_audit_retention(&p, &custom).await.unwrap();
        let got = read_audit_retention(&p).await.unwrap();
        assert_eq!(got.retain_recent_ms, 30 * 86_400_000);
        assert_eq!(got.max_rows, 10_000);
        assert_eq!(got.min_keep_rows, 500);
    }

    #[tokio::test]
    async fn audit_retention_partial_override() {
        let p = fresh_pool().await;
        // Override only retain_recent_ms
        let custom = crate::domain::audit::RetentionPolicy {
            retain_recent_ms: 7 * 86_400_000,
            max_rows: crate::domain::audit::RetentionPolicy::default().max_rows,
            min_keep_rows: crate::domain::audit::RetentionPolicy::default().min_keep_rows,
        };
        write_audit_retention(&p, &custom).await.unwrap();
        let got = read_audit_retention(&p).await.unwrap();
        assert_eq!(got.retain_recent_ms, 7 * 86_400_000);
        // other fields fall back to defaults
        let def = crate::domain::audit::RetentionPolicy::default();
        assert_eq!(got.max_rows, def.max_rows);
        assert_eq!(got.min_keep_rows, def.min_keep_rows);
    }

    #[tokio::test]
    async fn audit_retention_writing_defaults_clears_overrides() {
        let p = fresh_pool().await;
        let custom = crate::domain::audit::RetentionPolicy {
            retain_recent_ms: 1 * 86_400_000,
            max_rows: 100,
            min_keep_rows: 10,
        };
        write_audit_retention(&p, &custom).await.unwrap();
        // Now write the default — should clear all overrides
        write_audit_retention(&p, &crate::domain::audit::RetentionPolicy::default()).await.unwrap();
        let got = read_audit_retention(&p).await.unwrap();
        let def = crate::domain::audit::RetentionPolicy::default();
        assert_eq!(got.retain_recent_ms, def.retain_recent_ms);
        assert_eq!(got.max_rows, def.max_rows);
        assert_eq!(got.min_keep_rows, def.min_keep_rows);
    }

    #[tokio::test]
    async fn audit_retention_delete_clears_overrides() {
        let p = fresh_pool().await;
        let custom = crate::domain::audit::RetentionPolicy {
            retain_recent_ms: 7 * 86_400_000,
            max_rows: 1_000,
            min_keep_rows: 50,
        };
        write_audit_retention(&p, &custom).await.unwrap();
        delete_audit_retention(&p).await.unwrap();
        let got = read_audit_retention(&p).await.unwrap();
        let def = crate::domain::audit::RetentionPolicy::default();
        assert_eq!(got.retain_recent_ms, def.retain_recent_ms);
        assert_eq!(got.max_rows, def.max_rows);
        assert_eq!(got.min_keep_rows, def.min_keep_rows);
    }

    #[tokio::test]
    async fn audit_retention_falls_back_on_unparseable() {
        let p = fresh_pool().await;
        set(&p, "audit.retain_recent_ms", "not-a-number").await.unwrap();
        let def = crate::domain::audit::RetentionPolicy::default();
        let got = read_audit_retention(&p).await.unwrap();
        assert_eq!(got.retain_recent_ms, def.retain_recent_ms);
    }
}
