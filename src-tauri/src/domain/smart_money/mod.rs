//! L3 — Smart Money Score（聪明钱分数）（P0-1）。
//!
//! 通过聚合投注于某一市场的持币者的链上行为，
//! 为该预测市场的每一侧（YES/NO）计算 0-100 的「smart money」分数。
//! 该分数融合了四种信号：
//!
//!   1. **平均持币者胜率**（40%）—— 该侧下注钱包历史已结算 bet 中获胜的比例。
//!   2. **加权 PnL**（30%）—— 这些钱包的累计已实现 PnL，
//!      经归一化以避免单一巨鲸主导结果。
//!   3. **交易者质量分级**（20%）—— 由胜率与交易量综合派生的 0-1 分级；
//!      资深赢家排名更高。
//!   4. **新钱包比例**（10%）—— 持币者中近期创建钱包的比例。
//!      比例越低代表信任度越高，因此该分量在加权前被反转为 (1 - ratio)。
//!
//! 本函数为**纯函数**：接收预取的 `BetInput` / `WalletInput` 切片，
//! 返回 `SmartMoneyScore`。所有数据库访问位于 L2 命令（`commands::smart_money`）。

use serde::{Deserialize, Serialize};
use specta::Type;

/// 单一持币者在某市场的聚合仓位，由命令层预取。
/// `side` 为 `"yes"` / `"no"`（调用方已转为小写）。
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct BetInput {
    pub wallet_address: String,
    pub side: String,
    pub pnl: f64,
    pub shares: f64,
}

/// 钱包的历史交易画像，由命令层预取。
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct WalletInput {
    pub address: String,
    pub win_rate: f64,        // 0..1
    pub total_pnl: f64,
    pub resolved_bets: i64,
    pub created_at: i64,      // unix-ms;用于派生“新钱包”标记
}

/// 一侧的头部贡献者钱包。
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct TopWallet {
    pub address: String,
    pub pnl: f64,
    pub win_rate: f64,
}

/// 喂给头条分数的按方拆分。
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SideBreakdown {
    pub wallet_count: i64,
    pub avg_pnl: f64,
    pub win_rate: f64,
    pub median_position: f64,
    pub top_wallets: Vec<TopWallet>,
}

/// 单个市场的头条 smart-money 分数。
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SmartMoneyScore {
    pub market_id: String,
    pub yes_score: f64,
    pub no_score: f64,
    pub yes_breakdown: SideBreakdown,
    pub no_breakdown: SideBreakdown,
    pub computed_at: i64,
}

/// 阈值（unix-ms）,低于此值则钱包被视为“新”。
/// 默认为 `computed_at` 前 30 天。
const NEW_WALLET_WINDOW_MS: i64 = 30 * 24 * 3_600_000;

/// 计算单个市场的 smart-money 分数。
///
/// `bets_data` 应只包含目标市场的注单。
/// `wallets_data` 应包含 `bets_data` 引用的所有钱包的画像。
/// `computed_at` 是调用方的“当前时间”（unix-ms）。
pub fn compute_score(
    market_id: &str,
    bets_data: &[BetInput],
    wallets_data: &[WalletInput],
    computed_at: i64,
) -> SmartMoneyScore {
    let yes_bets: Vec<&BetInput> = bets_data.iter().filter(|b| b.side == "yes").collect();
    let no_bets: Vec<&BetInput> = bets_data.iter().filter(|b| b.side == "no").collect();

    let yes_breakdown = build_side_breakdown(&yes_bets, wallets_data, computed_at);
    let no_breakdown = build_side_breakdown(&no_bets, wallets_data, computed_at);

    let yes_score = side_score(&yes_breakdown, &yes_bets, wallets_data, computed_at);
    let no_score = side_score(&no_breakdown, &no_bets, wallets_data, computed_at);

    SmartMoneyScore {
        market_id: market_id.to_string(),
        yes_score,
        no_score,
        yes_breakdown,
        no_breakdown,
        computed_at,
    }
}

fn build_side_breakdown(
    side_bets: &[&BetInput],
    wallets: &[WalletInput],
    _computed_at: i64,
) -> SideBreakdown {
    let wallet_count = side_bets.len() as i64;
    if side_bets.is_empty() {
        return SideBreakdown {
            wallet_count: 0,
            avg_pnl: 0.0,
            win_rate: 0.0,
            median_position: 0.0,
            top_wallets: vec![],
        };
    }

    let pnls: Vec<f64> = side_bets.iter().map(|b| b.pnl).collect();
    let avg_pnl = pnls.iter().sum::<f64>() / pnls.len() as f64;

    let mut shares: Vec<f64> = side_bets.iter().map(|b| b.shares).collect();
    shares.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median_position = shares[shares.len() / 2];

    let win_rates: Vec<f64> = side_bets
        .iter()
        .filter_map(|b| wallets.iter().find(|w| w.address == b.wallet_address))
        .map(|w| w.win_rate)
        .collect();
    let win_rate = if win_rates.is_empty() {
        0.0
    } else {
        win_rates.iter().sum::<f64>() / win_rates.len() as f64
    };

    let mut top: Vec<TopWallet> = side_bets
        .iter()
        .filter_map(|b| {
            wallets.iter().find(|w| w.address == b.wallet_address).map(|w| TopWallet {
                address: w.address.clone(),
                pnl: b.pnl,
                win_rate: w.win_rate,
            })
        })
        .collect();
    top.sort_by(|a, b| b.pnl.partial_cmp(&a.pnl).unwrap_or(std::cmp::Ordering::Equal));
    top.truncate(5);

    SideBreakdown {
        wallet_count,
        avg_pnl,
        win_rate,
        median_position,
        top_wallets: top,
    }
}

