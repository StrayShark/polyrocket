//! L2 — Telemetry file-management IPC commands (v0.49a).
//!
//! The runtime on/off toggle is owned by `commands::sidecar`
//! (v0.42c). This module adds the file-retention surface
//! that v0.49a introduces: list the on-disk session files,
//! and manually trigger the retention sweep.
//!
//! - `list_telemetry_logs` — return the on-disk session
//!   files in the current log dir, with the current
//!   process's file flagged `is_current`.
//! - `purge_telemetry_logs` — manually trigger the
//!   retention sweep. Returns the count of files
//!   deleted. The retention window is
//!   `POLYROCKET_TELEMETRY_RETENTION_DAYS` (default 14).

use crate::infra::telemetry;

#[tauri::command]
pub fn list_telemetry_logs() -> Vec<telemetry::TelemetryLogInfo> {
    telemetry::list_telemetry_logs()
}

#[tauri::command]
pub fn purge_telemetry_logs() -> Result<u64, String> {
    telemetry::purge_telemetry_logs().map_err(|e| e.to_string())
}
