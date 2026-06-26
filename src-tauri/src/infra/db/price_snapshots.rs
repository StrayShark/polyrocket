//! L4 —— price_snapshots 表访问。
//!
//! v0.47a —— 新增 price_snapshots 表,用于
//! 随时间记录 best-bid / best-ask（以及派生的
//! mid_price）。数据来源是 `sync_markets`
//!（目前没有真实订单簿数据源,这些值在 v0.47
//! 是占位符;未来 v0.50+ 可接入真实 book feed,
//! schema 已就绪）。
//!
//! 关键设计点:
//!   - 每个 (market_id, captured_at) 对应一行。
//!     时间的唯一性由自增 `id` 列保证;我们
//!     不需要真正的 UNIQUE 约束,因为 captured_at
//!     粒度是秒级,重复可容忍。
//!   - 旧快照由 v0.47 的 retention 扫描清理。
//!     默认保留期:30 天。
//!   - 读取很快,因为有 `price_snapshots_market_recent_idx`
//!     索引。

use sqlx::SqlitePool;

/// v0.47a —— 记录一条价格快照。调用方
///（通常是 `sync_markets`）提供 market_id、
/// captured_at 以及四个价格字段。返回插入行的 rowid。
pub async fn record_snapshot(
    pool: &SqlitePool,
    market_id: &str,
    captured_at: i64,
    best_bid: f64,
    best_ask: f64,
) -> sqlx::Result<i64> {
    let mid = (best_bid + best_ask) / 2.0;
    let spread = best_ask - best_bid;
    let res = sqlx::query(
        "INSERT INTO price_snapshots
            (market_id, captured_at, best_bid, best_ask, mid_price, spread)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(market_id)
    .bind(captured_at)
    .bind(best_bid)
    .bind(best_ask)
    .bind(mid)
    .bind(spread)
    .execute(pool)
    .await?;
    Ok(res.last_insert_rowid())
}

/// v0.47a —— 查询某 market 的最新快照（若有）。
/// 若该 market 在表中无快照（典型场景:v0.47 之后
/// 从未 sync 的 market），返回 `None`。
pub async fn latest_snapshot(
    pool: &SqlitePool,
    market_id: &str,
) -> sqlx::Result<Option<(f64, f64, f64, f64, i64)>> {
    let row: Option<(f64, f64, f64, f64, i64)> = sqlx::query_as(
        "SELECT best_bid, best_ask, mid_price, spread, captured_at
         FROM price_snapshots
         WHERE market_id = ?
         ORDER BY captured_at DESC
         LIMIT 1",
    )
    .bind(market_id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// v0.47a —— 清理早于保留窗口的快照。默认 30 天。
/// 返回被清理的行数。
pub async fn purge_old(pool: &SqlitePool, retention_ms: i64) -> sqlx::Result<u64> {
    let cutoff = chrono::Utc::now().timestamp_millis() - retention_ms;
    let res = sqlx::query("DELETE FROM price_snapshots WHERE captured_at < ?")
        .bind(cutoff)
        .execute(pool)
        .await?;
    Ok(res.rows_affected())
}
