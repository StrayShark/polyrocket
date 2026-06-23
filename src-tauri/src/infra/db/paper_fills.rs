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
//! v0.119 — adds `ensure_paper_fills_table()` for
//! pre-v0.45 databases that don't have the paper_fills
//! table at all (the table was previously only created
//! during seed, but seed is skipped if wallets/markets/
//! bets already exist — so v0.44-era databases had
//! NO paper_fills table and crashed on first
//! v0.45a+ boot with "no such table: paper_fills").
//!
//! SQLite does not support `ALTER TABLE ... ADD COLUMN
//! IF NOT EXISTS`, so we use the `PRAGMA table_info`
//! pattern: query the column names, ADD COLUMN only
//! if missing. Idempotent — safe to run on every boot.

use sqlx::SqlitePool;

/// v0.119 — ensure the `paper_fills` table itself exists.
///
/// Previously the table was only created during first-run
/// seed (`infra::db::seed::apply_seed`), which is skipped
/// for any DB that already has wallets+markets+bets.
/// Pre-v0.45 databases therefore had no paper_fills table
/// and the v0.45a+ app crashed on boot with
/// "no such table: paper_fills" when the scheduler tried to
/// read it.
///
/// Idempotent — runs on every boot, no-op if table exists.
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
