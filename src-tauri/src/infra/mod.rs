//! L4 —— 基础设施层。
//!
//! 拥有所有 L3 domain 模块共享的资源：
//!
//! | 子模块        | 职责                                              |
//! |---------------|---------------------------------------------------|
//! | `error`       | `AppError` + `AppResult`（单一错误 enum）         |
//! | `state`       | `AppState`（Tauri 托管，持有 SqlitePool）         |
//! | `http`        | 共享的 `reqwest::Client` 工厂（进程级连接池）     |
//! | `db`          | SQLite 连接池初始化 + `_polyrocket_settings` k/v  |
//! | `scheduler`   | 3 个后台 tokio loop（健康探测 / 简报 / 异常检测） |
//!
//! 分层规则（见 overview.md §1.2）：
//! - L4 可依赖 L5（keyring、env、paths）—— OK
//! - L4 **不得**依赖 L3（LLM 客户端）—— 但 `scheduler`
//!   例外，它是向 L3 客户端扇出的**运行时桥**。
//!   它仅在函数调用时 `pub use` L3 客户端类型
//!   —— 依赖边是单向的：scheduler → domain::llm。
//! - L4 **不得**依赖 L1（React）或 L2（Tauri commands）

pub mod bootstrap;
pub mod db;
pub mod error;
pub mod http;
pub mod scheduler;
pub mod state;
pub mod telemetry;

// 重新导出，让 `crate::infra::AppError` 等用法更顺手，
// 调用方不必了解子模块布局。
pub use error::{AppError, AppResult};
pub use state::AppState;
