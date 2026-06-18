//! L2 — Tauri IPC commands (Application layer).
//!
//! One file per bounded module; each function is a `#[tauri::command]`
//! registered in `lib.rs::run()`. Commands are THIN: they read DTOs
//! from the frontend, call into L3 (`domain::*`) for business logic,
//! and persist via L4 (`infra::db`). See overview.md §3.3 for the
//! full list of 39 IPCs and the modules they live in.

pub mod audit;
pub mod bet;
pub mod brief;
pub mod copy;
pub mod llm;
pub mod llm_mgmt;
pub mod market;
pub mod mirror_executor;
pub mod notify;
pub mod pnl;
pub mod seed;
pub mod sidecar;
pub mod sidecar_health;
pub mod scheduler;
pub mod secrets;
pub mod signal;
pub mod telemetry; // v0.49a
pub mod wallet;