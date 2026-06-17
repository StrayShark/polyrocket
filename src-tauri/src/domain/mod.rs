//! L3 — Domain layer.
//!
//! Each subdirectory is one bounded domain. L3 depends on L4 (infra)
//! and L5 (platform) but never on L2 (commands) or L1 (React).
//!
//! | sub-module   | status     | owns                                          |
//! |--------------|------------|-----------------------------------------------|
//! | `llm`        | full       | 5 HTTP LLM clients + dispatch + 3 prompts    |
//! | `polymarket` | full       | read markets + jump-link + signed order stub |
//! | `consensus`  | stub v0.3c | M9 — multi-LLM side+strength aggregation      |
//! | `signal`     | stub v0.3c | M5 — model-vs-market edge detection           |
//! | `bet`        | stub v0.3c | M6 — bet lifecycle (open/won/lost)            |
//! | `copy`       | stub v0.3c | M7 — copy-trading whale watcher               |
//! | `pnl`        | stub v0.3c | M8 — dashboard KPI aggregation                |
//! | `lab`        | stub v0.3c | M5.2 — backtest / model experiments           |
//! | `wallet`     | stub v0.3c | M3 — wallet metadata (keys live in L5 keyring)|
//! | `seed`       | full v0.8a | v0.8a — first-run demo data (deterministic)   |
//! | `audit`      | full v0.8c | v0.8c — audit log retention policy (pure)     |
//!
//! Public API stability: `llm/` and `polymarket/` are used by L2 today;
//! stubs are scaffolded for the M3-M9 milestones and not yet imported
//! by L2 (their public types are still defined in L2 commands).

pub mod audit;
pub mod bet;
pub mod consensus;
pub mod copy;
pub mod lab;
pub mod llm;
pub mod mirror;
pub mod notify;
pub mod pnl;
pub mod polymarket;
pub mod seed;
pub mod sidecar_health;
pub mod signal;
pub mod wallet;
