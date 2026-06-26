//! L2 —— 侧车健康快照（v0.10d）。
//!
//! IPC：
//! - `sidecar_health_now()`        —— 执行一次探测并返回快照
//! - `sidecar_health_snapshot()`   —— 返回缓存的快照（不探测）

use crate::AppResult;
use crate::domain::sidecar_health::SidecarHealthSnapshot;
use crate::infra::db;
use crate::infra::state::AppState;
use tauri::State;

/// 执行一次侧车健康探测（v0.10d 中的 no-op 桩 —— 真实探测
/// 由 L1 的「sidecar ping」命令接入）。返回探测后的快照。
#[tauri::command]
pub async fn sidecar_health_now(state: State<'_, AppState>) -> AppResult<SidecarHealthSnapshot> {
    crate::infra::scheduler::run_sidecar_health_now(&state.db)
        .await
        .map_err(crate::AppError::Db)?;
    db::sidecar_health::recent(&state.db).await
}

/// 返回缓存中的快照（不再执行新的探测）。
#[tauri::command]
pub async fn sidecar_health_snapshot(state: State<'_, AppState>) -> AppResult<SidecarHealthSnapshot> {
    db::sidecar_health::recent(&state.db).await
}
