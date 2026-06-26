//! L3 —— 群体智慧（crowd wisdom）：资本加权意见（Phase 1.1）。
//!
//! 将持有者仓位聚合成 **资本加权** 的意见，衡量的不仅是投票 YES/NO 的 *人数*，
//! 还有每一方背后的 *资本量*。这实现了 Logic 1（行为金融）的核心论点：
//! 投入更多资本的 trader 通常拥有更深入的信息和更高的信念。
//!
//! 关键输出：
//!   - `yes_weighted_pct` / `no_weighted_pct` —— 资本加权占比
//!   - `hhi` —— 赫芬达尔-赫希曼指数（Herfindahl-Hirschman Index，集中度，0..1）
//!   - `top3_share` —— 前 3 大钱包占总资本的比例
//!   - `smart_divergence` —— 群体比例减去聪明钱比例
//!
//! 函数是 **纯函数**：所有数据通过 `HolderStance` 切片传入。
//! 数据库访问位于 L2 命令层（`commands::crowd_wisdom`）。

use serde::{Deserialize, Serialize};
use specta::Type;

/// 单个持有者对市场的方向性立场。
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct HolderStance {
    pub wallet: String,
    pub side: String,        // "YES" | "NO"
    pub shares: f64,         // position size in shares
    pub avg_price: f64,      // avg entry price (0..1)
    pub pnl: f64,            // realized + unrealized PnL in USD
    pub win_rate: f64,       // historical win rate (0..1)
}

/// 资本加权的群体意见结果。
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CrowdOpinion {
    pub market_id: String,
    pub yes_capital: f64,         // total $ committed to YES
    pub no_capital: f64,          // total $ committed to NO
    pub yes_weighted_pct: f64,    // yes_capital / total (0..1)
    pub no_weighted_pct: f64,     // no_capital / total (0..1)
    pub hhi: f64,                 // Herfindahl-Hirschman Index (0..1)
    pub top3_share: f64,          // top-3 wallets' share of total capital (0..1)
    pub n_holders: usize,
    pub smart_yes_pct: f64,       // capital-weighted YES % among smart wallets
    pub smart_divergence: f64,    // crowd_yes_pct - smart_yes_pct
    pub computed_at: i64,         // unix-ms
}

/// 胜率阈值，超过该阈值的钱包被视为「聪明钱」。
pub const SMART_WIN_RATE_THRESHOLD: f64 = 0.60;

