//! L2 — Active signals (M5).
//!
//! IPCs: `list_active_signals` (filter by min edge + category),
//! `recompute_signals` (stub — wires to M5 model-lab sidecar later).

use crate::AppResult;
use crate::infra::state::AppState;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use tauri::State;

/// Signal 数据传输对象。JOIN 了 `markets` 表的 `question` / `slug` 字段，
/// 让 L1 「Signals」页不用再二次查 market。
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

/// IPC: `list_active_signals` —— 拉所有 active signals（按 |edge| 降序）。
///
/// **`min_edge` 默认 0.05**：过滤 5% 以下的低确信度信号。
/// **`limit` 默认 50**：L1 dashboard 一次渲染 ≤ 50 行。
/// **`category` 当前未过滤**：UI 留 filter 但 SQL 不带（v0.62+ 加）。
///
/// **`active = 1` 含义**：信号没被 `domain::signal::expire_for_closed_markets`
/// 标记为 expired。M2 每日重算会刷新。
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

/// IPC: `recompute_signals` —— 手动触发 signal 重算。**当前是 stub**。
///
/// **未来实现**：调 `sidecar.signal_recompute` 方法（v0.51a+）让 Python 端
/// 重跑 model；或者内嵌 model 直接调。
///
/// **返回 0**：stub 状态。
#[tauri::command]
pub async fn recompute_signals(_state: State<'_, AppState>) -> AppResult<usize> {
    // TODO: invoke model-lab sidecar (Phase 2 milestone)
    Ok(0)
}