/// 将四个分量融合为 0-100 分。
fn side_score(
    breakdown: &SideBreakdown,
    side_bets: &[&BetInput],
    wallets: &[WalletInput],
    computed_at: i64,
) -> f64 {
    if side_bets.is_empty() {
        return 0.0;
    }

    // 1) 平均持币者胜率 (0..1) → 0..100
    let win_rate_component = breakdown.win_rate.clamp(0.0, 1.0) * 100.0;

    // 2) 加权 PnL。使用 tanh 把可能很大的总和限制到 0..1。
    let total_pnl: f64 = side_bets.iter().map(|b| b.pnl).sum();
    let pnl_component = (total_pnl / 100.0).tanh().clamp(0.0, 1.0) * 100.0;

    // 3) 交易者质量分级 (0..1)。结合已结算 bet 的胜率与交易量 ——
    //    胜率高且已结算 bet 多的钱包比仅有几笔 bet 但胜率高的钱包
    //    信号更可靠。
    let tier: f64 = side_bets
        .iter()
        .filter_map(|b| wallets.iter().find(|w| w.address == b.wallet_address))
        .map(|w| {
            let vol_score = (w.resolved_bets as f64 / 50.0).min(1.0);
            (w.win_rate * 0.6 + vol_score * 0.4).clamp(0.0, 1.0)
        })
        .sum::<f64>()
        / side_bets.len() as f64;
    let tier_component = tier * 100.0;

    // 4) 新钱包比例（取反）。近期创建钱包的比例越低 → 信任度越高。
    let new_count = side_bets
        .iter()
        .filter_map(|b| wallets.iter().find(|w| w.address == b.wallet_address))
        .filter(|w| w.created_at > computed_at - NEW_WALLET_WINDOW_MS)
        .count();
    let new_ratio = new_count as f64 / side_bets.len() as f64;
    let trust_component = (1.0 - new_ratio) * 100.0;

    let score = win_rate_component * 0.40
        + pnl_component * 0.30
        + tier_component * 0.20
        + trust_component * 0.10;

    score.clamp(0.0, 100.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wallet(addr: &str, win_rate: f64, pnl: f64, resolved: i64, created_at: i64) -> WalletInput {
        WalletInput {
            address: addr.into(),
            win_rate,
            total_pnl: pnl,
            resolved_bets: resolved,
            created_at,
        }
    }

    fn bet(addr: &str, side: &str, pnl: f64, shares: f64) -> BetInput {
        BetInput {
            wallet_address: addr.into(),
            side: side.into(),
            pnl,
            shares,
        }
    }

    #[test]
    fn empty_market_is_zero() {
        let s = compute_score("m", &[], &[], 1_000_000);
        assert!((s.yes_score - 0.0).abs() < 1e-9);
        assert!((s.no_score - 0.0).abs() < 1e-9);
        assert_eq!(s.yes_breakdown.wallet_count, 0);
    }

    #[test]
    fn high_win_rate_wallets_score_higher() {
        let now = 10_000_000_000;
        let wallets = vec![
            wallet("a", 0.9, 500.0, 100, 0),
            wallet("b", 0.2, -100.0, 5, 0),
        ];
        let bets_good = vec![bet("a", "yes", 50.0, 100.0)];
        let bets_bad = vec![bet("b", "yes", -20.0, 10.0)];
        let s_good = compute_score("m", &bets_good, &wallets, now);
        let s_bad = compute_score("m", &bets_bad, &wallets, now);
        assert!(s_good.yes_score > s_bad.yes_score);
    }

    #[test]
    fn score_is_bounded_0_100() {
        let now = 10_000_000_000;
        let wallets = vec![wallet("a", 1.0, 1_000_000.0, 1_000, 0)];
        let bets = vec![bet("a", "yes", 1_000_000.0, 1_000.0)];
        let s = compute_score("m", &bets, &wallets, now);
        assert!(s.yes_score <= 100.0);
        assert!(s.yes_score >= 0.0);
    }

    #[test]
    fn new_wallets_lower_trust() {
        let now = 10_000_000_000;
        let wallets_old = vec![wallet("a", 0.7, 100.0, 30, 0)];
        let wallets_new = vec![wallet("a", 0.7, 100.0, 30, now - 1_000)];
        let bets = vec![bet("a", "yes", 10.0, 10.0)];
        let s_old = compute_score("m", &bets, &wallets_old, now);
        let s_new = compute_score("m", &bets, &wallets_new, now);
        assert!(s_old.yes_score > s_new.yes_score);
    }

    #[test]
    fn top_wallets_sorted_by_pnl_desc() {
        let now = 10_000_000_000;
        let wallets = vec![
            wallet("a", 0.5, 10.0, 10, 0),
            wallet("b", 0.6, 200.0, 10, 0),
            wallet("c", 0.4, 50.0, 10, 0),
        ];
        let bets = vec![
            bet("a", "yes", 10.0, 5.0),
            bet("b", "yes", 200.0, 20.0),
            bet("c", "yes", 50.0, 8.0),
        ];
        let s = compute_score("m", &bets, &wallets, now);
        assert_eq!(s.yes_breakdown.top_wallets[0].address, "b");
        assert_eq!(s.yes_breakdown.top_wallets[1].address, "c");
        assert_eq!(s.yes_breakdown.top_wallets.len(), 3);
    }
}
