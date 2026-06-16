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

    Ok(pool)
}
