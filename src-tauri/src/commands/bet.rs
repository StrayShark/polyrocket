//! L2 — Bet placement (M6).
//!
//! IPCs: `place_jump_link` (mode A, zero compliance risk), `place_signed_order`
//! (mode B, keyring-signed, stub), `list_bets`.
//! Depends on L3 `domain::polymarket` for jump URL + signed-order stub,
//! L4 `infra::state::AppState` for the SQLite pool.

use crate::domain::polymarket;
use crate::AppResult;
use crate::infra::state::AppState;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use tauri::State;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct BetDto {
    pub id: String,
    pub wallet_id: String,
    pub market_id: String,
    pub signal_id: Option<i64>,
    pub mode: String,
    pub side: String,
    pub size: String,
    pub price: f64,
    pub shares: String,
    pub placed_at: i64,
    pub settled_at: Option<i64>,
    pub pnl: Option<String>,
    pub status: String,
    pub tx_hash: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PlaceJumpArgs {
    pub market_slug: String,
    pub market_id: String,
    pub wallet_id: String,
    pub side: String,
    pub size: String,
    pub price: f64,
    pub signal_id: Option<i64>,
}

/// Mode A: returns the jump URL — user clicks it, signs on Polymarket UI.
/// Zero compliance risk: polyrocket never holds the key.
#[tauri::command]
pub async fn place_jump_link(args: PlaceJumpArgs) -> AppResult<String> {
    Ok(polymarket::build_jump_url(&args.market_slug, &args.side, args.price))
}

#[derive(Debug, Deserialize)]
pub struct PlaceSignedArgs {
    pub market_id: String,
    pub wallet_id: String,
    pub side: String,
    pub price: f64,
    pub size: String,
    pub signal_id: Option<i64>,
    /// Alias of the key stored in OS keyring (e.g. "primary", "trade-1")
    pub key_alias: String,
}

/// Mode B: signed order via OS keyring.
/// Phase 2 (v2 milestone). MVP only implements the audit-log write so the
/// schema path is testable.
#[tauri::command]
pub async fn place_signed_order(
    state: State<'_, AppState>,
    args: PlaceSignedArgs,
) -> AppResult<BetDto> {
    let tx_hash = polymarket::place_signed_order(
        &args.market_id,
        &args.side,
        args.price,
        &args.size,
        &args.key_alias,
    )
    .await
    .unwrap_or_default();

    let id = Uuid::new_v4().to_string();
    let placed_at = chrono::Utc::now().timestamp_millis();
    let shares = (args.size.parse::<f64>().unwrap_or(0.0) / args.price).to_string();

    sqlx::query(
        "INSERT INTO bets (id, wallet_id, market_id, signal_id, mode, side, size, price, shares, placed_at, status, tx_hash)
         VALUES (?, ?, ?, ?, 'B_signed', ?, ?, ?, ?, ?, 'open', ?)",
    )
    .bind(&id)
    .bind(&args.wallet_id)
    .bind(&args.market_id)
    .bind(args.signal_id)
    .bind(&args.side)
    .bind(&args.size)
    .bind(args.price)
    .bind(&shares)
    .bind(placed_at)
    .bind(if tx_hash.is_empty() { None } else { Some(tx_hash.as_str()) })
    .execute(&state.db)
    .await?;

    sqlx::query(
        "INSERT INTO audit_log (actor, action, target, payload, result) VALUES ('user', 'bet.place', ?, ?, 'ok')",
    )
    .bind(&args.market_id)
    .bind(serde_json::json!({"mode": "B", "size": args.size, "price": args.price}))
    .execute(&state.db)
    .await?;

    Ok(BetDto {
        id,
        wallet_id: args.wallet_id,
        market_id: args.market_id,
        signal_id: args.signal_id,
        mode: "B_signed".into(),
        side: args.side,
        size: args.size,
        price: args.price,
        shares,
        placed_at,
        settled_at: None,
        pnl: None,
        status: "open".into(),
        tx_hash: if tx_hash.is_empty() { None } else { Some(tx_hash) },
        notes: None,
    })
}

#[derive(Debug, Deserialize)]
pub struct ListBetsArgs {
    pub status: Option<String>,
    pub wallet_id: Option<String>,
    pub limit: Option<i64>,
}

#[tauri::command]
pub async fn list_bets(
    state: State<'_, AppState>,
    args: ListBetsArgs,
) -> AppResult<Vec<BetDto>> {
    let limit = args.limit.unwrap_or(100);
    let rows = sqlx::query_as::<_, BetDto>(
        "SELECT id, wallet_id, market_id, signal_id, mode, side, size, price, shares, placed_at, settled_at, pnl, status, tx_hash, notes
         FROM bets ORDER BY placed_at DESC LIMIT ?",
    )
    .bind(limit)
    .fetch_all(&state.db)
    .await?;
    Ok(rows)
}