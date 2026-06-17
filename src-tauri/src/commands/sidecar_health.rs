//! L2 — Sidecar health snapshot (v0.10d).
//!
//! IPCs:
//! - `sidecar_health_now()`        — run a single probe + return the snapshot
//! - `sidecar_health_snapshot()`   — return the cached snapshot (no probe)

use crate::AppResult;
use crate::domain::sidecar_health::SidecarHealthSnapshot;
use crate::infra::db;
use crate::infra::state::AppState;
use tauri::State;

/// Run a single sidecar health probe (no-op stub for v0.10d —
/// the real probe is wired by the L1 "sidecar ping" command).
/// Returns the snapshot after the probe.
#[tauri::command]
pub async fn sidecar_health_now(state: State<'_, AppState>) -> AppResult<SidecarHealthSnapshot> {
    crate::infra::scheduler::run_sidecar_health_now(&state.db)
        .await
        .map_err(crate::AppError::Db)?;
    db::sidecar_health::recent(&state.db).await
}

/// Return the cached snapshot (no new probe).
#[tauri::command]
pub async fn sidecar_health_snapshot(state: State<'_, AppState>) -> AppResult<SidecarHealthSnapshot> {
    db::sidecar_health::recent(&state.db).await
}
