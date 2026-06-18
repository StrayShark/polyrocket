//! L2 — Mirror executor IPCs (M5 auto-execution).
//!
//! Manages the `copy_mirror_queue` table in SQLite and runs the
//! domain::mirror executor to pick which pending mirrors to submit
//! as Mode B bets.
//!
//! Flow:
//! 1. L4 scheduler (infra::scheduler) ticks every N seconds
//! 2. Calls `run_mirror_executor_pass` IPC (or directly via Rust)
//! 3. The pass reads pending mirrors, runs pick_next_mirror + find_rejections
//! 4. Picked mirrors are submitted as Mode B bets via sign_order
//! 5. Rejected mirrors are marked with their reason

use crate::AppError;
use crate::AppResult;
use crate::domain::bet::sign_order;
use crate::domain::copy::{MirrorOrder, MirrorStatus};
use crate::domain::mirror::{
    current_exposure, execute_pass, ExecutorConfig, ExecutorPassResult, RejectReason,
};
use crate::infra::state::AppState;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Row};
use std::collections::HashMap;
use tauri::State;

/// Persisted mirror order (DB row). Mirrors `copy_mirror_queue` table.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct MirrorRow {
    pub id: String,
    pub event_id: i64,
    pub target_id: String,
    pub market_id: String,
    pub side: String,
    pub size: String,
    pub flipped: bool,
    pub status: String,
    pub created_at: i64,
    pub submitted_at: Option<i64>,
    pub filled_at: Option<i64>,
    pub bet_id: Option<String>,
    pub reject_reason: Option<String>,
}

