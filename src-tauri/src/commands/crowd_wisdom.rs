//! L2 —— 群众智慧：资金加权观点（Phase 1.1）。
//!
//! IPC:crowd_opinion —— 聚合某市场的持币者仓位
//! 数据（来源 `bets` + `wallets` 表）,
//! 把资金加权计算委托给
//! `domain::crowd_wisdom::compute_crowd_opinion`。

use crate::AppResult;
use crate::domain::crowd_wisdom::{CrowdOpinion, HolderStance, compute_crowd_opinion};
use crate::infra::state::AppState;
use serde::{Deserialize, Serialize};
use specta::Type;
use sqlx::FromRow;
use tauri::State;

/// 资金加权群众观点的 wire DTO。
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CrowdOpinionDto {
    pub market_id: String,
    pub yes_capital: f64,
    pub no_capital: f64,
    pub yes_weighted_pct: f64,
    pub no_weighted_pct: f64,
    pub hhi: f64,
    pub top3_share: f64,
    pub n_holders: usize,
    pub smart_yes_pct: f64,
    pub smart_divergence: f64,
    pub computed_at: i64,
}

impl From<CrowdOpinion> for CrowdOpinionDto {
    fn from(c: CrowdOpinion) -> Self {
        Self {
            market_id: c.market_id,
            yes_capital: c.yes_capital,
            no_capital: c.no_capital,
            yes_weighted_pct: c.yes_weighted_pct,
            no_weighted_pct: c.no_weighted_pct,
            hhi: c.hhi,
            top3_share: c.top3_share,
            n_holders: c.n_holders,
            smart_yes_pct: c.smart_yes_pct,
            smart_divergence: c.smart_divergence,
            computed_at: c.computed_at,
        }
    }
}

/// 关联了钱包地址与统计信息的原始投注行。
#[derive(Debug, Clone, FromRow)]
struct HolderRow {
    wallet_address: String,
    side: String,
    shares: f64,
    avg_price: f64,
    pnl: f64,
    win_rate: f64,
}

/// IPC:`crowd_opinion` —— 通过聚合 `bets` + `wallets` 中的
/// 持币者仓位,计算某市场的资金加权观点。
///
/// 每个持币者的投票按其美元敞口
/// （`shares * avg_price`）加权,
/// 即实现「资本加权」逻辑。
#[tauri::command]
pub async fn crowd_opinion(
    state: State<'_, AppState>,
    market_id: String,
) -> AppResult<CrowdOpinionDto> {
    // 拉取该市场所有投注及其钱包统计。
    let rows: Vec<HolderRow> = sqlx::query_as::<_, HolderRow>(
        "SELECT w.address AS wallet_address,
                UPPER(b.side) AS side,
                CAST(b.shares AS REAL) AS shares,
                CAST(b.avg_price AS REAL) AS avg_price,
                CAST(b.pnl AS REAL) AS pnl,
                CASE
                    WHEN (w.wins + w.losses) > 0
                    THEN CAST(w.wins AS REAL) / CAST(w.wins + w.losses AS REAL)
                    ELSE 0.5
                END AS win_rate
         FROM bets b
         JOIN wallets w ON w.id = b.wallet_id
         WHERE b.market_id = ?",
    )
    .bind(&market_id)
    .fetch_all(&state.db)
    .await?;

    let stances: Vec<HolderStance> = rows
        .iter()
        .map(|r| HolderStance {
            wallet: r.wallet_address.clone(),
            side: r.side.clone(),
            shares: r.shares,
            avg_price: r.avg_price,
            pnl: r.pnl,
            win_rate: r.win_rate,
        })
        .collect();

    let now = chrono::Utc::now().timestamp_millis();
    let opinion = compute_crowd_opinion(&market_id, &stances, now);
    Ok(opinion.into())
}
