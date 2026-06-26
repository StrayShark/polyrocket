//! L4 —— paper_fills 表结构迁移。
//!
//! v0.45a —— 为 pre-v0.45 数据库的 paper_fills 表添加结算列
//!（settled_at、resolved_outcome、won、pnl_usdc），
//! 这些数据库的 paper_fills 表仍是 v0.44 原始 schema。
//!
//! v0.50a —— 为 pre-v0.50 数据库的 paper_fills 表添加
//! 订单类型列（order_type、limit_price、stop_price、post_only）。
//! 这些列与 commands::bet 中对 `bets` 表的添加保持一致。
//!
//! v0.119 —— 新增 `ensure_paper_fills_table()`，处理
//! 完全没有 paper_fills 表的 pre-v0.45 数据库（此前该表
//! 只在 seed 阶段创建，但当 wallets/markets/bets 已存在时
//! 会跳过 seed —— 因此 v0.44 时代的数据库没有
//! paper_fills 表，在 v0.45a+ 首次启动时因
//! "no such table: paper_fills" 崩溃）。
//!
//! SQLite 不支持 `ALTER TABLE ... ADD COLUMN IF NOT EXISTS`，
//! 因此我们使用 `PRAGMA table_info` 模式：先查询列名，
//! 仅在缺失时执行 ADD COLUMN。具有幂等性 —— 每次启动都可安全运行。

use sqlx::SqlitePool;

/// v0.119 —— 确保 `paper_fills` 表本身存在。
///
/// 此前该表仅在首次运行的 seed（`infra::db::seed::apply_seed`）
/// 中创建，对任何已存在 wallets+markets+bets 的数据库会被跳过。
/// 因此 pre-v0.45 数据库没有 paper_fills 表，
/// v0.45a+ 应用在启动时因 "no such table: paper_fills" 崩溃，
/// scheduler 读取该表时崩溃。
///
/// 具有幂等性 —— 每次启动都运行，若表已存在则 no-op。
pub async fn ensure_paper_fills_table(pool: &SqlitePool) -> sqlx::Result<()> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS paper_fills (
            id TEXT PRIMARY KEY,
            mirror_id TEXT NOT NULL,
            market_id TEXT NOT NULL,
            side TEXT NOT NULL,
            size TEXT NOT NULL,
            price REAL NOT NULL,
            placed_at INTEGER NOT NULL,
            notes TEXT,
            settled_at INTEGER,
            resolved_outcome TEXT,
            won INTEGER,
            pnl_usdc TEXT,
            order_type TEXT NOT NULL DEFAULT 'market',
            limit_price REAL,
            stop_price REAL,
            post_only INTEGER NOT NULL DEFAULT 0
        )",
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// v0.45a —— 确保 paper_fills 具有结算列。添加 4 个可空列：
///   - settled_at       INTEGER
///   - resolved_outcome TEXT
///   - won              INTEGER
///   - pnl_usdc         TEXT
///
/// v0.50a —— 同时添加订单类型列：
///   - order_type   TEXT     （默认 'market'）
///   - limit_price  REAL     （可空）
///   - stop_price   REAL     （可空）
///   - post_only    INTEGER  （默认 0）
pub async fn ensure_paper_fills_columns(pool: &SqlitePool) -> sqlx::Result<()> {
    // SQLite 将列存储在 sqlite_master 中。table_info pragma
    // 返回列列表。我们使用 (i64, String, String, i64, Option<String>, i64)
    // 是因为 PRAGMA table_info 的 cid 在 sqlx 中为 INTEGER（i64），
    // 且我们不需要完整的 6 元组结构，只需要 name（index 1）。
    let existing: Vec<(i64, String, String, i64, Option<String>, i64)> =
        sqlx::query_as("PRAGMA table_info(paper_fills)")
            .fetch_all(pool)
            .await?;
    let names: std::collections::HashSet<String> = existing
        .into_iter()
        .map(|(_, name, _, _, _, _)| name)
        .collect();

    if !names.contains("settled_at") {
        sqlx::query("ALTER TABLE paper_fills ADD COLUMN settled_at INTEGER")
            .execute(pool)
            .await?;
    }
    if !names.contains("resolved_outcome") {
        sqlx::query("ALTER TABLE paper_fills ADD COLUMN resolved_outcome TEXT")
            .execute(pool)
            .await?;
    }
    if !names.contains("won") {
        sqlx::query("ALTER TABLE paper_fills ADD COLUMN won INTEGER")
            .execute(pool)
            .await?;
    }
    if !names.contains("pnl_usdc") {
        sqlx::query("ALTER TABLE paper_fills ADD COLUMN pnl_usdc TEXT")
            .execute(pool)
            .await?;
    }

    // v0.50a —— 订单类型列。
    if !names.contains("order_type") {
        sqlx::query("ALTER TABLE paper_fills ADD COLUMN order_type TEXT NOT NULL DEFAULT 'market'")
            .execute(pool)
            .await?;
    }
    if !names.contains("limit_price") {
        sqlx::query("ALTER TABLE paper_fills ADD COLUMN limit_price REAL")
            .execute(pool)
            .await?;
    }
    if !names.contains("stop_price") {
        sqlx::query("ALTER TABLE paper_fills ADD COLUMN stop_price REAL")
            .execute(pool)
            .await?;
    }
    if !names.contains("post_only") {
        sqlx::query("ALTER TABLE paper_fills ADD COLUMN post_only INTEGER NOT NULL DEFAULT 0")
            .execute(pool)
            .await?;
    }
    Ok(())
}
