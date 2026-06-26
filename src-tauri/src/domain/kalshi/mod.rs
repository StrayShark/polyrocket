//! L3 —— Kalshi 跨平台套利（P1-4）。
//!
//! 将 Polymarket 市场与 Kalshi 上同一真实事件的 markets 进行匹配，
//! 当价差足够大时计算跨平台套利机会。仅含纯函数 —— 数据库 / API 访问
//! 由命令层（`commands::cross_platform_arb`）负责。
//!
//! 规范：P1-4 Kalshi Cross-Platform

use serde::{Deserialize, Serialize};
use specta::Type;

/// Kalshi 市场的一行 —— 镜像 Kalshi API 对某个 market ticker 的返回。
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct KalshiMarket {
    pub event_ticker: String,
    pub market_ticker: String,
    pub question: String,
    pub yes_price: Option<f64>,
    pub no_price: Option<f64>,
    pub volume: Option<f64>,
    pub close_date: Option<i64>,
}

/// Polymarket 与 Kalshi 之间的跨平台套利机会。
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CrossPlatformArb {
    pub match_name: String,
    pub market_question: String,
    pub pm_price: f64,
    pub kalshi_price: f64,
    pub spread: f64,
    pub direction: String,
    pub est_profit_per_1000: f64,
}

/// 将 Polymarket 问题与 Kalshi 问题进行模糊匹配。
///
/// 将两个字符串分词（转小写并去除停用词），当它们共享至少 2 个有效词，
/// 或在某一方词数极少时共享 1 个词，则返回 `true`。这是有意宽松的匹配 ——
/// 命令层可以进一步精细化匹配。
pub fn match_markets(pm_question: &str, kalshi_question: &str) -> bool {
    let pm = tokenize(pm_question);
    let kalshi = tokenize(kalshi_question);

    if pm.is_empty() || kalshi.is_empty() {
        return false;
    }

    let shared = pm.iter().filter(|w| kalshi.contains(w)).count();
    let min_needed = if pm.len() <= 2 || kalshi.len() <= 2 { 1 } else { 2 };
    shared >= min_needed
}

/// 计算跨平台套利机会。
///
/// 给定匹配事件的 Polymarket YES 价格和 Kalshi YES 价格，
/// 当价差 `>= 3%`（0.03）时存在套利机会：在一个平台买入便宜方，
/// 在另一平台卖出对立结果。低于阈值时返回 `None`。
///
/// `est_profit_per_1000` 是按 $1,000 名义本金计算的美元利润，
/// 即 `spread * 1000`。
pub fn compute_cross_arb(pm_price: f64, kalshi_price: f64) -> Option<CrossPlatformArb> {
    let spread = (pm_price - kalshi_price).abs();
    if spread < 0.03 {
        return None;
    }
    let direction = if kalshi_price < pm_price {
        "buy_kalshi_sell_pm"
    } else {
        "buy_pm_sell_kalshi"
    };
    Some(CrossPlatformArb {
        match_name: String::new(),
        market_question: String::new(),
        pm_price,
        kalshi_price,
        spread,
        direction: direction.into(),
        est_profit_per_1000: spread * 1000.0,
    })
}

/// 将一个问题分词为小写的有效词。
fn tokenize(s: &str) -> Vec<String> {
    const STOP: &[&str] = &[
        "the", "a", "an", "and", "or", "but", "in", "on", "at", "to", "for",
        "of", "is", "are", "will", "be", "has", "have", "had", "this",
        "that", "with", "from", "by", "as", "it", "its", "was", "were",
        "can", "could", "would", "should", "do", "does", "did", "not", "no",
        "yes", "if", "then", "than", "so", "about", "into", "over", "under",
    ];
    s.to_lowercase()
        .split_whitespace()
        .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).to_string())
        .filter(|w| !w.is_empty() && w.len() > 1 && !STOP.contains(&w.as_str()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn match_same_event() {
        assert!(match_markets(
            "Will Liverpool win the Premier League?",
            "Liverpool Premier League winner"
        ));
    }

    #[test]
    fn no_match_different_event() {
        assert!(!match_markets(
            "Will it rain in London?",
            "Liverpool Premier League winner"
        ));
    }

    #[test]
    fn arb_above_threshold() {
        let arb = compute_cross_arb(0.60, 0.50).unwrap();
        assert!((arb.spread - 0.10).abs() < 1e-9);
        assert_eq!(arb.direction, "buy_kalshi_sell_pm");
        assert!((arb.est_profit_per_1000 - 100.0).abs() < 1e-9);
    }

    #[test]
    fn no_arb_below_threshold() {
        assert!(compute_cross_arb(0.50, 0.52).is_none());
    }

    #[test]
    fn arb_direction_buy_pm() {
        let arb = compute_cross_arb(0.40, 0.55).unwrap();
        assert_eq!(arb.direction, "buy_pm_sell_kalshi");
    }
}
