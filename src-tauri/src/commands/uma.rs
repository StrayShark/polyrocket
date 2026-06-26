//! L2 —— UMA 争议状态（P2-3）。
//!
//! IPC:uma_dispute_status —— 返回某市场的简化版 UMA 争议状态。
//! 当前默认返回 "clear"，因为尚未接入 Polymarket UMA API。

use crate::AppResult;
use crate::domain::uma::UmaDisputeStatus;
use crate::infra::state::AppState;
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::State;

/// 包装 UMA 争议状态供前端使用的 DTO。
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct UmaDisputeStatusDto {
    pub market_id: String,
    pub status: String,
    pub detail: Option<String>,
    pub raised_at: Option<i64>,
    pub raised_by: Option<String>,
}

/// IPC:uma_dispute_status —— 返回某市场的 UMA 争议状态。
///
/// TODO(待实现)：接入 Polymarket UMA API 获取真实争议状态。
/// 当前对所有市场返回 "clear"，因为尚未接入 API。实现后，
/// 将会查询 Polymarket UMA 端点，并通过
/// `classify_uma_status` 把原始状态映射为简化版。
#[tauri::command]
pub async fn uma_dispute_status(
    state: State<'_, AppState>,
    market_id: String,
) -> AppResult<UmaDisputeStatusDto> {
    // 校验市场是否存在（找不到时返回空，但不报错
    // —— 前端可能在 in-flight 状态下调用本接口）。
    let _exists: Option<(String,)> =
        sqlx::query_as("SELECT id FROM markets WHERE id = ?")
            .bind(&market_id)
            .fetch_optional(&state.db)
            .await?;

    let status = UmaDisputeStatus {
        market_id: market_id.clone(),
        status: "clear".to_string(),
        detail: None,
        raised_at: None,
        raised_by: None,
    };

    Ok(UmaDisputeStatusDto {
        market_id: status.market_id,
        status: status.status,
        detail: status.detail,
        raised_at: status.raised_at,
        raised_by: status.raised_by,
    })
}
