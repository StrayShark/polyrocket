use crate::AppResult;
use crate::infra::state::AppState;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use tauri::State;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct SignalDto {
    pub id: i64,
    pub market_id: String,
    pub computed_at: i64,
    pub model_version: String,
    pub predicted_prob: f64,
    pub market_prob: f64,
    pub edge: f64,
    pub confidence: f64,
    pub horizon_hours: i64,
    pub rationale: Option<String>,
    pub market_question: Option<String>,
    pub market_slug: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ListSignalsArgs {
    pub min_edge: Option<f64>,
    pub category: Option<String>,
    pub limit: Option<i64>,
}

#[tauri::command]
pub async fn list_active_signals(
    state: State<'_, AppState>,
    args: ListSignalsArgs,
) -> AppResult<Vec<SignalDto>> {
    let min_edge = args.min_edge.unwrap_or(0.05);
    let limit = args.limit.unwrap_or(50);

    let rows = sqlx::query_as::<_, SignalDto>(
        "SELECT s.id, s.market_id, s.computed_at, s.model_version, s.predicted_prob, s.market_prob, s.edge, s.confidence, s.horizon_hours, s.rationale,
                m.question AS market_question, m.slug AS market_slug
         FROM signals s
         JOIN markets m ON m.id = s.market_id
         WHERE s.active = 1 AND ABS(s.edge) >= ?
         ORDER BY ABS(s.edge) DESC
         LIMIT ?",
    )
    .bind(min_edge)
    .bind(limit)
    .fetch_all(&state.db)
    .await?;

    Ok(rows)
}

/// Trigger a recompute of all active signals.
/// Real impl: invoke Python sidecar (model lab) or run embedded model.
#[tauri::command]
pub async fn recompute_signals(_state: State<'_, AppState>) -> AppResult<usize> {
    // TODO: invoke model-lab sidecar (Phase 2 milestone)
    Ok(0)
}