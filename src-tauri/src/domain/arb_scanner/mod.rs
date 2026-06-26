//! L3 — 套利扫描器（P1-3）。
//!
//! 检测同平台套利机会：当同一市场上 YES + NO 的合并成本
//! 低于 $1.00 时，同时买入两侧即可锁定无风险利润。
//! 仅含纯函数 —— 数据库访问位于 `commands::arb`。
//!
//! 规范：P1-3 Arbitrage Scanner

use serde::{Deserialize, Serialize};
use specta::Type;

/// 在单个市场上检测到的套利机会。
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ArbOpportunity {
    pub market_id: String,
    pub question: String,
    pub yes_cost: f64,
    pub no_cost: f64,
    pub total_cost: f64,
    pub profit_margin: f64,
    pub category: String,
}

/// 计算 YES+NO 组合的利润率（%）。
///
/// `margin = (1.0 - total_cost) / total_cost * 100.0`（利润率计算公式）
///
/// 总成本 $0.95 的组合可得到 `(0.05 / 0.95) * 100 ≈ 5.26%`。
pub fn compute_profit_margin(yes_cost: f64, no_cost: f64) -> f64 {
    let total = yes_cost + no_cost;
    if total <= 0.0 {
        return 0.0;
    }
    (1.0 - total) / total * 100.0
}

/// 检测一对 YES/NO 价格上的套利机会。
///
/// 当 `yes_cost + no_cost < 1.0`（即同时买入两侧的成本低于 $1.00 赔付）
/// 时返回 `Some(ArbOpportunity)`，否则返回 `None`。`market_id`、
/// `question` 和 `category` 字段使用占位符填充，因为这是一个
/// 纯函数 —— 真实元数据由命令层提供。
pub fn detect_arb(yes_price: f64, no_price: f64) -> Option<ArbOpportunity> {
    let total_cost = yes_price + no_price;
    if total_cost >= 1.0 {
        return None;
    }
    let margin = compute_profit_margin(yes_price, no_price);
    Some(ArbOpportunity {
        market_id: String::new(),
        question: String::new(),
        yes_cost: yes_price,
        no_cost: no_price,
        total_cost,
        profit_margin: margin,
        category: String::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_arb_below_one() {
        let arb = detect_arb(0.45, 0.50).unwrap();
        assert!((arb.total_cost - 0.95).abs() < 1e-9);
        assert!(arb.profit_margin > 0.0);
    }

    #[test]
    fn no_arb_at_or_above_one() {
        assert!(detect_arb(0.50, 0.50).is_none());
        assert!(detect_arb(0.60, 0.50).is_none());
    }

    #[test]
    fn margin_formula() {
        // 0.95 总成本 → (0.05 / 0.95) * 100 ≈ 5.263
        let m = compute_profit_margin(0.45, 0.50);
        assert!((m - 5.2631_578).abs() < 1e-3);
    }

    #[test]
    fn margin_zero_total() {
        assert_eq!(compute_profit_margin(0.0, 0.0), 0.0);
    }
}
