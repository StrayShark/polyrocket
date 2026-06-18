//! L4 — bets table schema migration.
//!
//! v0.50a — adds the order-type columns (order_type,
//! limit_price, stop_price, post_only) to the `bets`
//! table for pre-v0.50 databases. The `paper_fills`
//! table gets the same additions in `paper_fills.rs`.
//!
//! SQLite does not support `ALTER TABLE ... ADD COLUMN
//! IF NOT EXISTS`, so we use the `PRAGMA table_info`
//! pattern: query the column names, ADD COLUMN only
//! if missing. Idempotent — safe to run on every boot.

use sqlx::SqlitePool;

/// v0.50a — ensure `bets` has the order-type
/// columns. Adds 4 columns:
///   - order_type   TEXT     (default 'market')
///   - limit_price  REAL     (nullable)
///   - stop_price   REAL     (nullable)
///   - post_only    INTEGER  (default 0)
///
/// Pre-v0.50 bets (those without these fields) get
/// the default values: order_type='market',
/// limit_price=NULL, stop_price=NULL, post_only=0.
/// Existing bet rows are not retroactively re-typed
/// — the L1 surfaces them as "market" implicitly.
pub async fn ensure_bets_columns(pool: &SqlitePool) -> sqlx::Result<()> {
    let existing: Vec<(i64, String, String, i64, Option<String>, i64)> =
        sqlx::query_as("PRAGMA table_info(bets)")
            .fetch_all(pool)
            .await?;
    let names: std::collections::HashSet<String> = existing
        .into_iter()
        .map(|(_, name, _, _, _, _)| name)
        .collect();

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
    Ok(())
}

#[cfg(test)]
mod tests {
    // The migration itself needs a real SqlitePool, which
    // is too heavy for a pure lib test. The seed-driven
    // e2e tests in src-tauri/tests/ cover this path.
    //
    // What we DO test here is the helper's idempotency:
    // given a SqlitePool that already has the columns,
    // calling ensure_bets_columns a second time must
    // not error. We use a temp file-backed pool so the
    // pragma and ALTER both work as in production.

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
    }

    #[tokio::test]
    async fn idempotent_second_run_does_not_error() {
        let pool = make_pool().await;
        ensure_bets_columns(&pool).await.unwrap();
        // Second run: every column already present, so
        // the `if !names.contains(...)` guard short-circuits.
        ensure_bets_columns(&pool).await.unwrap();
        ensure_bets_columns(&pool).await.unwrap();
    }
}
