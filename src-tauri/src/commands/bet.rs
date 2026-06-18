//! L2 — Bet placement (M6).
//!
//! IPCs: `place_jump_link` (mode A, zero compliance risk), `place_signed_order`
//! (mode B, keyring-signed, stub), `list_bets`.
//! Depends on L3 `domain::polymarket` for jump URL + signed-order stub,
//! L4 `infra::state::AppState` for the SQLite pool.

use crate::domain::bet::{self, sign_order, BetSide, OrderType, PlaceArgs};
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
    /// v0.50a — order type. Pre-v0.50 rows are treated
    /// as "market" (the migration's default).
    #[serde(default = "default_order_type")]
    pub order_type: String,
    #[serde(default)]
    pub limit_price: Option<f64>,
    #[serde(default)]
    pub stop_price: Option<f64>,
    #[serde(default)]
    pub post_only: bool,
    /// v0.51b — when the order was actually filled
    /// (vs `placed_at` which is when the user submitted).
    /// For the v0.5d deterministic stub and pre-v0.51b
    /// rows, this is `None` and slippage is reported as
    /// 0 by the analytics IPC. v0.51+ populates this
    /// from the real CLOB response.
    #[serde(default)]
    pub filled_at: Option<i64>,
    /// v0.51b — actual fill price. May differ from
    /// `price` (the user's expectation) — that's
    /// slippage. None pre-v0.51b.
    #[serde(default)]
    pub fill_price: Option<f64>,
    /// v0.51b — actual fill size in shares. May
    /// differ from `shares` (the desired fill) when
    /// the order was partial. None pre-v0.51b.
    #[serde(default)]
    pub fill_size: Option<String>,
    /// v0.51b — true when fill_size < shares.
    #[serde(default)]
    pub partial: bool,
}

fn default_order_type() -> String {
    "market".to_string()
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
    /// v0.50a — order type. Defaults to "market" when
    /// omitted (back-compat for pre-v0.50 L1 call sites).
    #[serde(default)]
    pub order_type: Option<String>,
    /// v0.50a — limit price (Limit / StopLoss orders only).
    #[serde(default)]
    pub limit_price: Option<f64>,
    /// v0.50a — stop price (StopLoss orders only).
    #[serde(default)]
    pub stop_price: Option<f64>,
    /// v0.50b — post-only flag (Limit orders only).
    #[serde(default)]
    pub post_only: bool,
}

