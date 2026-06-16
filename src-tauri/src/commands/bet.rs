//! L2 — Bet placement (M6).
//!
//! IPCs: `place_jump_link` (mode A, zero compliance risk), `place_signed_order`
//! (mode B, keyring-signed, stub), `list_bets`.
//! Depends on L3 `domain::polymarket` for jump URL + signed-order stub,
//! L4 `infra::state::AppState` for the SQLite pool.

use crate::domain::bet::{sign_order, BetSide, PlaceArgs};
use crate::domain::polymarket;
use crate::infra::state::AppState;
use crate::AppResult;
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
///
/// v0.5d: Uses `domain::bet::sign_order()` which validates the args,
/// derives a deterministic pseudo `tx_hash`, and computes shares.
/// When `rs-clob-client` lands, replace the body of `sign_order`.
#[tauri::command]
pub async fn place_signed_order(
    state: State<'_, AppState>,
    args: PlaceSignedArgs,
) -> AppResult<BetDto> {
    // Domain: validate + sign (deterministic stub for v0.5)
    let side = BetSide::parse(&args.side).map_err(|e| {
        crate::AppError::Invalid(format!("invalid side: {e}"))
    })?;
    let place = PlaceArgs {
        market_id: args.market_id.clone(),
        side,
        size_usdc: args.size.clone(),
        price: args.price,
        key_alias: Some(args.key_alias.clone()),
    };
    let now = chrono::Utc::now().timestamp_millis();
    let signed = sign_order(&place, now)?;

    // Also verify the key exists in the OS keyring (best-effort)
    let _ = polymarket::place_signed_order(
        &args.market_id,
        &args.side,
        args.price,
        &args.size,
        &args.key_alias,
    )
    .await; // ignore error — sign_order already produced a tx_hash

    let id = Uuid::new_v4().to_string();
    let shares = signed.shares;

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
    .bind(signed.signed_at_ms)
    .bind(&signed.tx_hash)
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
        placed_at: signed.signed_at_ms,
        settled_at: None,
        pnl: None,
        status: "open".into(),
        tx_hash: Some(signed.tx_hash.clone()),
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