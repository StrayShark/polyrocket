//! L2 — Tauri IPC commands (Application layer).
//!
//! One file per bounded module; each function is a `#[tauri::command]`
//! registered in `lib.rs::run()`. Commands are THIN: they read DTOs
//! from the frontend, call into L3 (`domain::*`) for business logic,
//! and persist via L4 (`infra::db`). See overview.md §3.3 for the
//! full list of 39 IPCs and the modules they live in.

pub mod audit;
pub mod bankroll;  // v0.78 — M11
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
pub mod storage; // v0.53a
pub mod storage_migrate; // v0.54b
pub mod network; // v0.56
pub mod dialog; // v0.54a
pub mod telemetry; // v0.49a
pub mod active_model; // v0.49b
pub mod clob; // v0.51a
pub mod wallet;
pub mod wallet_balance; // v0.123