//! L4 —— SQLite 连接池初始化 + 设置表。
//!
//! schema（Drizzle,位于 `src/db/schema/`）由 webview 层负责。
//! Rust 命令只触及 **pool** 与一个边表
//! `_polyrocket_settings`,用于应用层级的非敏感 k/v
//! （host、chain id、feature flags）。
//!
//! 公共接口:
//! - [`init_pool`]    —— 创建 pool,应用 PRAGMA,创建设置表
//! - [`settings::get`]、[`settings::set`] —— 字符串键值存储
//! - [`settings::get_u64`]、[`settings::get_u32`]、[`settings::get_i32`] ——
//!   带默认值的强类型 getter
//!
//! L3 / L2 的调用方应使用强类型 getter;原生 `set()` 仅供
//! 内部使用（启动、变更偏好的 IPC 命令）。

pub mod audit;
pub mod bankroll;  // v0.78 —— M11
#[cfg(test)]
mod bankroll_e2e;  // v0.79b —— E2E 集成测试
pub mod bets_columns;
pub mod clob_snapshots;
pub mod migrations;  // v0.119 —— 主表迁移
pub mod paper_fills;
pub mod pool;
pub mod price_snapshots;
pub mod seed;
pub mod settings;
pub mod sidecar_health;

pub use pool::init_pool;
pub use seed::{apply_seed, is_seeded};
