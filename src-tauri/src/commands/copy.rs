//! L2 — Copy trading (M7).
//!
//! IPCs: `list_copy_targets`, `add_copy_target`, `recent_copy_events`.
//! Watches whale addresses for on-chain trades; real impl lands in
//! `domain::copy` per M7 milestone.

use crate::AppResult;
use crate::infra::state::AppState;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use tauri::State;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CopyTargetDto {
    pub id: String,
    pub address: String,
    pub label: Option<String>,
    pub enabled: bool,
    pub allocation_cap: Option<String>,
    pub min_edge: f64,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CopyEventDto {
    pub id: i64,
    pub target_id: String,
    pub market_id: String,
    pub detected_at: i64,
    pub side: String,
    pub size: String,
    pub price: f64,
    pub tx_hash: String,
    pub matched_bet_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AddCopyTargetArgs {
    pub address: String,
    pub label: Option<String>,
    pub allocation_cap: Option<String>,
    pub min_edge: Option<f64>,
}

/// IPC: `list_copy_targets` —— 拉所有 copy 跟踪目标。
///
/// **排序**：按 `created_at DESC`（最近添加的在前）。
/// **filter**：不过滤 enabled/disabled —— L1 拿到列表后自己渲染 toggle。
#[tauri::command]
pub async fn list_copy_targets(state: State<'_, AppState>) -> AppResult<Vec<CopyTargetDto>> {
    let rows = sqlx::query_as::<_, CopyTargetDto>(
        "SELECT id, address, label, enabled, allocation_cap, min_edge, created_at FROM copy_targets ORDER BY created_at DESC",
    )
    .fetch_all(&state.db)
    .await?;
    Ok(rows)
}

/// IPC: `add_copy_target` —— 添加一个 copy 跟踪目标。
///
/// **`min_edge` 默认 0.05**（5% 价差才 mirror）。
/// **`enabled` 默认 true**（刚加的 target 立即开始监控）。
///
/// **不重复校验**：调用方（`domain::copy::validate_target_args`）已经验证地址
/// 格式 + min_edge ∈ [0, 1]。这里直接 insert。
#[tauri::command]
pub async fn add_copy_target(
    state: State<'_, AppState>,
    args: AddCopyTargetArgs,
) -> AppResult<CopyTargetDto> {
    let id = Uuid::new_v4().to_string();
    let min_edge = args.min_edge.unwrap_or(0.05);
    sqlx::query(
        "INSERT INTO copy_targets (id, address, label, allocation_cap, min_edge) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&args.address)
    .bind(&args.label)
    .bind(&args.allocation_cap)
    .bind(min_edge)
    .execute(&state.db)
    .await?;
    Ok(CopyTargetDto {
        id,
        address: args.address,
        label: args.label,
        enabled: true,
        allocation_cap: args.allocation_cap,
        min_edge,
        created_at: chrono::Utc::now().timestamp_millis(),
    })
}

/// IPC: `recent_copy_events` —— 拉最近 N 条 copy event（可选按 target_id 过滤）。
///
/// **limit 默认 50**：L1 一次渲染 ≤ 50 行。
/// **排序**：`detected_at DESC`（最新在前）。
///
/// **`tx_hash` 不去重**：可能一个 tx 匹配多个 target，所以不是 unique。
#[tauri::command]
pub async fn recent_copy_events(
    state: State<'_, AppState>,
    target_id: Option<String>,
    limit: Option<i64>,
) -> AppResult<Vec<CopyEventDto>> {
    let limit = limit.unwrap_or(50);
    let rows = match target_id {
        Some(t) => {
            sqlx::query_as::<_, CopyEventDto>(
                "SELECT id, target_id, market_id, detected_at, side, size, price, tx_hash, matched_bet_id FROM copy_events WHERE target_id = ? ORDER BY detected_at DESC LIMIT ?",
            )
            .bind(t)
            .bind(limit)
            .fetch_all(&state.db)
            .await?
        }
        None => {
            sqlx::query_as::<_, CopyEventDto>(
                "SELECT id, target_id, market_id, detected_at, side, size, price, tx_hash, matched_bet_id FROM copy_events ORDER BY detected_at DESC LIMIT ?",
            )
            .bind(limit)
            .fetch_all(&state.db)
            .await?
        }
    };
    Ok(rows)
}