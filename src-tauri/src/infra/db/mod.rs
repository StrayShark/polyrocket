//! L4 — SQLite pool initialization + settings table.
//!
//! The schema (Drizzle, in `src/db/schema/`) is owned by the webview layer.
//! Rust commands touch only the **pool** and one side-table
//! `_polyrocket_settings` for app-level non-secret k/v (host, chain id,
//! feature flags).
//!
//! Public surface:
//! - [`init_pool`]    — create pool, apply PRAGMAs, create settings table
//! - [`settings::get`], [`settings::set`] — key/value string storage
//! - [`settings::get_u64`], [`settings::get_u32`], [`settings::get_i32`] —
//!   typed getters with defaults
//!
//! L3 / L2 callers should use the typed getters; raw `set()` is for
//! internal use (boot, IPC commands that mutate prefs).

pub mod audit;
pub mod bankroll;  // v0.78 — M11
#[cfg(test)]
mod bankroll_e2e;  // v0.79b — E2E integration test
pub mod bets_columns;
pub mod clob_snapshots;
pub mod migrations;  // v0.119 — primary table migrations
pub mod paper_fills;
pub mod pool;
pub mod price_snapshots;
pub mod seed;
pub mod settings;
pub mod sidecar_health;

pub use pool::init_pool;
pub use seed::{apply_seed, is_seeded};