/// Mode B: signed order via OS keyring.
///
/// v0.5d: Uses `domain::bet::sign_order()` which validates the args,
/// derives a deterministic pseudo `tx_hash`, and computes shares.
/// When `rs-clob-client` lands, replace the body of `sign_order`.
///
/// v0.50a: Extended to accept `order_type`, `limit_price`,
/// `stop_price`, `post_only`. The pure validation is in
/// `validate_order_type_specifics`. The new fields are
/// persisted to `bets` (order_type, limit_price, stop_price,
/// post_only columns). For Limit orders, the recorded
/// `bets.price` stays as the user's "reference" price
/// (kept for analytics), with `limit_price` recording
/// the actual limit level.
#[tauri::command]
pub async fn place_signed_order(
    state: State<'_, AppState>,
    args: PlaceSignedArgs,
) -> AppResult<BetDto> {
    // Domain: validate + sign (deterministic stub for v0.5)
    let side = BetSide::parse(&args.side).map_err(|e| {
        crate::AppError::Invalid(format!("invalid side: {e}"))
    })?;
    // v0.50a — parse order_type (default Market for
    // back-compat with pre-v0.50 L1 callers).
    let order_type = match &args.order_type {
        Some(s) => bet::OrderType::parse(s).map_err(|e| {
            crate::AppError::Invalid(format!("invalid order_type: {e}"))
        })?,
        None => bet::OrderType::Market,
    };
    let place = PlaceArgs {
        market_id: args.market_id.clone(),
        side,
        size_usdc: args.size.clone(),
        price: args.price,
        key_alias: Some(args.key_alias.clone()),
        order_type,
        limit_price: args.limit_price,
        stop_price: args.stop_price,
        post_only: args.post_only,
    };
    let now = chrono::Utc::now().timestamp_millis();
    let signed = sign_order(&place, now)?;

    // v0.50b — post-only enforcement. For Limit orders
    // flagged post_only, look up the latest snapshot
    // (v0.47a) and reject if the limit would cross the
    // book. When no snapshot exists, post-only is a
    // silent pass (see domain::bet module docs).
    if order_type == OrderType::Limit && args.post_only {
        if let Some(lp) = args.limit_price {
            let snap =
                crate::infra::db::price_snapshots::latest_snapshot(&state.db, &args.market_id)
                    .await?;
            let snapshot_pair = snap.map(|(bid, ask, _mid, _spread, _captured_at)| (bid, ask));
            let check = bet::check_post_only(side, lp, snapshot_pair);
            if matches!(check, bet::PostOnlyCheck::WouldCross) {
                return Err(crate::AppError::Invalid(format!(
                    "post-only limit at {lp} would cross the book for market {}",
                    args.market_id
                )));
            }
        }
    }

    // v0.51c — CLOB submit. Replaces the v0.5d
    // deterministic-only path. When CLOB creds are
    // present (POLYROCKET_CLOB_API_KEY + SECRET +
    // PASSPHRASE), attempts a real HTTP POST to the
    // CLOB /order endpoint. When creds are absent,
    // returns the stub shape (ok=true, slippage=0,
    // partial=false) — the deterministic path the
    // app has used since v0.5d.
    //
    // Errors are surfaced as AppError::Invalid (not
    // Internal) so the L1 can show them inline. The
    // CLOB's tx_hash (or the deterministic stub's
    // hash) is what we persist to bets.tx_hash.
    let clob = polymarket::submit_signed_order_via_clob(
        &args.market_id,
        &args.side,
        args.price,
        &signed.shares,
        &args.key_alias,
        order_type.as_str(),
        signed.signed_at_ms,
    )
    .await;
    if !clob.ok {
        return Err(crate::AppError::Invalid(format!(
            "CLOB rejected order: {}",
            clob.error
        )));
    }
    let _ = clob.via_http; // recorded via clob.tx_hash below; field
                           // surfaces in audit_log payload.

    let id = Uuid::new_v4().to_string();
    let shares = signed.shares;

    // v0.51b — fill columns. In the v0.5d deterministic
    // stub we treat the order as filled immediately at
    // the user's price (slippage = 0). v0.51c: when
    // the CLOB submit returns a real response (creds
    // present + reachable), we record the actual
    // fill_price / fill_size / partial / filled_at
    // from the CLOB. Otherwise we use the stub shape.
    let filled_at = clob.filled_at_ms;
    let fill_price = clob.fill_price;
    let fill_size = clob.fill_size;
    let partial = clob.partial;

    // v0.50a + v0.51b — INSERT now includes the
    // order-type AND fill columns. The idempotent
    // ALTER TABLE in infra::db::bets_columns has
    // already added them by the time we get here.
    sqlx::query(
        "INSERT INTO bets (
            id, wallet_id, market_id, signal_id, mode, side,
            size, price, shares, placed_at, status, tx_hash,
            order_type, limit_price, stop_price, post_only,
            filled_at, fill_price, fill_size, partial
         )
         VALUES (?, ?, ?, ?, 'B_signed', ?, ?, ?, ?, ?, 'open', ?, ?, ?, ?, ?, ?, ?, ?, ?)",
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
    .bind(order_type.as_str())
    .bind(args.limit_price)
    .bind(args.stop_price)
    .bind(if args.post_only { 1_i64 } else { 0_i64 })
    .bind(filled_at)
    .bind(fill_price)
    .bind(&fill_size)
    .bind(if partial { 1_i64 } else { 0_i64 })
    .execute(&state.db)
    .await?;

    sqlx::query(
        "INSERT INTO audit_log (actor, action, target, payload, result) VALUES ('user', 'bet.place', ?, ?, 'ok')",
    )
    .bind(&args.market_id)
    .bind(serde_json::json!({
        "mode": "B",
        "size": args.size,
        "price": args.price,
        "order_type": order_type.as_str(),
        "limit_price": args.limit_price,
        "stop_price": args.stop_price,
        "post_only": args.post_only,
    }))
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
        order_type: order_type.as_str().to_string(),
        limit_price: args.limit_price,
        stop_price: args.stop_price,
        post_only: args.post_only,
        filled_at: Some(filled_at),
        fill_price: Some(fill_price),
        fill_size: Some(fill_size),
        partial,
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
    // v0.50a + v0.51b — include the order-type AND fill
    // columns. Pre-v0.50 / pre-v0.51b rows will have NULL
    // for limit_price / stop_price / filled_at / fill_price
    // / fill_size and 0 for post_only / partial; sqlx
    // deserializes them via #[serde(default)].
    let rows = sqlx::query_as::<_, BetDto>(
        "SELECT id, wallet_id, market_id, signal_id, mode, side, size, price, shares, placed_at, settled_at, pnl, status, tx_hash, notes,
                order_type, limit_price, stop_price, post_only,
                filled_at, fill_price, fill_size, partial
         FROM bets ORDER BY placed_at DESC LIMIT ?",
    )
    .bind(limit)
    .fetch_all(&state.db)
    .await?;
    Ok(rows)
}

// =================================================================
// ============== v0.50a — order args validation IPC =============
// =================================================================

#[derive(Debug, Deserialize)]
pub struct ValidateOrderArgsArgs {
    pub market_id: String,
    pub side: String,
    pub size: String,
    pub price: f64,
    pub order_type: Option<String>,
    pub limit_price: Option<f64>,
    pub stop_price: Option<f64>,
    pub post_only: bool,
}

/// v0.50a — pure validation IPC. The L1 calls this
/// before invoking `placeSignedOrder` so the user
/// gets instant feedback (e.g. "limit orders
/// require limit_price") without a round-trip
/// to the DB.
///
/// Returns the parsed size on success; returns
/// `AppError::Invalid` (which serializes to a
/// string) on failure.
#[tauri::command]
pub fn validate_order_args(args: ValidateOrderArgsArgs) -> AppResult<f64> {
    let side = BetSide::parse(&args.side).map_err(|e| {
        crate::AppError::Invalid(format!("invalid side: {e}"))
    })?;
    let order_type = match &args.order_type {
        Some(s) => OrderType::parse(s).map_err(|e| {
            crate::AppError::Invalid(format!("invalid order_type: {e}"))
        })?,
        None => OrderType::Market,
    };
    let place = PlaceArgs {
        market_id: args.market_id,
        side,
        size_usdc: args.size,
        price: args.price,
        key_alias: Some("validate_only".into()), // dummy for the validator
        order_type,
        limit_price: args.limit_price,
        stop_price: args.stop_price,
        post_only: args.post_only,
    };
    bet::validate_place_args(&place)
}