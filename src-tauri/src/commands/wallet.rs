//! L2 —— 钱包管理（M3）。
//!
//! IPC:list_wallets、add_wallet。钱包元数据存于
//! SQLite（`wallets` 表）；私钥**绝不**入库 —— 由
//! L5 `platform::keyring` 存放在系统 keyring 中。

use crate::AppResult;
use crate::infra::state::AppState;
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

/// IPC: `list_wallets` —— 拉所有 wallet 元数据。
///
/// **不含私钥** —— 私钥在 OS keyring，跟 SQLite 永远不接触。
/// **排序**：`created_at DESC`。
#[tauri::command]
pub async fn list_wallets(state: State<'_, AppState>) -> AppResult<Vec<WalletDto>> {
    let rows = sqlx::query_as::<_, WalletDto>(
        "SELECT id, address, label, chain_id, wallet_type, created_at, last_synced_at FROM wallets ORDER BY created_at DESC",
    )
    .fetch_all(&state.db)
    .await?;
    Ok(rows)
}

/// IPC: `add_wallet` —— 添加一个 wallet 元数据。
///
/// **chain_id 默认 137**（Polygon 主网），L1 不传就用默认。
/// **wallet_type 默认 "eoa"**（普通 EOA 账户）。
///
/// **本 IPC 只写 metadata**（address, label, chain_id, wallet_type）——
/// 私钥走 `polyrocket_wallet_set_pk`（`commands/secrets.rs`）。
#[tauri::command]
pub async fn add_wallet(
    state: State<'_, AppState>,
    args: AddWalletArgs,
) -> AppResult<WalletDto> {
    let id = Uuid::new_v4().to_string();
    let chain_id = args.chain_id.unwrap_or(137); // Polygon 主网
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