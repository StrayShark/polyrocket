//! L2 —— 套利扫描命令（P1-3）。
//!
//! IPC:`arb_scan`(扫描足球市场中的套利机会)
//! 和 `list_arb_opportunities`(从 `arb_opportunities` 表中
//! 获取缓存的套利行)。

use crate::AppResult;
use crate::domain::arb_scanner::{detect_arb, ArbOpportunity};
use crate::infra::state::AppState;
use serde::{Deserialize, Serialize};
use specta::Type;
use sqlx::FromRow;
use tauri::State;

/// 套利机会的 DTO —— 镜像 `arb_opportunities` 表。
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, Type)]
pub struct ArbOpportunityDto {
    pub market_id: String,
    pub question: String,
    pub yes_cost: f64,
    pub no_cost: f64,
    pub total_cost: f64,
    pub profit_margin: f64,
    pub category: String,
}

impl From<ArbOpportunity> for ArbOpportunityDto {
    fn from(a: ArbOpportunity) -> Self {
        ArbOpportunityDto {
            market_id: a.market_id,
            question: a.question,
            yes_cost: a.yes_cost,
            no_cost: a.no_cost,
            total_cost: a.total_cost,
            profit_margin: a.profit_margin,
            category: a.category,
        }
    }
}

/// 用于读取市场价格价的中间行。
#[derive(sqlx::FromRow)]
struct MarketPriceRow {
    id: String,
    question: String,
    category: String,
    yes_price: Option<f64>,
    no_price: Option<f64>,
}

/// IPC:`arb_scan` —— 扫描活跃足球市场中的套利机会。
///
/// 查询 `category = 'football'` 且同时存在 YES 与 NO 价的市场,
/// 对每条运行 `detect_arb`,并返回 `yes_cost + no_cost < 1.0` 的结果。
#[tauri::command]
pub async fn arb_scan(state: State<'_, AppState>) -> AppResult<Vec<ArbOpportunityDto>> {
    let rows: Vec<MarketPriceRow> = sqlx::query_as(
        "SELECT id, question, category, yes_price, no_price
         FROM markets
         WHERE active = 1 AND resolved = 0
           AND category = 'football'
           AND yes_price IS NOT NULL AND no_price IS NOT NULL",
    )
    .fetch_all(&state.db)
    .await?;

    let mut opportunities = Vec::new();
    for row in rows {
        let yes = row.yes_price.unwrap_or(0.0);
        let no = row.no_price.unwrap_or(0.0);
        if let Some(mut arb) = detect_arb(yes, no) {
            arb.market_id = row.id;
            arb.question = row.question;
            arb.category = row.category;
            opportunities.push(ArbOpportunityDto::from(arb));
        }
    }

    // 审计日志 —— 记录一次套利扫描的执行。
    let found = opportunities.len();
    sqlx::query(
        "INSERT INTO audit_log (actor, action, target, payload, result)
         VALUES ('user', 'arb.scan', 'markets', ?, 'ok')",
    )
    .bind(serde_json::json!({ "opportunities_found": found }))
    .execute(&state.db)
    .await?;

    Ok(opportunities)
}

/// IPC:`list_arb_opportunities` —— 从数据库获取缓存的套利行。
#[tauri::command]
pub async fn list_arb_opportunities(
    state: State<'_, AppState>,
    limit: Option<i64>,
) -> AppResult<Vec<ArbOpportunityDto>> {
    let limit = limit.unwrap_or(50);
    let rows = sqlx::query_as::<_, ArbOpportunityDto>(
        "SELECT market_id, question, yes_cost, no_cost, total_cost, profit_margin,
                'polymarket' AS category
         FROM arb_opportunities
         ORDER BY detected_at DESC
         LIMIT ?",
    )
    .bind(limit)
    .fetch_all(&state.db)
    .await?;
    Ok(rows)
}
