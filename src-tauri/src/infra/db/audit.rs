//! L4 — Audit log retention applier (DB-bound).
//!
//! Wraps `domain::audit::plan_purge` with the SQL needed to actually
//! delete rows. Designed to be cheap enough to run on a daily
//! scheduler tick.

use crate::domain::audit::{plan_purge, AuditRow, RetentionPolicy};
use crate::infra::error::AppResult;
use sqlx::SqlitePool;

/// Apply the retention policy: delete rows that are both older than
/// the cutoff AND past the safety floor. Returns the number of rows
/// deleted (0 if the DB is already within policy).
///
/// The `now_ms` parameter is exposed for tests; in production pass
/// `chrono::Utc::now().timestamp_millis()`.
pub async fn purge_old(
    pool: &SqlitePool,
    policy: &RetentionPolicy,
    now_ms: i64,
) -> AppResult<usize> {
    // SELECT only the (id, at) we need; never SELECT * here.
    let rows: Vec<AuditRow> = sqlx::query_as(
        "SELECT id, at FROM audit_log ORDER BY at DESC",
    )
    .fetch_all(pool)
    .await?;

    let to_delete = plan_purge(&rows, policy, now_ms);
    if to_delete.is_empty() {
        return Ok(0);
    }
    let n = to_delete.len();

    // Build a parameterised DELETE: "DELETE FROM audit_log WHERE id IN (?, ?, ?)"
    // SQLite has a default SQLITE_MAX_VARIABLE_NUMBER of 999; chunk to be safe.
    for chunk in to_delete.chunks(500) {
        let placeholders = std::iter::repeat("?")
            .take(chunk.len())
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!("DELETE FROM audit_log WHERE id IN ({})", placeholders);
        let mut q = sqlx::query(&sql);
        for id in chunk {
            q = q.bind(*id);
        }
        q.execute(pool).await?;
    }
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
        sqlx::query(
            "CREATE TABLE audit_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                at INTEGER NOT NULL,
                actor TEXT NOT NULL,
                action TEXT NOT NULL,
                target TEXT,
                payload TEXT,
                result TEXT
            )",
        )
        .execute(&pool)
        .await
        .unwrap();
        pool
    }

    async fn insert(pool: &SqlitePool, at: i64, action: &str) {
        sqlx::query("INSERT INTO audit_log (at, actor, action, result) VALUES (?, 'test', ?, 'ok')")
            .bind(at)
            .bind(action)
            .execute(pool)
            .await
            .unwrap();
    }

    async fn count(pool: &SqlitePool) -> i64 {
        sqlx::query_scalar("SELECT COUNT(*) FROM audit_log")
            .fetch_one(pool)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn purge_old_empty_db_returns_zero() {
        let pool = empty_pool().await;
        let n = purge_old(&pool, &RetentionPolicy::default(), 1_000_000).await.unwrap();
        assert_eq!(n, 0);
    }

    #[tokio::test]
    async fn purge_old_recent_rows_kept() {
        let pool = empty_pool().await;
        let now = 1_000_000_000_000;
        for i in 0..10 {
            insert(&pool, now - i * 3_600_000, &format!("a{i}")).await;
        }
        let n = purge_old(&pool, &RetentionPolicy::default(), now).await.unwrap();
        assert_eq!(n, 0);
        assert_eq!(count(&pool).await, 10);
    }

    #[tokio::test]
    async fn purge_old_removes_only_old_rows() {
        let pool = empty_pool().await;
        let now = 1_000_000_000_000;
        let old = now - 365 * 86_400_000;
        // 5 recent + 5 old
        for i in 0..5 {
            insert(&pool, now - i * 3_600_000, "recent").await;
        }
        for i in 0..5 {
            insert(&pool, old + i, "old").await;
        }
        // Floor (1000) is way above 10, so the 5 recent stay and the
        // 5 old also stay (within floor).
        let n = purge_old(&pool, &RetentionPolicy::default(), now).await.unwrap();
        assert_eq!(n, 0);
        assert_eq!(count(&pool).await, 10);
    }

    #[tokio::test]
    async fn purge_old_purges_above_floor() {
        let pool = empty_pool().await;
        let now = 1_000_000_000_000;
        let old = now - 365 * 86_400_000;
        // 1500 total: 1000 recent + 500 old
        for i in 0..1000 {
            insert(&pool, now - i * 3_600_000, "recent").await;
        }
        for i in 0..500 {
            insert(&pool, old + i, "old").await;
        }
        assert_eq!(count(&pool).await, 1500);
        let n = purge_old(&pool, &RetentionPolicy::default(), now).await.unwrap();
        assert_eq!(n, 500);
        assert_eq!(count(&pool).await, 1000);
    }

    #[tokio::test]
    async fn purge_old_handles_chunks() {
        // > 500 row purge exercises the chunking path
        let pool = empty_pool().await;
        let now = 1_000_000_000_000;
        let old = now - 365 * 86_400_000;
        for i in 0..2000 {
            insert(&pool, old + i, "old").await;
        }
        let policy = RetentionPolicy { min_keep_rows: 100, ..Default::default() };
        let n = purge_old(&pool, &policy, now).await.unwrap();
        assert_eq!(n, 1900);
        assert_eq!(count(&pool).await, 100);
    }

    #[tokio::test]
    async fn purge_old_is_idempotent() {
        let pool = empty_pool().await;
        let now = 1_000_000_000_000;
        let old = now - 365 * 86_400_000;
        for i in 0..2000 {
            insert(&pool, old + i, "old").await;
        }
        let policy = RetentionPolicy { min_keep_rows: 100, ..Default::default() };
        let first = purge_old(&pool, &policy, now).await.unwrap();
        let second = purge_old(&pool, &policy, now).await.unwrap();
        assert_eq!(first, 1900);
        assert_eq!(second, 0, "second call should be a no-op");
    }
}
