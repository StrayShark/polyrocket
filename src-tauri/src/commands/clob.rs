//! L2 — CLOB (Central Limit Order Book) IPC commands (v0.51a).
//!
//! Three IPCs today:
//! - `clob_feed_status` — return whether a real CLOB
//!   feed is configured and connected. Without
//!   `POLYROCKET_CLOB_API_KEY` + `POLYROCKET_CLOB_API_SECRET`
//!   + `POLYROCKET_CLOB_API_PASSPHRASE` in the env, the
//!   feed is "not_configured" and the L1 should fall
//!   back to `price_snapshots`.
//! - `record_clob_snapshot_now` — manual one-shot
//!   insert. Useful for tests and for "paste a snapshot
//!   from the UI" workflows.
//! - `latest_clob_snapshot` — return the most recent
//!   snapshot for a market.
//!
//! The real CLOB WebSocket listener is v0.51+ (still
//! pending). The schema and IPCs are in place so the
//! moment a feed is wired up, everything else falls
//! into place.

use crate::AppResult;
use crate::infra::db::clob_snapshots::{
    self, ClobSnapshot,
};
use crate::infra::state::AppState;
use serde::Serialize;
use tauri::State;

#[derive(Debug, Clone, Serialize)]
pub struct ClobFeedStatus {
    /// One of:
    ///   "not_configured" — env vars missing; the L1
    ///     should use price_snapshots instead
    ///   "configured"     — credentials present, but no
    ///     feed task is running yet (v0.51+)
    ///   "connected"      — feed is actively streaming
    ///     (v0.51+ once the WebSocket listener lands)
    pub state: &'static str,
    /// The current snapshot count for any market
    /// (sum across markets). 0 when no snapshots yet.
    pub total_snapshots: i64,
    /// Number of distinct markets with at least one
    /// snapshot.
    pub markets_with_snapshots: i64,
}

/// v0.51a — return the current CLOB feed status.
/// Without env credentials, returns `not_configured`.
/// The L1 uses this to decide whether to render
/// "live" indicators or fall back to v0.47a's
/// `price_snapshots`.
///
/// v0.119 — accepts either naming convention:
///   - `POLYROCKET_CLOB_API_KEY / _SECRET / _PASSPHRASE` (legacy)
///   - `POLYMARKET_API_KEY / POLYMARKET_API_SECRET /
///     POLYMARKET_API_PASSPHRASE` (standard Polymarket convention)
///
/// `POLYMARKET_*` wins when both are set (more specific).
#[tauri::command]
pub async fn clob_feed_status(state: State<'_, AppState>) -> AppResult<ClobFeedStatus> {
    // v0.119 — accept either naming convention. `POLYMARKET_*` takes
    // precedence if both are set.
    let api_key = std::env::var("POLYMARKET_API_KEY")
        .ok()
        .filter(|v| !v.is_empty())
        .or_else(|| std::env::var("POLYROCKET_CLOB_API_KEY").ok().filter(|v| !v.is_empty()));
    let api_secret = std::env::var("POLYMARKET_API_SECRET")
        .ok()
        .filter(|v| !v.is_empty())
        .or_else(|| std::env::var("POLYROCKET_CLOB_API_SECRET").ok().filter(|v| !v.is_empty()));
    let passphrase = std::env::var("POLYMARKET_API_PASSPHRASE")
        .ok()
        .filter(|v| !v.is_empty())
        .or_else(|| std::env::var("POLYROCKET_CLOB_API_PASSPHRASE").ok().filter(|v| !v.is_empty()));

    let state_str = match (api_key, api_secret, passphrase) {
        (Some(k), Some(s), Some(p))
            if !k.is_empty() && !s.is_empty() && !p.is_empty() =>
        {
            // v0.51+ — once the WebSocket listener lands,
            // this becomes "connected" / "reconnecting".
            // For now, even with creds, we're "configured".
            "configured"
        }
        _ => "not_configured",
    };
    // Aggregate counts.
    let total: i64 = sqlx::query_scalar(
        "SELECT COUNT(DISTINCT (market_id, captured_at)) FROM clob_snapshots",
    )
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);
    let markets: i64 = sqlx::query_scalar(
        "SELECT COUNT(DISTINCT market_id) FROM clob_snapshots",
    )
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);
    Ok(ClobFeedStatus {
        state: state_str,
        total_snapshots: total,
        markets_with_snapshots: markets,
    })
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct RecordClobSnapshotArgs {
    pub market_id: String,
    pub captured_at: i64,
    pub bids: Vec<(f64, f64)>,
    pub asks: Vec<(f64, f64)>,
}

/// v0.51a — manual insert. Records one snapshot
/// (a batch of bids + asks at the same timestamp)
/// and returns the row count. Used by tests and by
/// the L1's "paste a snapshot" workflow.
#[tauri::command]
pub async fn record_clob_snapshot_now(
    state: State<'_, AppState>,
    args: RecordClobSnapshotArgs,
) -> AppResult<usize> {
    clob_snapshots::record_clob_snapshot(
        &state.db,
        &args.market_id,
        args.captured_at,
        &args.bids,
        &args.asks,
    )
    .await
    .map_err(|e| crate::AppError::Internal(format!("record_clob_snapshot: {e}")))?;
    Ok(args.bids.len() + args.asks.len())
}

/// v0.51a — return the most recent snapshot for a
/// market. Returns None when the market has no
/// snapshots. The L1 uses this to render the order
/// book ladder.
#[tauri::command]
pub async fn latest_clob_snapshot(
    state: State<'_, AppState>,
    market_id: String,
) -> AppResult<Option<ClobSnapshot>> {
    clob_snapshots::latest_clob_snapshot(&state.db, &market_id)
        .await
        .map_err(|e| crate::AppError::Internal(format!("latest_clob_snapshot: {e}")))
}
