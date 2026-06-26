//! L2 —— 均值回归检测（Phase 1.2）。
//!
//! IPC:reversion_signal —— 拉取某市场的 price snapshot,
//! 把滚动统计与 fade 信号计算委托给
//! `domain::mean_reversion::detect_reversion`。

use crate::AppResult;
use crate::domain::mean_reversion::{
    DEFAULT_WINDOW, DEFAULT_Z_THRESHOLD, ReversionSignal, detect_reversion,
};
use crate::infra::state::AppState;
use serde::{Deserialize, Serialize};
use specta::Type;
use sqlx::FromRow;
use tauri::State;

/// 均值回归信号的 Wire DTO。
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ReversionSignalDto {
    pub market_id: String,
    pub current: f64,
    pub mean: f64,
    pub std_dev: f64,
    pub z_score: f64,
    pub bollinger_upper: f64,
    pub bollinger_lower: f64,
    pub is_overextended: bool,
    pub direction: String,
    pub window_size: usize,
    pub fade_signal: f64,
    pub confidence: f64,
    pub computed_at: i64,
}

impl From<ReversionSignal> for ReversionSignalDto {
    fn from(r: ReversionSignal) -> Self {
        Self {
            market_id: r.market_id,
            current: r.stats.current,
            mean: r.stats.mean,
            std_dev: r.stats.std_dev,
            z_score: r.stats.z_score,
            bollinger_upper: r.stats.bollinger_upper,
            bollinger_lower: r.stats.bollinger_lower,
            is_overextended: r.stats.is_overextended,
            direction: r.stats.direction,
            window_size: r.stats.window_size,
            fade_signal: r.fade_signal,
            confidence: r.confidence,
            computed_at: r.computed_at,
        }
    }
}

/// 用于回归分析的原始 price snapshot 行。
#[derive(Debug, Clone, FromRow)]
struct PriceRow {
    mid_price: f64,
}

/// IPC:reversion_signal —— 基于市场的 price snapshot 历史计算均值回归
/// 统计与 fade 信号。
///
/// 参数：
///   - `market_id` —— 待分析的市场
///   - `window` —— 滚动窗口大小（默认 20）
///   - `z_threshold` —— 判定过度的 z-score 阈值（默认 2.0）
#[tauri::command]
pub async fn reversion_signal(
    state: State<'_, AppState>,
    market_id: String,
    window: Option<usize>,
    z_threshold: Option<f64>,
) -> AppResult<ReversionSignalDto> {
    let win = window.unwrap_or(DEFAULT_WINDOW);
    let z_thresh = z_threshold.unwrap_or(DEFAULT_Z_THRESHOLD);

    // 按时间升序拉取 price snapshot。
    let rows: Vec<PriceRow> = sqlx::query_as::<_, PriceRow>(
        "SELECT mid_price
         FROM price_snapshots
         WHERE market_id = ?
         ORDER BY captured_at ASC",
    )
    .bind(&market_id)
    .fetch_all(&state.db)
    .await?;

    let prices: Vec<f64> = rows.iter().map(|r| r.mid_price).collect();
    let now = chrono::Utc::now().timestamp_millis();
    let signal = detect_reversion(&market_id, &prices, win, z_thresh, now);

    Ok(signal.into())
}
