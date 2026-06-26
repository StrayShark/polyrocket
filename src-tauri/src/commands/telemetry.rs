//! L2 —— telemetry 文件管理 IPC 命令（v0.49a）。
//!
//! 运行时的开 / 关开关由 `commands::sidecar` 拥有（v0.42c）。
//! 本模块新增 v0.49a 引入的文件保留表面：
//! 列出磁盘上的会话文件，并手动触发保留清扫。
//!
//! - `list_telemetry_logs` —— 返回当前日志目录中的
//!   磁盘会话文件，并把当前进程的文件标记为 `is_current`。
//! - `purge_telemetry_logs` —— 手动触发保留清扫。
//!   返回被删除的文件数。保留窗口由
//!   `POLYROCKET_TELEMETRY_RETENTION_DAYS` 决定（默认 14）。

use crate::infra::telemetry;

#[tauri::command]
pub fn list_telemetry_logs() -> Vec<telemetry::TelemetryLogInfo> {
    telemetry::list_telemetry_logs()
}

#[tauri::command]
pub fn purge_telemetry_logs() -> Result<u64, String> {
    telemetry::purge_telemetry_logs().map_err(|e| e.to_string())
}
