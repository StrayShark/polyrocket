//! L4 — Infrastructure layer.
//!
//! Owns resources shared across all L3 domain modules:
//!
//! | sub-module | responsibility                                        |
//! |------------|-------------------------------------------------------|
//! | `error`    | `AppError` + `AppResult` (single error enum)          |
//! | `state`    | `AppState` (Tauri-managed, holds SqlitePool)          |
//! | `http`     | shared `reqwest::Client` factory (process-wide pool)  |
//! | `db`       | SQLite pool init + `_polyrocket_settings` k/v table   |
//! | `scheduler`| 3 background tokio loops (health probe / brief / anomaly) |
//!
//! Layer rules (see overview.md §1.2):
//! - L4 may depend on L5 (keyring, env, paths) — OK
//! - L4 may NOT depend on L3 (LLM clients) — except `scheduler`,
//!   which is a **runtime bridge** that fans out to L3 clients.
//!   It uses `pub use` of L3 client types at function call time
//!   only — the dependency edge is one-way: scheduler → domain::llm.
//! - L4 may NOT depend on L1 (React) or L2 (Tauri commands)

pub mod db;
pub mod error;
pub mod http;
pub mod scheduler;
pub mod state;
pub mod telemetry;

// Re-exports for ergonomic `crate::infra::AppError` etc. without
// requiring callers to know the sub-module layout.
pub use error::{AppError, AppResult};
pub use state::AppState;
