//! L4 —— bets 表结构迁移。
//!
//! v0.50a —— 为 pre-v0.50 的数据库的 `bets` 表添加订单类型相关列
//!（order_type、limit_price、stop_price、post_only）。
//!
//! v0.51b —— 添加成交相关列（filled_at、fill_price、
//! fill_size、partial），用于真实 CLOB 执行上线时。
//! 当前（v0.51a）还没有真实执行 —— v0.5d 中的确定性桩
//! 会填充 filled_at = placed_at、fill_price = price，
//! 因此按构造滑点恒为 0。
//!
//! SQLite 不支持 `ALTER TABLE ... ADD COLUMN IF NOT EXISTS`，
//! 因此我们使用 `PRAGMA table_info` 模式：先查询列名，
//! 仅在缺失时执行 ADD COLUMN。具有幂等性 —— 每次启动都可安全运行。

use sqlx::SqlitePool;

/// v0.50a + v0.51b —— 确保 `bets` 表同时具有订单类型列和成交列，
/// 共添加 8 个列：
///   v0.50a:
///     - order_type   TEXT     （默认 'market'）
///     - limit_price  REAL     （可空）
///     - stop_price   REAL     （可空）
///     - post_only    INTEGER  （默认 0）
///   v0.51b:
///     - filled_at    INTEGER  （可空）
///     - fill_price   REAL     （可空）
///     - fill_size    TEXT     （可空 —— 使用字符串以保持
///       与现有 `size` 列的向后兼容）
///     - partial      INTEGER  （默认 0；若订单为部分
///       成交则为 1 —— v0.51+）
///
/// pre-v0.50/v0.51 的 bets 行获得默认值。
/// 现有 bets 行不会被回溯重写类型。
pub async fn ensure_bets_columns(pool: &SqlitePool) -> sqlx::Result<()> {
    let existing: Vec<(i64, String, String, i64, Option<String>, i64)> =
        sqlx::query_as("PRAGMA table_info(bets)")
            .fetch_all(pool)
            .await?;
    let names: std::collections::HashSet<String> = existing
        .into_iter()
        .map(|(_, name, _, _, _, _)| name)
        .collect();

    // v0.50a —— 订单类型列
    if !names.contains("order_type") {
        sqlx::query("ALTER TABLE bets ADD COLUMN order_type TEXT NOT NULL DEFAULT 'market'")
            .execute(pool)
            .await?;
    }
    if !names.contains("limit_price") {
        sqlx::query("ALTER TABLE bets ADD COLUMN limit_price REAL")
            .execute(pool)
            .await?;
    }
    if !names.contains("stop_price") {
        sqlx::query("ALTER TABLE bets ADD COLUMN stop_price REAL")
            .execute(pool)
            .await?;
    }
    if !names.contains("post_only") {
        sqlx::query("ALTER TABLE bets ADD COLUMN post_only INTEGER NOT NULL DEFAULT 0")
            .execute(pool)
            .await?;
    }

    // v0.51b —— 成交列
    if !names.contains("filled_at") {
        sqlx::query("ALTER TABLE bets ADD COLUMN filled_at INTEGER")
            .execute(pool)
            .await?;
    }
    if !names.contains("fill_price") {
        sqlx::query("ALTER TABLE bets ADD COLUMN fill_price REAL")
            .execute(pool)
            .await?;
    }
    if !names.contains("fill_size") {
        sqlx::query("ALTER TABLE bets ADD COLUMN fill_size TEXT")
            .execute(pool)
            .await?;
    }
    if !names.contains("partial") {
        sqlx::query("ALTER TABLE bets ADD COLUMN partial INTEGER NOT NULL DEFAULT 0")
            .execute(pool)
            .await?;
    }

    // v0.79a —— M11 资金分配链接
    if !names.contains("allocation_id") {
        // TEXT 可空，无默认值 —— pre-v0.79 的 bets 为 NULL
        //（即这些下注是手动或通过跟单产生的，
        // 而非来自资金分配批次）。
        sqlx::query("ALTER TABLE bets ADD COLUMN allocation_id TEXT")
            .execute(pool)
            .await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    // 迁移本身需要一个真实的 SqlitePool，对纯 lib 单元测试
    // 来说过重。src-tauri/tests/ 下的种子驱动的 e2e 测试
    // 已经覆盖了这条路径。
    //
    // 这里真正测试的是辅助函数的幂等性：对于一个已经
    // 拥有这些列的 SqlitePool，再次调用 ensure_bets_columns
    // 不应报错。我们使用一个临时文件后端的 pool，
    // 这样 PRAGMA 与 ALTER 的行为都与生产环境一致。

    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn make_pool() -> SqlitePool {
        let dir = std::env::temp_dir().join(format!(
            "polyrocket_bets_columns_test_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let url = format!("sqlite://{}?mode=rwc", dir.join("test.db").display());
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect(&url)
            .await
            .unwrap();
        sqlx::query(
            "CREATE TABLE bets (
                id TEXT PRIMARY KEY,
                market_id TEXT NOT NULL,
                side TEXT NOT NULL,
                size TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .unwrap();
        pool
    }

    #[tokio::test]
    async fn adds_columns_on_first_run() {
        let pool = make_pool().await;
        ensure_bets_columns(&pool).await.unwrap();
        let cols: Vec<(i64, String)> = sqlx::query_as("PRAGMA table_info(bets)")
            .fetch_all(&pool)
            .await
            .unwrap();
        let names: std::collections::HashSet<String> = cols.into_iter().map(|(_, n)| n).collect();
        for expected in ["order_type", "limit_price", "stop_price", "post_only"] {
            assert!(names.contains(expected), "missing {expected}");
        }
        // v0.79a —— M11 资金分配链接
        assert!(names.contains("allocation_id"), "missing allocation_id (v0.79a)");
    }

    #[tokio::test]
    async fn idempotent_second_run_does_not_error() {
        let pool = make_pool().await;
        ensure_bets_columns(&pool).await.unwrap();
        // 第二次运行：所有列已存在，因此 `if !names.contains(...)`
        // 守卫短路返回。
        ensure_bets_columns(&pool).await.unwrap();
        ensure_bets_columns(&pool).await.unwrap();
    }
}
