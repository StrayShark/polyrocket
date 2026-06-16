use crate::AppResult;
use crate::state::AppState;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use tauri::State;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct WalletDto {
    pub id: String,
    pub address: String,
    pub label: Option<String>,
    pub chain_id: i64,
    pub wallet_type: String,
    pub created_at: i64,
    pub last_synced_at: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct AddWalletArgs {
    pub address: String,
    pub label: Option<String>,
    pub chain_id: Option<i64>,
    pub wallet_type: Option<String>,
}

#[tauri::command]
pub async fn list_wallets(state: State<'_, AppState>) -> AppResult<Vec<WalletDto>> {
    let rows = sqlx::query_as::<_, WalletDto>(
        "SELECT id, address, label, chain_id, wallet_type, created_at, last_synced_at FROM wallets ORDER BY created_at DESC",
    )
    .fetch_all(&state.db)
    .await?;
    Ok(rows)
}

#[tauri::command]
pub async fn add_wallet(
    state: State<'_, AppState>,
    args: AddWalletArgs,
) -> AppResult<WalletDto> {
    let id = Uuid::new_v4().to_string();
    let chain_id = args.chain_id.unwrap_or(137); // Polygon mainnet
    let wallet_type = args.wallet_type.unwrap_or_else(|| "eoa".to_string());

    sqlx::query(
        "INSERT INTO wallets (id, address, label, chain_id, wallet_type) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&args.address)
    .bind(&args.label)
    .bind(chain_id)
    .bind(&wallet_type)
    .execute(&state.db)
    .await?;

    Ok(WalletDto {
        id,
        address: args.address,
        label: args.label,
        chain_id,
        wallet_type,
        created_at: chrono::Utc::now().timestamp_millis(),
        last_synced_at: None,
    })
}