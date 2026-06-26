//! L2 —— Smart Money 评分（P0-1）。
//!
//! IPC:smart_money_score —— 从 bets + wallets 表聚合某市场的
//! 持仓者数据，将评分计算交给 `domain::smart_money::compute_score`，
//! 并返回计算结果。

use crate::AppResult;
use crate::domain::smart_money::{
    BetInput, SideBreakdown, SmartMoneyScore, TopWallet, WalletInput, compute_score,
};
use crate::infra::state::AppState;
use serde::{Deserialize, Serialize};
use specta::Type;
use sqlx::FromRow;
use tauri::State;

/// 头部贡献者钱包的传输 DTO。
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct TopWalletDto {
    pub address: String,
    pub pnl: f64,
    pub win_rate: f64,
}

impl From<TopWallet> for TopWalletDto {
    fn from(w: TopWallet) -> Self {
        Self {
            address: w.address,
            pnl: w.pnl,
            win_rate: w.win_rate,
        }
    }
}

/// 按方向汇总的传输 DTO。
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SideBreakdownDto {
    pub wallet_count: i64,
    pub avg_pnl: f64,
    pub win_rate: f64,
    pub median_position: f64,
    pub top_wallets: Vec<TopWalletDto>,
}

impl From<SideBreakdown> for SideBreakdownDto {
    fn from(b: SideBreakdown) -> Self {
        Self {
            wallet_count: b.wallet_count,
            avg_pnl: b.avg_pnl,
            win_rate: b.win_rate,
            median_position: b.median_position,
            top_wallets: b.top_wallets.into_iter().map(Into::into).collect(),
        }
    }
}

/// 单个市场 smart-money 评分的传输 DTO。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartMoneyScoreDto {
    pub market_id: String,
    pub yes_score: f64,
    pub no_score: f64,
    pub yes_breakdown: SideBreakdownDto,
    pub no_breakdown: SideBreakdownDto,
    pub computed_at: i64,
}

impl From<SmartMoneyScore> for SmartMoneyScoreDto {
    fn from(s: SmartMoneyScore) -> Self {
        Self {
            market_id: s.market_id,
            yes_score: s.yes_score,
            no_score: s.no_score,
            yes_breakdown: s.yes_breakdown.into(),
            no_breakdown: s.no_breakdown.into(),
            computed_at: s.computed_at,
        }
    }
}

/// 单个市场的原始 bet 行（已与 wallets 表连接以获取地址）。
#[derive(Debug, Clone, FromRow)]
struct BetRow {
    wallet_address: String,
    side: String,
    pnl: f64,
    shares: f64,
}

/// 单个钱包跨其全部 bet 的聚合统计（不限于本市场）。
#[derive(Debug, Clone, FromRow)]
struct WalletStatRow {
    address: String,
    wins: i64,
    losses: i64,
    total_pnl: f64,
    resolved_bets: i64,
    created_at: i64,
}

/// IPC:smart_money_score —— 通过聚合 bets + wallets
/// 中各持仓者的行为，为一个市场计算 0-100 区间的 smart-money 评分。
///
/// 本命令是一个薄适配层：取原始数据、映射为领域层
/// 的 `BetInput` / `WalletInput` 类型，再委托给
/// `domain::smart_money::compute_score`。
#[tauri::command]
pub async fn smart_money_score(
    state: State<'_, AppState>,
    market_id: String,
) -> AppResult<SmartMoneyScoreDto> {
    // 1) 拉取该市场的所有 bet,JOIN wallets 以获取链上地址。
    //    `pnl` 与 `shares` 在 bets 表中为 TEXT 类型,
    //    所以我们将其 CAST 为 REAL。
    let bet_rows: Vec<BetRow> = sqlx::query_as::<_, BetRow>(
        "SELECT w.address AS wallet_address,
                LOWER(b.side) AS side,
                CAST(b.pnl AS REAL) AS pnl,
                CAST(b.shares AS REAL) AS shares
         FROM bets b
         JOIN wallets w ON w.id = b.wallet_id
         WHERE b.market_id = ?",
    )
    .bind(&market_id)
    .fetch_all(&state.db)
    .await?;

    if bet_rows.is_empty() {
        // 无持仓者 → 直接返回零分，而不是把空数据
        // 丢给 compute_score（后者也能跑通，但这样更直观）。
        let empty = SideBreakdown {
            wallet_count: 0,
            avg_pnl: 0.0,
            win_rate: 0.0,
            median_position: 0.0,
            top_wallets: vec![],
        };
        let now = chrono::Utc::now().timestamp_millis();
        return Ok(SmartMoneyScoreDto {
            market_id,
            yes_score: 0.0,
            no_score: 0.0,
            yes_breakdown: empty.clone().into(),
            no_breakdown: empty.into(),
            computed_at: now,
        });
    }

    // 2) 收集在该市场上下注过的去重钱包地址，然后
    //    再拉取它们在全部 bet 上的聚合交易画像。
    let wallet_ids: Vec<String> = {
        let mut v: Vec<String> = bet_rows.iter().map(|b| b.wallet_address.clone()).collect();
        v.sort();
        v.dedup();
        v
    };

    // 构造占位符字符串列表："?, ?, ?"，用于 IN 子句。
    let placeholders: String = (0..wallet_ids.len())
        .map(|_| "?")
        .collect::<Vec<_>>()
        .join(", ");

    let sql = format!(
        "SELECT w.address,
                SUM(CASE WHEN b.status = 'won'  THEN 1 ELSE 0 END) AS wins,
                SUM(CASE WHEN b.status = 'lost' THEN 1 ELSE 0 END) AS losses,
                COALESCE(SUM(CAST(b.pnl AS REAL)), 0.0) AS total_pnl,
                SUM(CASE WHEN b.status IN ('won','lost') THEN 1 ELSE 0 END) AS resolved_bets,
                w.created_at
         FROM wallets w
         JOIN bets b ON b.wallet_id = w.id
         WHERE w.address IN ({placeholders})
         GROUP BY w.address"
    );

    let mut q = sqlx::query_as::<_, WalletStatRow>(&sql);
    for id in &wallet_ids {
        q = q.bind(id);
    }
    let stat_rows: Vec<WalletStatRow> = q.fetch_all(&state.db).await?;

    // 3) 把原始行映射为领域层输入类型。
    let bets_data: Vec<BetInput> = bet_rows
        .iter()
        .map(|b| BetInput {
            wallet_address: b.wallet_address.clone(),
            side: b.side.clone(),
            pnl: b.pnl,
            shares: b.shares,
        })
        .collect();

    let wallets_data: Vec<WalletInput> = stat_rows
        .iter()
        .map(|s| {
            let resolved = (s.wins + s.losses).max(1);
            let win_rate = s.wins as f64 / resolved as f64;
            WalletInput {
                address: s.address.clone(),
                win_rate,
                total_pnl: s.total_pnl,
                resolved_bets: s.resolved_bets,
                created_at: s.created_at,
            }
        })
        .collect();

    // 4) 委托给纯 domain 函数。
    let now = chrono::Utc::now().timestamp_millis();
    let score = compute_score(&market_id, &bets_data, &wallets_data, now);

    Ok(score.into())
}
