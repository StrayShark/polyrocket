//! L2 —— 跨平台套利命令（P1-4）。
//!
//! IPC:`cross_platform_arb_scan` —— 扫描 Polymarket 与 Kalshi
//! 之间的套利机会。在 Kalshi API 集成完全接通之前,
//! 本接口返回 `arb_opportunities` 表中缓存的行
//! （过滤 `platform != 'polymarket'`）,若无缓存则返回空列表。

use crate::AppResult;
use crate::domain::kalshi::CrossPlatformArb;
use crate::infra::state::AppState;
use serde::{Deserialize, Serialize};
use specta::Type;
use sqlx::FromRow;
use tauri::State;

/// 跨平台套利机会的 DTO —— 镜像 `CrossPlatformArb`。
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, Type)]
pub struct CrossPlatformArbDto {
    pub match_name: String,
    pub market_question: String,
    pub pm_price: f64,
    pub kalshi_price: f64,
    pub spread: f64,
    pub direction: String,
    pub est_profit_per_1000: f64,
}

impl From<CrossPlatformArb> for CrossPlatformArbDto {
    fn from(a: CrossPlatformArb) -> Self {
        CrossPlatformArbDto {
            match_name: a.match_name,
            market_question: a.market_question,
            pm_price: a.pm_price,
            kalshi_price: a.kalshi_price,
            spread: a.spread,
            direction: a.direction,
            est_profit_per_1000: a.est_profit_per_1000,
        }
    }
}

/// IPC:`cross_platform_arb_scan` —— 扫描 Polymarket ↔ Kalshi 套利。
///
/// Kalshi API 集成尚未完全接通,因此本接口从
/// `arb_opportunities` 中返回平台 != 'polymarket' 的
/// 缓存跨平台行;无缓存时返回空列表。
#[tauri::command]
pub async fn cross_platform_arb_scan(
    state: State<'_, AppState>,
) -> AppResult<Vec<CrossPlatformArbDto>> {
    let rows: Vec<CrossPlatformArbDto> = sqlx::query_as(
        "SELECT '' AS match_name, question AS market_question,
                yes_cost AS pm_price, no_cost AS kalshi_price,
                (1.0 - total_cost) AS spread,
                'buy_kalshi_sell_pm' AS direction,
                profit_margin * 10.0 AS est_profit_per_1000
         FROM arb_opportunities
         WHERE platform != 'polymarket'
         ORDER BY detected_at DESC
         LIMIT 100",
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    Ok(rows)
}
