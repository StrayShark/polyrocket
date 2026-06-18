//! L4 — SQLite pool + PRAGMAs.
//!
//! Mirrors the Drizzle schema declared in `src/db/schema/index.ts`.
//! Path resolution goes through [`crate::platform::paths`] so the
//! layout is owned in one place.

use crate::infra::error::AppResult;
use crate::platform::paths::{db_path, sqlite_url};
use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use tauri::{AppHandle, Manager};

/// Initialize SQLite pool in the app data dir.
///
/// Applies three PRAGMAs that match the Drizzle runtime expectations:
/// - `journal_mode = WAL`     — better concurrent reads during writes
/// - `synchronous = NORMAL`   — fsync once per commit, not per write
/// - `foreign_keys = ON`      — enforce FK constraints (off by default in SQLite)
///
/// Also creates the `_polyrocket_settings` side-table (see [`super::settings`]).
pub async fn init_pool(app: &AppHandle) -> AppResult<SqlitePool> {
    // v0.53a — resolve_db_path checks
    // `storage_path.json` first. If absent or invalid,
    // fall back to the OS default. The JSON file is
    // written by commands::storage::set_storage_path
    // AFTER the user picks a custom path; it takes
    // effect on the NEXT launch (we can't migrate
    // an already-open pool mid-flight).
    let db_path = crate::platform::paths::resolve_db_path(app)?;
    let db_url = sqlite_url(&db_path);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&db_url)
        .await?;

    sqlx::query("PRAGMA journal_mode = WAL").execute(&pool).await?;
    sqlx::query("PRAGMA synchronous = NORMAL").execute(&pool).await?;
    sqlx::query("PRAGMA foreign_keys = ON").execute(&pool).await?;

    super::settings::ensure_table(&pool).await?;
    ensure_copy_mirror_queue(&pool).await?;
    ensure_price_snapshots(&pool).await?;
    // v0.51a — clob_snapshots table (real order
    // book per market per timestamp). Idempotent.
    ensure_clob_snapshots(&pool).await?;
    // v0.45a — paper_fills settlement columns. Idempotent:
    // ALTER TABLE ADD COLUMN is a no-op if the column
    // already exists when wrapped in the IF NOT EXISTS
    // guard, BUT sqlite doesn't support IF NOT EXISTS on
    // ADD COLUMN. We use the `PRAGMA table_info` check
    // pattern instead.
    super::paper_fills::ensure_paper_fills_columns(&pool).await?;
    // v0.50a — bets order-type columns (order_type,
    // limit_price, stop_price, post_only). Same
    // idempotent pattern.
    super::bets_columns::ensure_bets_columns(&pool).await?;

    // v0.8a — first-run demo data seeder.
    // Idempotent: if the DB is already populated (e.g. user has used
    // the app before), this is a no-op. Otherwise it inserts the
    // canonical demo bundle so the UI shows a populated dashboard
    // out of the box.
    if !super::seed::is_seeded(&pool).await? {
        let inserted = super::seed::apply_seed(&pool, false).await?;
        tracing::info!(
            rows = inserted,
            "first-run seeder populated demo data"
        );
    }

    Ok(pool)
}

/// Migration helper for the mirror queue (v0.6a M5 auto-execution).
/// Called by `init_pool` so first launch after upgrade creates the table.
pub async fn ensure_copy_mirror_queue(pool: &SqlitePool) -> sqlx::Result<()> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS copy_mirror_queue (
            id TEXT PRIMARY KEY,
            event_id INTEGER NOT NULL,
            target_id TEXT NOT NULL,
            market_id TEXT NOT NULL,
            side TEXT NOT NULL,
            size TEXT NOT NULL,
            flipped INTEGER NOT NULL DEFAULT 0,
            status TEXT NOT NULL DEFAULT 'pending',
            created_at INTEGER NOT NULL,
            submitted_at INTEGER,
            filled_at INTEGER,
            bet_id TEXT,
            reject_reason TEXT
        )",
    )
    .execute(pool)
    .await?;
    sqlx::query(
        "CREATE INDEX IF NOT EXISTS mirror_queue_status_idx
         ON copy_mirror_queue(status, created_at)",
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// v0.47a — ensure the price_snapshots table +
/// its market/recent index exist. Idempotent.
pub async fn ensure_price_snapshots(pool: &SqlitePool) -> sqlx::Result<()> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS price_snapshots (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            market_id TEXT NOT NULL,
            captured_at INTEGER NOT NULL,
            best_bid REAL NOT NULL,
            best_ask REAL NOT NULL,
            mid_price REAL NOT NULL,
            spread REAL NOT NULL
        )",
    )
    .execute(pool)
    .await?;
    sqlx::query(
        "CREATE INDEX IF NOT EXISTS price_snapshots_market_recent_idx
         ON price_snapshots(market_id, captured_at DESC)",
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// v0.51a — ensure the `clob_snapshots` table +
/// its market/recent index exist. Idempotent.
///
/// `clob_snapshots` is the FULL order-book record
/// (one row per price level per side per timestamp),
/// as opposed to `price_snapshots` (one row per
/// market per timestamp with a single bid/ask pair).
pub async fn ensure_clob_snapshots(pool: &SqlitePool) -> sqlx::Result<()> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS clob_snapshots (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            market_id TEXT NOT NULL,
            captured_at INTEGER NOT NULL,
            side TEXT NOT NULL,
            price REAL NOT NULL,
            size REAL NOT NULL
        )",
    )
    .execute(pool)
    .await?;
    sqlx::query(
        "CREATE INDEX IF NOT EXISTS clob_snapshots_market_recent_idx
         ON clob_snapshots(market_id, captured_at DESC)",
    )
    .execute(pool)
    .await?;
    Ok(())
}