impl From<MirrorRow> for MirrorOrder {
    fn from(r: MirrorRow) -> Self {
        MirrorOrder {
            id: r.id,
            event_id: r.event_id,
            target_id: r.target_id,
            market_id: r.market_id,
            side: r.side,
            size: r.size,
            flipped: r.flipped,
            status: MirrorStatus::parse(&r.status).unwrap_or(MirrorStatus::Pending),
            created_at: r.created_at,
            submitted_at: r.submitted_at,
            filled_at: r.filled_at,
            bet_id: r.bet_id,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct EnqueueArgs {
    pub event_id: i64,
    pub target_id: String,
    pub market_id: String,
    pub side: String,
    pub size: String,
    pub flipped: bool,
}

/// Enqueue a new mirror (called when should_mirror returns a decision).
#[tauri::command]
pub async fn enqueue_mirror(
    state: State<'_, AppState>,
    args: EnqueueArgs,
) -> AppResult<MirrorRow> {
    let now = chrono::Utc::now().timestamp_millis();
    let id = format!("mir_{}_{}_{}", args.event_id, args.market_id, now);
    sqlx::query(
        "INSERT INTO copy_mirror_queue
            (id, event_id, target_id, market_id, side, size, flipped, status, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, 'pending', ?)
         ON CONFLICT(id) DO NOTHING",
    )
    .bind(&id)
    .bind(args.event_id)
    .bind(&args.target_id)
    .bind(&args.market_id)
    .bind(&args.side)
    .bind(&args.size)
    .bind(args.flipped)
    .bind(now)
    .execute(&state.db)
    .await?;
    let row: MirrorRow = sqlx::query_as("SELECT * FROM copy_mirror_queue WHERE id = ?")
        .bind(&id)
        .fetch_one(&state.db)
        .await?;
    Ok(row)
}

#[derive(Debug, Deserialize)]
pub struct ListMirrorsArgs {
    pub status: Option<String>,
    pub limit: Option<i64>,
}

/// List mirrors newest first, optionally filtered by status.
#[tauri::command]
pub async fn list_mirrors(
    state: State<'_, AppState>,
    args: ListMirrorsArgs,
) -> AppResult<Vec<MirrorRow>> {
    let limit = args.limit.unwrap_or(100).clamp(1, 1000);
    let rows: Vec<MirrorRow> = match args.status.as_deref() {
        Some(s) => {
            sqlx::query_as(
                "SELECT * FROM copy_mirror_queue WHERE status = ? ORDER BY created_at DESC LIMIT ?",
            )
            .bind(s)
            .bind(limit)
            .fetch_all(&state.db)
            .await?
        }
        None => {
            sqlx::query_as("SELECT * FROM copy_mirror_queue ORDER BY created_at DESC LIMIT ?")
                .bind(limit)
                .fetch_all(&state.db)
                .await?
        }
    };
    Ok(rows)
}

#[derive(Debug, Deserialize)]
pub struct RunPassArgs {
    pub key_alias: String,
    pub wallet_id: String,
}

/// Run one executor pass: pick pending → submit, reject stale.
#[tauri::command]
pub async fn run_mirror_executor_pass(
    state: State<'_, AppState>,
    args: RunPassArgs,
) -> AppResult<ExecutorPassResult> {
    run_pass_impl(&state, args.key_alias, args.wallet_id).await
}

// =================================================================
// ============== v0.44c — paper mode IPC + paper_fills query ======
// =================================================================

/// v0.44c — args for `set_mirror_paper_mode`. The L1
/// pushes the user's paper-mode pref to Rust on
/// Settings mount and on every toggle. The
/// scheduler reads the current value on every tick.
#[derive(Debug, Clone, Deserialize)]
pub struct SetMirrorPaperModeArgs {
    pub enabled: bool,
}

/// v0.44c — runtime override of paper mode. After
/// this call, the next mirror executor pass writes
/// picked orders to `paper_fills` (paper mode on)
/// or to `bets` (paper mode off). The env-var
/// `POLYROCKET_MIRROR_PAPER_MODE` only matters at
/// process start; after this IPC the user's choice
/// wins.
#[tauri::command]
pub async fn set_mirror_paper_mode(
    state: State<'_, AppState>,
    args: SetMirrorPaperModeArgs,
) -> AppResult<bool> {
    let mut guard = state
        .mirror_paper_mode
        .lock()
        .map_err(|e| AppError::Internal(format!("mirror_paper_mode lock: {e}")))?;
    *guard = args.enabled;
    Ok(args.enabled)
}

/// v0.44c — read the current paper-mode override.
/// The L1 calls this on Settings mount so the
/// toggle reflects what the Rust side currently
/// has (in case the env var set it at startup).
#[tauri::command]
pub async fn get_mirror_paper_mode(
    state: State<'_, AppState>,
) -> AppResult<bool> {
    let guard = state
        .mirror_paper_mode
        .lock()
        .map_err(|e| AppError::Internal(format!("mirror_paper_mode lock: {e}")))?;
    Ok(*guard)
}

/// v0.44c — wire-format mirror of a single
/// `paper_fills` row. Used by `list_paper_fills`
/// below.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PaperFillDto {
    pub id: String,
    pub mirror_id: String,
    pub market_id: String,
    pub side: String,
    pub size: String,
    pub price: f64,
    pub placed_at: i64,
    pub notes: Option<String>,
}

/// v0.44c — args for `list_paper_fills`. Same
/// shape as `list_bets` for consistency.
#[derive(Debug, Deserialize)]
pub struct ListPaperFillsArgs {
    pub limit: Option<i64>,
}

/// v0.44c — query the paper_fills table. The L1
/// uses this to render the [PAPER] badges in Copy
/// / PnL and to show the paper-only PnL summary.
/// Default limit 100, newest first.
#[tauri::command]
pub async fn list_paper_fills(
    state: State<'_, AppState>,
    args: ListPaperFillsArgs,
) -> AppResult<Vec<PaperFillDto>> {
    let limit = args.limit.unwrap_or(100);
    let rows = sqlx::query_as::<_, PaperFillDto>(
        "SELECT id, mirror_id, market_id, side, size, price, placed_at, notes
         FROM paper_fills ORDER BY placed_at DESC LIMIT ?",
    )
    .bind(limit)
    .fetch_all(&state.db)
    .await?;
    Ok(rows)
}

pub async fn run_pass_impl(
    state: &AppState,
    key_alias: String,
    wallet_id: String,
) -> AppResult<ExecutorPassResult> {
    let cfg = ExecutorConfig::from_env();
    let now = chrono::Utc::now().timestamp_millis();

    // 1. Load all pending mirrors
    let rows: Vec<MirrorRow> = sqlx::query_as(
        "SELECT * FROM copy_mirror_queue WHERE status IN ('pending','submitted')",
    )
    .fetch_all(&state.db)
    .await?;
    let orders: Vec<MirrorOrder> = rows.into_iter().map(Into::into).collect();

    // 2. Load market close times
    let market_ids: Vec<String> = orders.iter().map(|o| o.market_id.clone()).collect();
    let market_closes = if market_ids.is_empty() {
        HashMap::new()
    } else {
        let placeholders = market_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let query = format!(
            "SELECT id, end_date FROM markets WHERE id IN ({})",
            placeholders
        );
        let mut q = sqlx::query(&query);
        for id in &market_ids {
            q = q.bind(id);
        }
        let rows = q.fetch_all(&state.db).await?;
        let mut map = HashMap::new();
        for row in rows {
            let id: String = row.try_get("id")?;
            let end: i64 = row.try_get("end_date")?;
            map.insert(id, end);
        }
        map
    };

    // 3. Run pure decision logic
    let result = execute_pass(&orders, &market_closes, now, &cfg)?;

    // 4. Apply rejections
    for (id, reason) in &result.rejected {
        sqlx::query(
            "UPDATE copy_mirror_queue
             SET status = 'rejected', reject_reason = ?
             WHERE id = ? AND status = 'pending'",
        )
        .bind(reason.as_str())
        .bind(id)
        .execute(&state.db)
        .await?;
        // Audit log
        sqlx::query(
            "INSERT INTO audit_log (actor, action, target, payload, result)
             VALUES ('system', 'mirror.reject', ?, ?, 'ok')",
        )
        .bind(id)
        .bind(serde_json::json!({"reason": reason.as_str()}))
        .execute(&state.db)
        .await?;
    }

    // 5. Apply picks — submit as Mode B bets, OR as
    // paper fills if paper_mode is on. The decision
    // logic (what to pick, what to reject) is
    // unchanged; only the write path differs.
    for id in &result.picked {
        let order = orders.iter().find(|o| &o.id == id).cloned();
        let Some(o) = order else { continue };
        // Use sign_order from domain::bet to get a deterministic tx_hash
        let side = match crate::domain::bet::BetSide::parse(&o.side) {
            Ok(s) => s,
            Err(_) => {
                // skip if invalid
                sqlx::query(
                    "UPDATE copy_mirror_queue
                     SET status = 'rejected', reject_reason = 'invalid'
                     WHERE id = ?",
                )
                .bind(&o.id)
                .execute(&state.db)
                .await?;
                continue;
            }
        };

        if cfg.paper_mode {
            // v0.44 — paper mode. Skip signing
            // entirely; just write a paper_fills
            // row and mark the mirror as paper-
            // submitted. The fill is real (size,
            // price, market_id, side) but the user
            // didn't actually trade. The mirror is
            // also marked `paper_submitted` (not
            // `submitted`) so the L1 can filter.
            let paper_id = uuid::Uuid::new_v4().to_string();
            sqlx::query(
                "INSERT INTO paper_fills
                    (id, mirror_id, market_id, side, size, price, placed_at, notes)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(&paper_id)
            .bind(&o.id)
            .bind(&o.market_id)
            .bind(&o.side)
            .bind(&o.size)
            .bind(0.5)
            .bind(now)
            .bind("paper mode — no CLOB submission")
            .execute(&state.db)
            .await?;

            sqlx::query(
                "UPDATE copy_mirror_queue
                 SET status = 'paper_submitted', submitted_at = ?, bet_id = ?
                 WHERE id = ?",
            )
            .bind(now)
            .bind(&paper_id)
            .bind(&o.id)
            .execute(&state.db)
            .await?;

            sqlx::query(
                "INSERT INTO audit_log (actor, action, target, payload, result)
                 VALUES ('system', 'mirror.paper_submit', ?, ?, 'ok')",
            )
            .bind(&o.id)
            .bind(serde_json::json!({
                "paper_id": paper_id,
                "market_id": o.market_id,
                "size": o.size,
                "side": o.side,
                "note": "paper mode — fill not submitted to CLOB",
            }))
            .execute(&state.db)
            .await?;
            continue;
        }

        let place = crate::domain::bet::PlaceArgs {
            market_id: o.market_id.clone(),
            side,
            size_usdc: o.size.clone(),
            price: 0.5, // mirror uses mid for v0.6; v0.7 will resolve best_ask
            key_alias: Some(key_alias.clone()),
        };
        let signed = sign_order(&place, now).map_err(|e| {
            AppError::Internal(format!("sign_order: {e}"))
        })?;

        // Insert the bet row
        let bet_id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO bets
                (id, wallet_id, market_id, signal_id, mode, side, size, price, shares, placed_at, status, tx_hash)
             VALUES (?, ?, ?, NULL, 'B_signed', ?, ?, ?, ?, ?, 'open', ?)",
        )
        .bind(&bet_id)
        .bind(&wallet_id)
        .bind(&o.market_id)
        .bind(&o.side)
        .bind(&o.size)
        .bind(0.5)
        .bind(&signed.shares)
        .bind(signed.signed_at_ms)
        .bind(&signed.tx_hash)
        .execute(&state.db)
        .await?;

        // Mark mirror as submitted + link to bet
        sqlx::query(
            "UPDATE copy_mirror_queue
             SET status = 'submitted', submitted_at = ?, bet_id = ?
             WHERE id = ?",
        )
        .bind(signed.signed_at_ms)
        .bind(&bet_id)
        .bind(&o.id)
        .execute(&state.db)
        .await?;

        // Audit
        sqlx::query(
            "INSERT INTO audit_log (actor, action, target, payload, result)
             VALUES ('system', 'mirror.submit', ?, ?, 'ok')",
        )
        .bind(&o.id)
        .bind(serde_json::json!({
            "bet_id": bet_id,
            "tx_hash": signed.tx_hash,
            "market_id": o.market_id,
            "size": o.size,
            "side": o.side,
        }))
        .execute(&state.db)
        .await?;
    }

    Ok(result)
}

#[derive(Debug, Serialize)]
pub struct MirrorQueueStats {
    pub n_pending: i64,
    pub n_submitted: i64,
    pub n_filled: i64,
    pub n_rejected: i64,
    pub n_expired: i64,
    pub total_exposure_usdc: f64,
    pub headroom_usdc: f64,
}

/// Aggregate stats for the queue + executor headroom.
#[tauri::command]
pub async fn mirror_queue_stats(state: State<'_, AppState>) -> AppResult<MirrorQueueStats> {
    let row = sqlx::query(
        "SELECT status, COUNT(*) as n, COALESCE(SUM(CAST(size AS REAL)), 0.0) as exposure
         FROM copy_mirror_queue
         WHERE status IN ('pending', 'submitted', 'filled', 'rejected', 'expired')
         GROUP BY status",
    )
    .fetch_all(&state.db)
    .await?;
    let mut s = MirrorQueueStats {
        n_pending: 0,
        n_submitted: 0,
        n_filled: 0,
        n_rejected: 0,
        n_expired: 0,
        total_exposure_usdc: 0.0,
        headroom_usdc: 0.0,
    };
    for row in row {
        let status: String = row.try_get("status")?;
        let n: i64 = row.try_get("n")?;
        let exp: f64 = row.try_get("exposure")?;
        match status.as_str() {
            "pending" => s.n_pending = n,
            "submitted" => s.n_submitted = n,
            "filled" => s.n_filled = n,
            "rejected" => s.n_rejected = n,
            "expired" => s.n_expired = n,
            _ => {}
        }
        if status == "pending" || status == "submitted" {
            s.total_exposure_usdc += exp;
        }
    }
    let cfg = ExecutorConfig::from_env();
    s.headroom_usdc = (cfg.max_total_exposure_usdc - s.total_exposure_usdc).max(0.0);
    Ok(s)
}
