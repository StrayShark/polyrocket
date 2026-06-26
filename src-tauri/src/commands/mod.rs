//! L2 —— Tauri IPC commands（Application 层）。
//!
//! 每个有界模块对应一个文件；每个函数都是一个 `#[tauri::command]`，
//! 在 `lib.rs::run()` 中注册。Commands 是「薄」的：它们从 L1 读取 DTO，
//! 调用 L3（`domain::*`）执行业务逻辑，再通过 L4（`infra::db`）落库。
//! 完整的 39 个 IPC 列表及所在模块参见 overview.md §3.3。

pub mod audit;
pub mod bankroll;  // v0.78 — M11
pub mod arb;  // P1-3
pub mod bet;
pub mod brief;
pub mod calendar;  // P0-2 — market calendar
pub mod copy;
pub mod crowd_wisdom;  // Phase 1.1 — capital-weighted opinion
pub mod cross_platform_arb;  // P1-4
pub mod football_context;  // P2-2 — rest days & fatigue
pub mod llm;
pub mod llm_mgmt;
pub mod market;
pub mod mean_reversion;  // Phase 1.2 — mean reversion
pub mod mirror_executor;
pub mod news;  // P1-1
pub mod nl_query;  // P1-2
pub mod notify;
pub mod pnl;
pub mod poisson;  // P2-1 — Poisson score matrix
pub mod seed;
pub mod sidecar;
pub mod sidecar_health;
pub mod scheduler;
pub mod secrets;
pub mod signal;
pub mod smart_money;  // P0-1 — smart money score
pub mod spike;  // P0-3 — spike detection
pub mod storage; // v0.53a
pub mod storage_migrate; // v0.54b
pub mod network; // v0.56
pub mod dialog; // v0.54a
pub mod telemetry; // v0.49a
pub mod active_model; // v0.49b
pub mod clob; // v0.51a
pub mod uma;  // P2-3 — UMA dispute status
pub mod wallet;
pub mod wallet_balance; // v0.123