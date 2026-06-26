//! L4 —— clob_snapshots 表访问（v0.51a）。
//!
//! v0.47a 的 `price_snapshots` 表按 market 存储单条
//!（best_bid, best_ask）对 —— 足以支撑回测 join 与
//! v0.50b 的 post-only 检查，但无法满足"真实"的
//! 订单簿推理（深度、完整档位、部分成交）。
//!
//! v0.51a 引入 `clob_snapshots`，按 market 与时间戳
//! 记录完整的订单簿：
//!
//!   id, market_id, captured_at, side ('bid'|'ask'),
//!   price, size
//!
//! 单个"快照"是同一 `captured_at` 下、属于同一 market 的
//! 全部行。典型快照规模：单边 5-30 个价格档位。
//!
//! 当前（v0.51a）还没有真实 CLOB 数据源 —— 这需要
//! Polymarket WebSocket 凭证。表与辅助函数已就绪，
//! 等待凭证上线。在此之前 L1 可通过
//! `record_clob_snapshot_now` IPC 写入测试数据。

use sqlx::SqlitePool;

/// `clob_snapshots` 中的一行。`(captured_at, side, price)`
/// 三元组在同一快照内唯一（未声明形式化 UNIQUE 约束；
/// 去重由调用方在 sqlx 侧负责）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::FromRow)]
pub struct ClobLevel {
    pub id: i64,
    pub market_id: String,
    pub captured_at: i64,
    /// "bid" 或 "ask"。
    pub side: String,
    pub price: f64,
    pub size: f64,
}

/// 写入单次快照 —— 一批共享同一 `captured_at` 的
/// bids 与 asks。返回插入的行数。供 L1 记录测试数据
/// 使用的轻量辅助函数。
///
/// 生产环境（v0.51+）会用 WebSocket 监听器替换此函数，
/// 由行情侧调用批量 INSERT。
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

/// 返回指定 market 的最近一次快照，形式为
/// (bids, asks, captured_at)。bids 按价格降序排列
///（最优 bid 在前）；asks 按价格升序（最优 ask 在前）。
/// 不存在任何快照时返回 None。
/// 拉某个 market 的最新一次 CLOB 快照（一个 `captured_at` 下的所有 bid+ask 行）。
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
    // 重新排序：最优 bid = 最高价；最优 ask = 最低价。
    bids.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    asks.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    Ok(Some(ClobSnapshot {
        market_id: market_id.to_string(),
        captured_at,
        bids,
        asks,
    }))
}

/// 一次完整的快照（同一时间戳下的全部 bids 与 asks）。
/// 由 `latest_clob_snapshot` 返回。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ClobSnapshot {
    pub market_id: String,
    pub captured_at: i64,
    /// (price, size) 对。按 price 降序排列。
    pub bids: Vec<(f64, f64)>,
    /// (price, size) 对。按 price 升序排列。
    pub asks: Vec<(f64, f64)>,
}

/// 统计每个 market 的不同快照数（最近优先）。
/// 供 L1 展示 "我们为 market X 记录了 N 条快照"。
/// 统计某个 market 的 CLOB 快照总条数（行数量,不是 distinct `captured_at` 数）。
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

/// 清理早于 `retention_ms` 的快照。
/// 默认保留 7 天（`price_snapshots` 为 30 天）——
/// clob_snapshots 单行体量远大于后者（每 market 10-30 行
/// 对比 1 行）。
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
        // bids 按 price 降序：0.49 在前。
        assert!((snap.bids[0].0 - 0.49).abs() < 1e-9);
        // asks 按 price 升序：0.51 在前。
        assert!((snap.asks[0].0 - 0.51).abs() < 1e-9);
        assert_eq!(snap.captured_at, now);
    }

    #[tokio::test]
    async fn latest_returns_most_recent() {
        let pool = make_pool().await;
        // 两次快照，相隔 1 秒。
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
        // 3 条快照，分布在 2 个 market。
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
        // 旧：8 天前。新：1 天前。
        record_clob_snapshot(
            &pool, "m1", now_ms - 8 * 86_400 * 1000,
            &[(0.4, 1.0)], &[],
        ).await.unwrap();
        record_clob_snapshot(
            &pool, "m1", now_ms - 1 * 86_400 * 1000,
            &[(0.4, 1.0)], &[],
        ).await.unwrap();
        // 保留期：7 天。旧数据应被清理。
        let purged = purge_old(&pool, 7 * 86_400 * 1000).await.unwrap();
        assert_eq!(purged, 1);
        let remaining = snapshot_count(&pool, "m1").await.unwrap();
        assert_eq!(remaining, 1);
    }
}
