use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use tauri::{AppHandle, Manager};

/// Initialize SQLite pool in app data dir.
/// Mirrors the Drizzle schema declared in `src/db/schema/index.ts`.
pub async fn init_pool(app: &AppHandle) -> crate::AppResult<SqlitePool> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| crate::AppError::Internal(format!("app_data_dir: {e}")))?;
    std::fs::create_dir_all(&dir)?;
    let db_url = format!("sqlite://{}?mode=rwc", dir.join("polyrocket.db").display());

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&db_url)
        .await?;

    sqlx::query("PRAGMA journal_mode = WAL").execute(&pool).await?;
    sqlx::query("PRAGMA synchronous = NORMAL").execute(&pool).await?;
    sqlx::query("PRAGMA foreign_keys = ON").execute(&pool).await?;

    // App-level non-secret key/value settings (host, chain id, env flags).
    // Created on first launch; deliberately NOT in Drizzle schema because
    // the webview never reads it — only Rust commands touch it.
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS _polyrocket_settings (
            k TEXT PRIMARY KEY,
            v TEXT NOT NULL,
            updated_at INTEGER NOT NULL DEFAULT (unixepoch() * 1000)
        )",
    )
    .execute(&pool)
    .await?;

    Ok(pool)
}