//! L4 —— 审计日志保留策略执行器（绑定数据库）。
//!
//! 将 `domain::audit::plan_purge` 与实际删除行所需的 SQL 包装在一起。
//! 设计得足够轻量,可在每日调度器节拍上运行。

use crate::domain::audit::{plan_purge, AuditRow, RetentionPolicy};
use crate::infra::error::AppResult;
use sqlx::SqlitePool;

/// 应用保留策略:删除同时满足“早于截止时间”且
/// “超出安全下限”的行。返回被删除的行数（若 DB
/// 已满足策略则返回 0）。
///
/// `now_ms` 参数暴露给测试使用;在生产环境中传入
/// `chrono::Utc::now().timestamp_millis()`。
pub async fn purge_old(
    pool: &SqlitePool,
    policy: &RetentionPolicy,
    now_ms: i64,
) -> AppResult<usize> {
    // 仅 SELECT 我们需要的 (id, at);这里绝不使用 SELECT *。
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

    // 构造参数化 DELETE："DELETE FROM audit_log WHERE id IN (?, ?, ?)"
    // SQLite 默认 SQLITE_MAX_VARIABLE_NUMBER 为 999;为安全起见进行分块。
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
        // 5 条最近 + 5 条旧记录
        for i in 0..5 {
            insert(&pool, now - i * 3_600_000, "recent").await;
        }
        for i in 0..5 {
            insert(&pool, old + i, "old").await;
        }
        // 下限 (1000) 远大于 10,所以 5 条最近的保留,
        // 5 条旧的也保留(未超出下限)。
        let n = purge_old(&pool, &RetentionPolicy::default(), now).await.unwrap();
        assert_eq!(n, 0);
        assert_eq!(count(&pool).await, 10);
    }

    #[tokio::test]
    async fn purge_old_purges_above_floor() {
        let pool = empty_pool().await;
        let now = 1_000_000_000_000;
        let old = now - 365 * 86_400_000;
        // 共 1500 条:1000 条最近 + 500 条旧
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
        // 超过 500 行的清理测试分块路径
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
