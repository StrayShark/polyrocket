//! L4 — price_snapshots table access.
//!
//! v0.47a — adds the price_snapshots table for
//! recording best-bid / best-ask (and derived
//! mid_price) over time. The data is sourced from
//! `sync_markets` (which currently doesn't have a
//! real order-book feed, so the values are placeholders
//! for v0.47; future v0.50+ can wire in a real
//! book feed and the schema is ready).
//!
//! Key design points:
//!   - One row per (market_id, captured_at). The
//!     unique-by-time semantics come from the
//!     autoincrement `id` column; we don't bother
//!     with a real UNIQUE constraint because
//!     captured_at granularity is per-second and
//!     duplicates are tolerable.
//!   - Old snapshots are pruned by the v0.47
//!     retention sweep. Default retention: 30 days.
//!   - Reads are cheap because of the
//!     `price_snapshots_market_recent_idx` index.

use sqlx::SqlitePool;

/// v0.47a — record a single price snapshot. The
/// caller (typically `sync_markets`) supplies the
/// market_id, captured_at, and the four price
/// fields. Returns the rowid of the inserted row.
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

/// v0.47a — query the most recent snapshot for a
/// market, if any. Returns `None` if the market has
/// no snapshots in the table (typical for markets
/// that haven't been synced since v0.47).
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

/// v0.47a — purge snapshots older than the
/// retention window. Default 30 days. Returns the
/// number of rows purged.
pub async fn purge_old(pool: &SqlitePool, retention_ms: i64) -> sqlx::Result<u64> {
    let cutoff = chrono::Utc::now().timestamp_millis() - retention_ms;
    let res = sqlx::query("DELETE FROM price_snapshots WHERE captured_at < ?")
        .bind(cutoff)
        .execute(pool)
        .await?;
    Ok(res.rows_affected())
}
