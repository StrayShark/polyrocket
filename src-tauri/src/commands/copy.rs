use crate::AppResult;
use crate::state::AppState;
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

#[tauri::command]
pub async fn list_copy_targets(state: State<'_, AppState>) -> AppResult<Vec<CopyTargetDto>> {
    let rows = sqlx::query_as::<_, CopyTargetDto>(
        "SELECT id, address, label, enabled, allocation_cap, min_edge, created_at FROM copy_targets ORDER BY created_at DESC",
    )
    .fetch_all(&state.db)
    .await?;
    Ok(rows)
}

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