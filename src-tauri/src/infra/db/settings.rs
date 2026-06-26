//! L4 —— 应用层级的非敏感键值设置。
//!
//! 为什么要用单独的表？Drizzle schema 由 webview 层持有,
//! 仅包含用户可见数据。Polymarket `host` URL、
//! 当前 `chain_id`、或 feature flags
//!（`POLYROCKET_TELEMETRY=1`）之类的设置由 Rust
//! 命令改写,但 webview 从不读取 —— 它们不属于 Drizzle。
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
//! 所有值以文本存储。使用强类型 getter
//!（[`get_u64`]、[`get_u32`]、[`get_i32`]）以默认值解析。

use sqlx::SqlitePool;

/// 创建 `_polyrocket_settings` 表。**幂等**（IF NOT EXISTS）。
///
/// **调用方**：`init_pool()` 在 startup 调一次。后续不再调。
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

/// Upsert 一个字符串值。**总是** touch `updated_at = unixepoch() * 1000`。
///
/// **业务流程**：L1 Settings 改任何 Rust 端可见的设置（如 `polymarket.host`、
/// `chain_id`）→ IPC 调到 `set`，写完立刻可读。
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

/// 读一个字符串值。**不存在**返回 `None`，**值存在但 parse 失败**也返回原始字符串。
pub async fn get(pool: &SqlitePool, k: &str) -> sqlx::Result<Option<String>> {
    let v: Option<String> = sqlx::query_scalar(
        "SELECT v FROM _polyrocket_settings WHERE k = ?",
    )
    .bind(k)
    .fetch_optional(pool)
    .await?;
    Ok(v)
}

/// 读字符串，缺失时返回 `default`（无字符串 clone 的 borrow 形式）。
pub async fn get_or(pool: &SqlitePool, k: &str, default: &str) -> sqlx::Result<String> {
    Ok(get(pool, k).await?.unwrap_or_else(|| default.to_string()))
}

/// 读取并按 u64 解析；缺失或无法解析时返回 `default`。
pub async fn get_u64(pool: &SqlitePool, k: &str, default: u64) -> sqlx::Result<u64> {
    Ok(get(pool, k)
        .await?
        .and_then(|v| v.parse().ok())
        .unwrap_or(default))
}

/// 读取并按 u32 解析；缺失或无法解析时返回 `default`。
pub async fn get_u32(pool: &SqlitePool, k: &str, default: u32) -> sqlx::Result<u32> {
    Ok(get(pool, k)
        .await?
        .and_then(|v| v.parse().ok())
        .unwrap_or(default))
}

/// 读取并按 i32 解析；缺失或无法解析时返回 `default`。
pub async fn get_i32(pool: &SqlitePool, k: &str, default: i32) -> sqlx::Result<i32> {
    Ok(get(pool, k)
        .await?
        .and_then(|v| v.parse().ok())
        .unwrap_or(default))
}

/// 删除一个 key。缺失时为 no-op。
pub async fn delete(pool: &SqlitePool, k: &str) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM _polyrocket_settings WHERE k = ?")
        .bind(k)
        .execute(pool)
        .await?;
    Ok(())
}

// =================================================================
// ============== v0.13c —— 审计保留用户策略 ==============
// =================================================================
//
// 默认 `RetentionPolicy`（90 天 / 5 万 / 1k）位于
// `domain::audit`。用户可从 Settings 覆盖。
// 覆盖项存于同一张 `_polyrocket_settings` 表下
// 三个独立 key,因此部分覆盖仍能与默认值合并。
// `delete_user_retention()` 清除覆盖并回退到默认值。

const K_RETAIN_MS: &str = "audit.retain_recent_ms";
const K_MAX_ROWS: &str = "audit.max_rows";
const K_MIN_KEEP: &str = "audit.min_keep_rows";

/// 读取用户覆盖的保留策略。缺失的 key 回退到
/// `RetentionPolicy::default()`。
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

/// 写入用户保留策略。仅持久化与默认值不同的字段
///（减少表 churn）。调用 `delete_audit_retention()`
/// 可清除所有覆盖并回退到默认值。
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

/// 删除所有保留覆盖。调度器会在下一个 tick 上
/// 回退到 `RetentionPolicy::default()`。
pub async fn delete_audit_retention(pool: &SqlitePool) -> sqlx::Result<()> {
    delete(pool, K_RETAIN_MS).await.ok();
    delete(pool, K_MAX_ROWS).await.ok();
    delete(pool, K_MIN_KEEP).await.ok();
    Ok(())
}

/// 读取并按 i64 解析；缺失或无法解析时返回 `default`。
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
        delete(&p, "k").await.unwrap();  // 缺失 → 不报错
        assert_eq!(get(&p, "k").await.unwrap(), None);
    }

    #[tokio::test]
    async fn get_or_returns_default_for_missing() {
        let p = fresh_pool().await;
        assert_eq!(get_or(&p, "absent", "fallback").await.unwrap(), "fallback");
    }

    // v0.13c —— 审计保留覆盖

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
        // 仅覆盖 retain_recent_ms
        let custom = crate::domain::audit::RetentionPolicy {
            retain_recent_ms: 7 * 86_400_000,
            max_rows: crate::domain::audit::RetentionPolicy::default().max_rows,
            min_keep_rows: crate::domain::audit::RetentionPolicy::default().min_keep_rows,
        };
        write_audit_retention(&p, &custom).await.unwrap();
        let got = read_audit_retention(&p).await.unwrap();
        assert_eq!(got.retain_recent_ms, 7 * 86_400_000);
        // 其它字段回退到默认值
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
        // 现在写入默认值 —— 应清除所有覆盖
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
