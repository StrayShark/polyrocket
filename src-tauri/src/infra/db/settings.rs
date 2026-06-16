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
}
