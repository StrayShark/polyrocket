//! L2 — PnL dashboard KPIs (M8).
//!
//! IPC: `dashboard_kpis` — aggregates `bets`, `model_performance`,
//! `signals` tables for the home dashboard tiles. Computation will
//! move into `domain::pnl` per M8 milestone.

use crate::AppResult;
use crate::infra::state::AppState;
use serde::Serialize;
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