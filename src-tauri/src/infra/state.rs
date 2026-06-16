//! L4 — Shared application state passed to every Tauri command.
//!
//! Currently holds only the SQLite pool. The pool is `Clone` (internally
//! an `Arc`) so commands that take `State<AppState>` can also clone it
//! out for worker tasks. The `SchedulerHandle` is managed separately
//! (in `setup`) because not every command needs it.

use sqlx::SqlitePool;

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
}