/// 根据持有者立场计算资本加权的群体意见。
///
/// **资本加权**：每位持有者的投票按 `shares * avg_price`（其美元投入）加权。
/// 这是核心的「资本加权」逻辑 —— 一个持有 10,000 股 @ $0.80（投入 $8,000）
/// 的钱包，其投票权是 1,000 股 @ $0.10（投入 $100）的 80 倍。
///
/// **HHI（赫芬达尔-赫希曼指数）**：所有持有者市场份额平方的总和，范围 [0, 1]：
///   - < 0.1   → 多元化（许多小额持有者）
///   - 0.1-0.25 → 中度集中
///   - > 0.25  → 高度集中（少数巨鲸主导）
///   - > 0.5   → 接近垄断（单一巨鲸）
///
/// **Top-3 占比**：总资本中前 3 大持有者所占的比例。
/// 高 top3_share + 高 HHI = 巨鲸驱动的市场。
///
/// **聪明钱偏离度**：若聪明钱（win_rate ≥ 0.60）70% 押 YES，
/// 但整体群体 50% 押 YES，则偏离度 = +0.20，
/// 暗示散户情绪可能与知情 trader 出现分歧。
pub fn compute_crowd_opinion(
    market_id: &str,
    stances: &[HolderStance],
    computed_at: i64,
) -> CrowdOpinion {
    if stances.is_empty() {
        return CrowdOpinion {
            market_id: market_id.to_string(),
            yes_capital: 0.0,
            no_capital: 0.0,
            yes_weighted_pct: 0.5,
            no_weighted_pct: 0.5,
            hhi: 0.0,
            top3_share: 0.0,
            n_holders: 0,
            smart_yes_pct: 0.5,
            smart_divergence: 0.0,
            computed_at,
        };
    }

    // -- 每方资本
    let mut yes_capital = 0.0_f64;
    let mut no_capital = 0.0_f64;
    let mut capital_by_wallet: Vec<(String, f64, String, f64)> = Vec::new();
    // (钱包, 资本, 方向, 胜率)

    for s in stances {
        let capital = s.shares * s.avg_price;
        let side_upper = s.side.to_uppercase();
        if side_upper == "YES" {
            yes_capital += capital;
        } else {
            no_capital += capital;
        }
        capital_by_wallet.push((s.wallet.clone(), capital, side_upper.clone(), s.win_rate));
    }

    let total_capital = yes_capital + no_capital;
    let yes_pct = if total_capital > 0.0 {
        yes_capital / total_capital
    } else {
        0.5
    };
    let no_pct = 1.0 - yes_pct;

    // -- HHI：市场份额平方之和
    let hhi = if total_capital > 0.0 {
        capital_by_wallet
            .iter()
            .map(|(_, cap, _, _)| {
                let share = cap / total_capital;
                share * share
            })
            .sum()
    } else {
        0.0
    };

    // -- 前 3 大占比
    capital_by_wallet.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    let top3_sum: f64 = capital_by_wallet.iter().take(3).map(|(_, cap, _, _)| *cap).sum();
    let top3_share = if total_capital > 0.0 {
        top3_sum / total_capital
    } else {
        0.0
    };

    // -- 聪明钱资本加权 YES 比例
    let smart_stances: Vec<&(String, f64, String, f64)> = capital_by_wallet
        .iter()
        .filter(|(_, _, _, wr)| *wr >= SMART_WIN_RATE_THRESHOLD)
        .collect();

    let smart_yes_pct = if smart_stances.is_empty() {
        yes_pct // 没有聪明钱时回退到群体比例
    } else {
        let smart_yes_capital: f64 = smart_stances
            .iter()
            .filter(|(_, _, side, _)| side == "YES")
            .map(|(_, cap, _, _)| *cap)
            .sum();
        let smart_total: f64 = smart_stances.iter().map(|(_, cap, _, _)| *cap).sum();
        if smart_total > 0.0 {
            smart_yes_capital / smart_total
        } else {
            0.5
        }
    };

    let smart_divergence = yes_pct - smart_yes_pct;

    CrowdOpinion {
        market_id: market_id.to_string(),
        yes_capital,
        no_capital,
        yes_weighted_pct: yes_pct,
        no_weighted_pct: no_pct,
        hhi,
        top3_share,
        n_holders: stances.len(),
        smart_yes_pct,
        smart_divergence,
        computed_at,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stance(wallet: &str, side: &str, shares: f64, price: f64, wr: f64) -> HolderStance {
        HolderStance {
            wallet: wallet.into(),
            side: side.into(),
            shares,
            avg_price: price,
            pnl: 0.0,
            win_rate: wr,
        }
    }

    #[test]
    fn empty_stances() {
        let r = compute_crowd_opinion("m", &[], 1000);
        assert_eq!(r.n_holders, 0);
        assert!((r.yes_weighted_pct - 0.5).abs() < 1e-9);
    }

    #[test]
    fn capital_weighted_basic() {
        // 钱包 A：10000 股 YES @ $0.80 = $8000
        // 钱包 B：1000 股 NO @ $0.10 = $100
        // 资本加权 YES = 8000/8100 ≈ 98.8%
        let stances = vec![
            stance("A", "YES", 10000.0, 0.80, 0.50),
            stance("B", "NO", 1000.0, 0.10, 0.50),
        ];
        let r = compute_crowd_opinion("m", &stances, 1000);
        assert!((r.yes_weighted_pct - 8000.0 / 8100.0).abs() < 1e-6);
        assert!((r.yes_capital - 8000.0).abs() < 1e-6);
        assert!((r.no_capital - 100.0).abs() < 1e-6);
    }

    #[test]
    fn hhi_diversified() {
        // 10 个均等持有者 → HHI = 10 * (0.1)^2 = 0.1
        let stances: Vec<HolderStance> = (0..10)
            .map(|i| stance(&format!("w{i}"), "YES", 100.0, 0.50, 0.50))
            .collect();
        let r = compute_crowd_opinion("m", &stances, 1000);
        assert!((r.hhi - 0.1).abs() < 1e-6);
    }

    #[test]
    fn hhi_concentrated() {
        // 1 个巨鲸持有 99% 资本 → HHI ≈ 0.98
        let stances = vec![
            stance("whale", "YES", 9900.0, 0.50, 0.50),
            stance("retail1", "YES", 100.0, 0.50, 0.50),
        ];
        let r = compute_crowd_opinion("m", &stances, 1000);
        assert!(r.hhi > 0.95);
        assert!((r.top3_share - 1.0).abs() < 1e-6); // only 2 holders, top3 = all
    }

    #[test]
    fn smart_divergence_positive() {
        // 群体是 50-50，但聪明钱 90% 押 YES
        let stances = vec![
            stance("smart1", "YES", 1000.0, 0.50, 0.80),
            stance("smart2", "YES", 1000.0, 0.50, 0.70),
            stance("retail1", "NO", 1000.0, 0.50, 0.40),
            stance("retail2", "NO", 1000.0, 0.50, 0.30),
        ];
        let r = compute_crowd_opinion("m", &stances, 1000);
        // 群体 YES = 2000/4000 = 0.5
        // 聪明钱 YES = 2000/2000 = 1.0
        // 偏离度 = 0.5 - 1.0 = -0.5
        assert!((r.yes_weighted_pct - 0.5).abs() < 1e-6);
        assert!((r.smart_yes_pct - 1.0).abs() < 1e-6);
        assert!((r.smart_divergence - (-0.5)).abs() < 1e-6);
    }

    #[test]
    fn smart_divergence_zero_when_no_smart() {
        // 所有钱包 win_rate < 0.60 → 聪明钱回退到群体值
        let stances = vec![
            stance("a", "YES", 1000.0, 0.50, 0.40),
            stance("b", "NO", 1000.0, 0.50, 0.30),
        ];
        let r = compute_crowd_opinion("m", &stances, 1000);
        assert!((r.smart_divergence).abs() < 1e-9);
    }

    #[test]
    fn top3_share_calculation() {
        // 5 个持有者资本分别为 5000、3000、1000、500、500
        // 总和 = 10000，前 3 大 = 9000 → 0.9
        let stances = vec![
            stance("a", "YES", 10000.0, 0.50, 0.50), // cap 5000
            stance("b", "YES", 6000.0, 0.50, 0.50),  // cap 3000
            stance("c", "YES", 2000.0, 0.50, 0.50),  // cap 1000
            stance("d", "YES", 1000.0, 0.50, 0.50),  // cap 500
            stance("e", "YES", 1000.0, 0.50, 0.50),  // cap 500
        ];
        let r = compute_crowd_opinion("m", &stances, 1000);
        assert!((r.top3_share - 0.9).abs() < 1e-6);
    }
}
