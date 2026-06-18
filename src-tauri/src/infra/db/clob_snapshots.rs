//! L4 — clob_snapshots table access (v0.51a).
//!
//! The `price_snapshots` table from v0.47a stores a
//! single (best_bid, best_ask) pair per market — useful
//! for backtest joins and the v0.50b post-only check,
//! but not for "real" order-book reasoning (depth, full
//! ladder, partial fills).
//!
//! v0.51a introduces `clob_snapshots`, which records the
//! FULL order book per market per timestamp:
//!
//!   id, market_id, captured_at, side ('bid'|'ask'),
//!   price, size
//!
//! A single "snapshot" is the set of rows that share
//! a `captured_at` value for a given market. Typical
//! snapshot sizes: 5-30 price levels per side.
//!
//! Today (v0.51a) we don't yet have a real CLOB feed —
//! that requires Polymarket WebSocket credentials. The
//! table + helpers are ready for when those land. In
//! the meantime the L1 can populate via the
//! `record_clob_snapshot_now` IPC for testing.

use sqlx::SqlitePool;

/// One row in `clob_snapshots`. The `(captured_at,
/// side, price)` triple is unique per snapshot
/// (no formal UNIQUE constraint; sqlx-side dedup
/// is the caller's responsibility).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::FromRow)]
pub struct ClobLevel {
    pub id: i64,
    pub market_id: String,
    pub captured_at: i64,
    /// "bid" or "ask".
    pub side: String,
    pub price: f64,
    pub size: f64,
}

/// Insert a single snapshot — a batch of bids and
/// asks all sharing the same `captured_at`. Returns
/// the number of rows inserted. Trivial helper for
/// the L1 to record test data.
///
/// Production usage (v0.51+) will replace this with
/// a WebSocket listener that calls a bulk INSERT
/// from the feed side.
pub async fn record_clob_snapshot(
    pool: &SqlitePool,
    market_id: &str,
    captured_at: i64,
    bids: &[(f64, f64)],
    asks: &[(f64, f64)],
) -> sqlx::Result<usize> {
    let mut tx = pool.begin().await?;
    let mut inserted = 0usize;
    for (price, size) in bids {
        sqlx::query(
            "INSERT INTO clob_snapshots
                (market_id, captured_at, side, price, size)
             VALUES (?, ?, 'bid', ?, ?)",
        )
        .bind(market_id)
        .bind(captured_at)
        .bind(price)
        .bind(size)
        .execute(&mut *tx)
        .await?;
        inserted += 1;
    }
    for (price, size) in asks {
        sqlx::query(
            "INSERT INTO clob_snapshots
                (market_id, captured_at, side, price, size)
             VALUES (?, ?, 'ask', ?, ?)",
        )
        .bind(market_id)
        .bind(captured_at)
        .bind(price)
        .bind(size)
        .execute(&mut *tx)
        .await?;
        inserted += 1;
    }
    tx.commit().await?;
    Ok(inserted)
}

/// Return the most recent snapshot for a market as
/// (bids, asks, captured_at). Bids are sorted by
/// price DESC (best bid first); asks by price ASC
/// (best ask first). Returns None when no snapshots
/// exist.
/// 拉某个 market 的最新一次 CLOB 快照（一个 `captured_at` 下的所有 bid+ask rows）。
///
/// **返回**：`Vec<ClobLevel>` —— bid/ask 都按 price 排序。空 Vec 表示该 market
/// 从未记录过快照。
pub async fn latest_clob_snapshot(
    pool: &SqlitePool,
    market_id: &str,
) -> sqlx::Result<Option<ClobSnapshot>> {
    let row: Option<(i64,)> = sqlx::query_as(
        "SELECT captured_at FROM clob_snapshots
         WHERE market_id = ?
         ORDER BY captured_at DESC
         LIMIT 1",
    )
    .bind(market_id)
    .fetch_optional(pool)
    .await?;
    let Some((captured_at,)) = row else {
        return Ok(None);
    };
    let levels: Vec<ClobLevel> = sqlx::query_as::<_, ClobLevel>(
        "SELECT id, market_id, captured_at, side, price, size
         FROM clob_snapshots
         WHERE market_id = ? AND captured_at = ?
         ORDER BY side ASC, price ASC",
    )
    .bind(market_id)
    .bind(captured_at)
    .fetch_all(pool)
    .await?;
    let mut bids: Vec<(f64, f64)> = Vec::new();
    let mut asks: Vec<(f64, f64)> = Vec::new();
    for l in levels {
        match l.side.as_str() {
            "bid" => bids.push((l.price, l.size)),
            "ask" => asks.push((l.price, l.size)),
            _ => {}
        }
    }
    // Re-sort: best bid = highest price; best ask = lowest.
    bids.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    asks.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    Ok(Some(ClobSnapshot {
        market_id: market_id.to_string(),
        captured_at,
        bids,
        asks,
    }))
}

/// One full snapshot (all bids + all asks at one
/// timestamp). Returned by `latest_clob_snapshot`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ClobSnapshot {
    pub market_id: String,
    pub captured_at: i64,
    /// (price, size) pairs. Sorted DESC by price.
    pub bids: Vec<(f64, f64)>,
    /// (price, size) pairs. Sorted ASC by price.
    pub asks: Vec<(f64, f64)>,
}

