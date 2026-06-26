//! L4 — 共享侧车（sidecar）状态（Tauri 命令间的单例）。
//!
//! 作为 Tauri 管理的状态持有，以便任意 `#[tauri::command]` 都可通过
//! `State<'_, SidecarState>` 访问正在运行的侧车（sidecar）进程。

pub use crate::commands::sidecar::{SidecarState, SidecarStatus};
