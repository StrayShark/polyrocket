//! L4 — paper_fills table schema migration.
//!
//! v0.45a — adds the settlement columns (settled_at,
//! resolved_outcome, won, pnl_usdc) to the paper_fills
//! table for pre-v0.45 databases that have rows in
//! the original v0.44 schema.
//!
//! v0.50a — adds the order-type columns (order_type,
//! limit_price, stop_price, post_only) to the
//! paper_fills table for pre-v0.50 databases. These
//! mirror the `bets` table additions in commands::bet.
//!
//! SQLite does not support `ALTER TABLE ... ADD COLUMN
//! IF NOT EXISTS`, so we use the `PRAGMA table_info`
//! pattern: query the column names, ADD COLUMN only
//! if missing. Idempotent — safe to run on every boot.

use sqlx::SqlitePool;

/// v0.45a — ensure paper_fills has the settlement
/// columns. Adds 4 nullable columns:
///   - settled_at       INTEGER
///   - resolved_outcome TEXT
///   - won              INTEGER
///   - pnl_usdc         TEXT
///
/// v0.50a — also adds the order-type columns:
///   - order_type   TEXT     (default 'market')
///   - limit_price  REAL     (nullable)
///   - stop_price   REAL     (nullable)
///   - post_only    INTEGER  (default 0)
pub async fn ensure_paper_fills_columns(pool: &SqlitePool) -> sqlx::Result<()> {
    // SQLite stores columns in sqlite_master. The
    // table_info pragma gives us the column list.
    // We use (i64, String, String, i64, Option<String>, i64)
    // because PRAGMA table_info returns cid as INTEGER
    // (i64 in sqlx), and we don't need the full
    // 6-tuple struct. We only need the name (index 1).
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

    // v0.50a — order-type columns.
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
