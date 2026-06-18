//! L4 — Sidecar health probe DB (v0.10d).
//!
//! Stores each probe result in a small ring-buffer table. The
//! scheduler pings every 30s, writes one row, and a periodic
//! GC keeps only the last 24h of rows.
//!
//! Pure SQL helpers + the `record_probe` + `recent` + `purge_old`
//! wrapper functions. L3 pure snapshot lives in
//! `domain::sidecar_health`.

use crate::domain::sidecar_health::{SidecarHealthKind, SidecarHealthRow, SidecarHealthSnapshot};
use crate::infra::error::AppResult;
use sqlx::SqlitePool;

/// 24h in ms — how long we keep probe rows.
const RETAIN_MS: i64 = 24 * 3_600_000;

/// v0.10d — 创建 `sidecar_health` 表 + 索引。**幂等**（IF NOT EXISTS）。
///
/// **调用方**：`init_pool()` 在 startup 调一次。后续不再调。
pub async fn ensure_table(pool: &SqlitePool) -> AppResult<()> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS sidecar_health (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            at_ms INTEGER NOT NULL,
            kind TEXT NOT NULL,
            latency_ms INTEGER,
            error TEXT
        )",
    )
    .execute(pool)
    .await?;
    sqlx::query(
        "CREATE INDEX IF NOT EXISTS sidecar_health_at_idx
         ON sidecar_health(at_ms DESC)",
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// 写一行 sidecar 探测结果。**每次 sidecar 启动/ping/崩溃 都会调一次**。
///
/// **业务流程**：`run_sidecar_health_loop` 每 30s 调一次，按结果填 `kind`/`latency_ms`/`error`。
/// 配合 `purge_old` 形成 24h 滚动 ring buffer。
pub async fn record_probe(
    pool: &SqlitePool,
    at_ms: i64,
    kind: SidecarHealthKind,
    latency_ms: Option<i64>,
    error: Option<&str>,
) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO sidecar_health (at_ms, kind, latency_ms, error)
         VALUES (?, ?, ?, ?)",
    )
    .bind(at_ms)
    .bind(kind.as_str())
    .bind(latency_ms)
    .bind(error)
    .execute(pool)
    .await?;
    Ok(())
}

/// 拉最近 200 条探测记录 → 转成 `SidecarHealthSnapshot` 供 L1 Settings 渲染。
///
/// **为什么 LIMIT 200**：L1 只展示「最近几次 + 趋势」；200 条 ≈ 100 分钟数据，
/// 够画 1-2 张图。
pub async fn recent(pool: &SqlitePool) -> AppResult<SidecarHealthSnapshot> {
    let rows: Vec<(i64, String, Option<i64>, Option<String>)> = sqlx::query_as(
        "SELECT at_ms, kind, latency_ms, error
         FROM sidecar_health
         ORDER BY at_ms DESC
         LIMIT 200",
    )
    .fetch_all(pool)
    .await?;
    let mapped: Vec<SidecarHealthRow> = rows
        .into_iter()
        .map(|(at_ms, kind, latency_ms, error)| SidecarHealthRow {
            at_ms,
            kind: SidecarHealthKind::parse(&kind),
            latency_ms,
            error,
        })
        .collect();
    Ok(SidecarHealthSnapshot::from_rows(&mapped))
}

/// 删除 24h 之前的探测记录。返回删除行数。
///
/// **调用方**：`run_sidecar_health_loop` 每小时调一次（不调每个 tick，避免重复写）。
pub async fn purge_old(pool: &SqlitePool, now_ms: i64) -> AppResult<usize> {
    let n: usize = sqlx::query("DELETE FROM sidecar_health WHERE at_ms < ?")
        .bind(now_ms - RETAIN_MS)
        .execute(pool)
        .await?
        .rows_affected() as usize;
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn empty_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite");
        ensure_table(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn ensure_table_idempotent() {
        let pool = empty_pool().await;
        // Run twice — should not error
        ensure_table(&pool).await.unwrap();
    }

    #[tokio::test]
    async fn record_and_recent() {
        let pool = empty_pool().await;
        record_probe(&pool, 100, SidecarHealthKind::Ok, Some(5), None).await.unwrap();
        record_probe(&pool, 200, SidecarHealthKind::Failed, None, Some("timeout")).await.unwrap();
        record_probe(&pool, 300, SidecarHealthKind::Ok, Some(3), None).await.unwrap();
        let snap = recent(&pool).await.unwrap();
        assert_eq!(snap.success_count, 2);
        assert_eq!(snap.failure_count, 1);
        assert_eq!(snap.last_success_at_ms, Some(300));
        assert_eq!(snap.last_failure_at_ms, Some(200));
    }

    #[tokio::test]
    async fn purge_old_removes_only_old_rows() {
        let pool = empty_pool().await;
        let now = 1_000_000_000_000i64;
        // 1 fresh + 2 old (older than 24h)
        record_probe(&pool, now, SidecarHealthKind::Ok, Some(5), None).await.unwrap();
        record_probe(&pool, now - 25 * 3_600_000, SidecarHealthKind::Ok, Some(5), None).await.unwrap();
        record_probe(&pool, now - 30 * 3_600_000, SidecarHealthKind::Failed, None, Some("oops")).await.unwrap();
        let n = purge_old(&pool, now).await.unwrap();
        assert_eq!(n, 2);
        // The fresh one survives
        let snap = recent(&pool).await.unwrap();
        assert_eq!(snap.success_count, 1);
        assert_eq!(snap.failure_count, 0);
    }
}
