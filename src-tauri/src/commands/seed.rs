//! L2 —— 演示数据填充器（M13+）。
//!
//! IPC：
//! - `seed_demo_data(force)` —— 应用标准演示数据集合
//! - `is_seeded()`           —— 报告数据库是否已经包含数据
//!
//! 首次启动时的自动填充在 `init_pool` 内完成（参见 `infra::db::seed`）；
//! 本 IPC 用于显式重新填充（Settings → "Reset demo data" 或作为开发助手）。

use crate::infra::db;
use crate::infra::error::AppResult;
use crate::infra::state::AppState;
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)] // 保留供未来的 seed_status IPC + 测试使用
pub struct SeedStatus {
    pub seeded: bool,
    pub last_seed_rows: i64,
    pub last_seed_at_ms: i64,
}

#[derive(Debug, Deserialize)]
pub struct SeedArgs {
    /// 即使已经填充过也重新应用（重置演示数据集合）。
    pub force: Option<bool>,
}

/// 应用演示数据集合。除非 `force=true`，否则幂等。
#[tauri::command]
pub async fn seed_demo_data(
    state: State<'_, AppState>,
    args: SeedArgs,
) -> AppResult<usize> {
    let force = args.force.unwrap_or(false);
    let inserted = db::apply_seed(&state.db, force).await?;
    Ok(inserted)
}

/// 返回数据库是否已包含演示数据。
#[tauri::command]
pub async fn is_seeded(state: State<'_, AppState>) -> AppResult<bool> {
    db::is_seeded(&state.db).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seed_args_default_force_is_false() {
        // 默认值为 false（幂等）；force 必须显式传入。
        let a = SeedArgs { force: None };
        assert_eq!(a.force.unwrap_or(false), false);
    }
}