/// Number of distinct snapshots per market (most
/// recent first). Used by the L1 to show "we have
/// N snapshots for market X".
/// 统计某个 market 的 CLOB 快照总条数（rows 数量，不是 distinct `captured_at` 数）。
///
/// **用途**：L1 「History → Order Book」展示 + retention 决策。
pub async fn snapshot_count(
    pool: &SqlitePool,
    market_id: &str,
) -> sqlx::Result<i64> {
    let n: i64 = sqlx::query_scalar(
        "SELECT COUNT(DISTINCT captured_at) FROM clob_snapshots
         WHERE market_id = ?",
    )
    .bind(market_id)
    .fetch_one(pool)
    .await?;
    Ok(n)
}

/// Purge snapshots older than `retention_ms`.
/// Default retention is 7 days (vs 30 for
/// `price_snapshots`) — clob_snapshots are much
/// larger per row (10-30 vs 1 per market).
/// 删除某个 market 超过 `keep` 条数的历史快照。返回删除行数。
///
/// **策略**：保留最近 `keep` 条（按 `captured_at` DESC 排序），删剩下的。
/// 这是 LRU 风格的截断，不按时间窗口 —— 因为不同 market 的 CLOB 活跃度差很多。
///
/// **调用方**：手动 + 未来 scheduler（按 v0.51a+ 的存储策略决定）。
pub async fn purge_old(
    pool: &SqlitePool,
    retention_ms: i64,
) -> sqlx::Result<u64> {
    let cutoff = chrono::Utc::now().timestamp_millis() - retention_ms;
    let res = sqlx::query("DELETE FROM clob_snapshots WHERE captured_at < ?")
        .bind(cutoff)
        .execute(pool)
        .await?;
    Ok(res.rows_affected())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn make_pool() -> SqlitePool {
        let dir = std::env::temp_dir().join(format!(
            "polyrocket_clob_snapshots_test_{}",
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
            "CREATE TABLE clob_snapshots (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                market_id TEXT NOT NULL,
                captured_at INTEGER NOT NULL,
                side TEXT NOT NULL,
                price REAL NOT NULL,
                size REAL NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "CREATE INDEX clob_snapshots_market_recent_idx
             ON clob_snapshots(market_id, captured_at DESC)",
        )
        .execute(&pool)
        .await
        .unwrap();
        pool
    }

    #[tokio::test]
    async fn record_and_retrieve_snapshot() {
        let pool = make_pool().await;
        let now = 1_700_000_000_000_i64;
        let bids = vec![(0.49, 100.0), (0.48, 200.0), (0.47, 300.0)];
        let asks = vec![(0.51, 100.0), (0.52, 200.0), (0.53, 300.0)];
        let n = record_clob_snapshot(&pool, "m1", now, &bids, &asks)
            .await
            .unwrap();
        assert_eq!(n, 6);
        let snap = latest_clob_snapshot(&pool, "m1").await.unwrap().unwrap();
        assert_eq!(snap.bids.len(), 3);
        assert_eq!(snap.asks.len(), 3);
        // bids sorted DESC: 0.49 first.
        assert!((snap.bids[0].0 - 0.49).abs() < 1e-9);
        // asks sorted ASC: 0.51 first.
        assert!((snap.asks[0].0 - 0.51).abs() < 1e-9);
        assert_eq!(snap.captured_at, now);
    }

    #[tokio::test]
    async fn latest_returns_most_recent() {
        let pool = make_pool().await;
        // Two snapshots, 1 second apart.
        record_clob_snapshot(
            &pool, "m1", 1_000,
            &[(0.40, 10.0)], &[(0.50, 10.0)],
        ).await.unwrap();
        record_clob_snapshot(
            &pool, "m1", 2_000,
            &[(0.45, 20.0)], &[(0.55, 20.0)],
        ).await.unwrap();
        let snap = latest_clob_snapshot(&pool, "m1").await.unwrap().unwrap();
        assert_eq!(snap.captured_at, 2_000);
        assert!((snap.bids[0].0 - 0.45).abs() < 1e-9);
    }

    #[tokio::test]
    async fn snapshot_count_distinct_timestamps() {
        let pool = make_pool().await;
        // 3 snapshots, 2 markets.
        record_clob_snapshot(&pool, "m1", 1_000, &[(0.4, 1.0)], &[]).await.unwrap();
        record_clob_snapshot(&pool, "m1", 2_000, &[(0.4, 1.0)], &[]).await.unwrap();
        record_clob_snapshot(&pool, "m2", 1_000, &[(0.4, 1.0)], &[]).await.unwrap();
        assert_eq!(snapshot_count(&pool, "m1").await.unwrap(), 2);
        assert_eq!(snapshot_count(&pool, "m2").await.unwrap(), 1);
    }

    #[tokio::test]
    async fn purge_old_removes_only_old_rows() {
        let pool = make_pool().await;
        let now_ms = chrono::Utc::now().timestamp_millis();
        // Old: 8 days ago. Recent: 1 day ago.
        record_clob_snapshot(
            &pool, "m1", now_ms - 8 * 86_400 * 1000,
            &[(0.4, 1.0)], &[],
        ).await.unwrap();
        record_clob_snapshot(
            &pool, "m1", now_ms - 1 * 86_400 * 1000,
            &[(0.4, 1.0)], &[],
        ).await.unwrap();
        // Retention: 7 days. Old should be purged.
        let purged = purge_old(&pool, 7 * 86_400 * 1000).await.unwrap();
        assert_eq!(purged, 1);
        let remaining = snapshot_count(&pool, "m1").await.unwrap();
        assert_eq!(remaining, 1);
    }
}
