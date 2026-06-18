//! L2 — PnL dashboard KPIs (M8).
//!
//! IPC: `dashboard_kpis` — aggregates `bets`, `model_performance`,
//! `signals` tables for the home dashboard tiles. Computation will
//! move into `domain::pnl` per M8 milestone.

use crate::AppResult;
use crate::infra::error::AppError;
use crate::infra::state::AppState;
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Debug, Serialize)]
pub struct DashboardKpis {
    pub total_equity_usdc: String,
    pub open_pnl_usdc: String,
    pub win_rate_30d: f64,
    pub brier_score: f64,
    pub active_signals: i64,
    pub open_positions: i64,
}

/// Aggregated KPIs for the Dashboard page.
/// Computed in Rust for speed — full query stays local.
#[tauri::command]
pub async fn dashboard_kpis(state: State<'_, AppState>) -> AppResult<DashboardKpis> {
    let total_equity: Option<String> =
        sqlx::query_scalar("SELECT COALESCE(SUM(size), '0') FROM bets WHERE status = 'open'")
            .fetch_optional(&state.db)
            .await?;
    let open_pnl: Option<String> =
        sqlx::query_scalar("SELECT COALESCE(SUM(pnl), '0') FROM bets WHERE status = 'open'")
            .fetch_optional(&state.db)
            .await?;

    let wins: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM bets WHERE status = 'won' AND placed_at >= ?")
        .bind((chrono::Utc::now().timestamp_millis() - 30 * 24 * 3600 * 1000))
        .fetch_one(&state.db)
        .await?;
    let losses: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM bets WHERE status = 'lost' AND placed_at >= ?")
        .bind((chrono::Utc::now().timestamp_millis() - 30 * 24 * 3600 * 1000))
        .fetch_one(&state.db)
        .await?;
    let total = (wins + losses).max(1);
    let win_rate = wins as f64 / total as f64;

    let brier: Option<f64> = sqlx::query_scalar(
        "SELECT brier_score FROM model_performance WHERE category IS NULL ORDER BY window_end DESC LIMIT 1",
    )
    .fetch_optional(&state.db)
    .await?;

    let active_signals: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM signals WHERE active = 1")
        .fetch_one(&state.db)
        .await?;
    let open_positions: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM bets WHERE status = 'open'")
        .fetch_one(&state.db)
        .await?;

    Ok(DashboardKpis {
        total_equity_usdc: total_equity.unwrap_or_else(|| "0".into()),
        open_pnl_usdc: open_pnl.unwrap_or_else(|| "0".into()),
        win_rate_30d: win_rate,
        brier_score: brier.unwrap_or(0.0),
        active_signals,
        open_positions,
    })
}

// =================================================================
// ============== v0.45b — paper trading PnL summary ==============
// =================================================================

/// v0.45b — paper trading PnL summary. Aggregates
/// the `paper_fills` table into a single struct the
/// L1 can show on the Dashboard / PnL page as a
/// "what would have happened" stat.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaperPnlSummary {
    /// Total paper fills recorded (settled + unsettled).
    pub total_fills: i64,
    /// Number of paper fills that have been
    /// reconciled against a market resolution.
    pub settled_fills: i64,
    /// Number of those that were wins (side matched
    /// the resolution outcome).
    pub won_fills: i64,
    /// Number that were losses.
    pub lost_fills: i64,
    /// Settled win rate (won / settled). 0.0 when
    /// no fills are settled yet.
    pub win_rate: f64,
    /// Total realized PnL across all settled fills,
    /// in USDC. Positive = gains, negative = losses.
    pub realized_pnl_usdc: String,
    /// Whether the user has paper mode enabled.
    /// The L1 uses this to decide whether to show
    /// the card at all.
    pub paper_mode_enabled: bool,
}

/// v0.45b — paper PnL IPC. Returns the aggregate
/// summary, plus the current paper_mode flag (so
/// the L1 can show / hide the card without a
/// second query).
#[tauri::command]
pub async fn paper_pnl_summary(
    state: State<'_, AppState>,
) -> AppResult<PaperPnlSummary> {
    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM paper_fills")
        .fetch_one(&state.db)
        .await?;
    let settled: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM paper_fills WHERE settled_at IS NOT NULL",
    )
    .fetch_one(&state.db)
    .await?;
    let won: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM paper_fills WHERE won = 1",
    )
    .fetch_one(&state.db)
    .await?;
    let lost = settled - won;
    let win_rate = if settled > 0 {
        won as f64 / settled as f64
    } else {
        0.0
    };
    // Realized PnL — sum of pnl_usdc across settled
    // fills. We compute in SQL to avoid floating
    // point error in the Rust loop.
    let realized_pnl: f64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(CAST(pnl_usdc AS REAL)), 0.0) FROM paper_fills
         WHERE settled_at IS NOT NULL AND pnl_usdc IS NOT NULL",
    )
    .fetch_one(&state.db)
    .await?;
    let paper_mode = *state
        .mirror_paper_mode
        .lock()
        .map_err(|e| AppError::Internal(format!("mirror_paper_mode lock: {e}")))?;
    Ok(PaperPnlSummary {
        total_fills: total,
        settled_fills: settled,
        won_fills: won,
        lost_fills: lost,
        win_rate,
        realized_pnl_usdc: format!("{:.4}", realized_pnl),
        paper_mode_enabled: paper_mode,
    })
}