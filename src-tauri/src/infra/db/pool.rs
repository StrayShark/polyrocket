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
    let db_path = db_path(app)?;
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
