//! L3 — 资金分配（M11, v0.78）。
//!
//! **本模块职责**：将一组 `Signal`（含 `edge`、`confidence`、`market_prob`）
//! 与 `BankrollConfig` 转换为 `Vec<AllocationItem>`（每市场 $ 金额）
//! 的纯函数算法。
//!
//! **本模块非职责**：数据库写入、IPC、UI。这些分别属于
//! L2（commands）和 L1（React）。本模块是**纯函数 + 纯数据**，
//! 不依赖 `AppState`、SQL 或 `tokio::spawn`。
//!
//! **算法**：分数凯利准则 + 多重约束上限。
//! 完整设计参见 `docs/bankroll-allocation-design.md` §1。
//!
//! 规范：docs/polyrocket-modules.md M11。

use crate::domain::signal::Signal;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::HashMap;

/// 每个钱包的资金配置。存储在 `bankroll_config` 表中。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BankrollConfig {
    /// 所使用的完整凯利比例。0.25 = 四分之一凯利（默认）。
    /// 范围：(0, 1]。允许 >1.0，但不推荐。
    pub kelly_multiplier: f64,
    /// 单信号分配占资金比例的硬上限。
    /// 范围：(0, 1]。默认 0.10 = 10%。
    pub max_per_signal_pct: f64,
    /// 从不分配的保留比例。默认 0.20 = 20%。
    /// 范围：[0, 1)。
    pub reserve_pct: f64,
    /// 考虑的最小 |edge|。默认 0.05 = 5%。
    /// 低于此值的信号在第 1 步过滤时被丢弃。
    pub min_edge_pct: f64,
    /// 总敞口占资金比例的最大值。默认 0.80。
    /// 范围：(0, 1]。
    pub max_total_exposure_pct: f64,
    /// 考虑的最小 confidence。默认 0.6。
    /// 范围：[0, 1]。
    pub min_confidence: f64,
}

impl Default for BankrollConfig {
    fn default() -> Self {
        Self {
            kelly_multiplier: 0.25,
            max_per_signal_pct: 0.10,
            reserve_pct: 0.20,
            min_edge_pct: 0.05,
            max_total_exposure_pct: 0.80,
            min_confidence: 0.60,
        }
    }
}

/// 投注方向（预测市场中的 Yes/No 代币）。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Type)]
pub enum BetSide {
    Yes,
    No,
}

/// 单个市场的一项分配。同一市场的多个信号会被合并为单个
/// `AllocationItem`（`source_signal_ids` 列出合并的信号）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Type)]
pub struct AllocationItem {
    pub market_id: String,
    pub side: BetSide,
    /// 最终 USDC 金额，保留两位小数（使用字符串以避免
    /// 浮点精度问题 —— 数据库列为 TEXT）。
    pub size_usdc: String,
    /// 应用任何上限之前的原始凯利比例。范围：[0, 1]。
    pub kelly_pct: f64,
    /// 若分配被上限限制，记录原因（若有）。
    pub capped_reason: Option<CappedReason>,
    /// 合并到本分配的原始信号 ID 列表。
    pub source_signal_ids: Vec<String>,
    /// 模型版本（合并信号中最近的版本）。
    pub model_version: String,
    /// 置信度（合并信号中的最大值）。
    pub confidence: f64,
    /// 预期 ROI = edge × confidence。仅用于展示。
    pub expected_roi: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Type)]
pub enum CappedReason {
    /// 分配受 `max_per_signal_pct` 限制。
    PerSignalCap,
    /// 分配受市场流动性限制。
    Liquidity,
    /// 总敞口 > max_total_exposure_pct，按比例缩减。
    TotalExposure,
}

/// 最终分配结果。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Type)]
pub struct AllocationResult {
    /// 所有市场分配的总 USDC（size_usdc 之和）。
    pub total_allocated_usdc: String,
    /// 保留未分配的 USDC。
    pub reserved_usdc: String,
    /// 每个市场的分配。
    pub per_market: Vec<AllocationItem>,
    /// 因流动性问题被丢弃的市场（凯利 > 0 但无法容纳）。
    pub dropped_markets: Vec<String>,
}

/// `compute_allocation` 的输入。
#[derive(Debug, Clone)]
pub struct AllocationInput<'a> {
    /// 可用 USDC 总额（字符串以匹配数据库 TEXT 列）。
    /// 必须能解析为 f64 > 0。若为 0 则返回空结果。
    pub bankroll_usdc: &'a str,
    /// 每个钱包的配置。
    pub config: &'a BankrollConfig,
    /// 考虑的信号。同市场的信号会被合并。
    pub signals: &'a [Signal],
    /// 可选：market_id → 该市场最大可分配 USDC（流动性上限）。
    pub market_liquidity: Option<&'a HashMap<String, String>>,
}

/// 将 USDC 字符串解析为 f64。解析错误或非有限值时返回 None。
fn parse_usdc(s: &str) -> Option<f64> {
    let v: f64 = s.parse().ok()?;
    if !v.is_finite() || v < 0.0 {
        return None;
    }
    Some(v)
}

/// 将 f64 格式化为保留两位小数的 USDC 字符串（银行家舍入）。
fn fmt_usdc(v: f64) -> String {
    // 保留两位小数。使用 `* 100.0` + `round` / `100.0` 以
    // 达到分精度。`f64::round` 在 IEEE 754 中是银行家舍入。
    let cents = (v * 100.0).round();
    format!("{:.2}", cents / 100.0)
}

/// 计算单个信号的凯利比例。
///
/// 在以下情况下返回 None：
/// - market_prob 为 0 或 1（无法计算赔率）
/// - predicted_prob 无效（< 0；> 1.5 时被裁剪，≤ 0 时返回 None）
/// - confidence < min_confidence（置信度低于阈值）
/// - edge 非正（无可下注的 edge）
///
/// 其他情况下返回 (kelly_fraction, side)。predicted_prob 在
/// 使用前被裁剪到 [0, 1]，以优雅处理 LLM 输出漂移。
pub fn kelly_fraction(
    signal: &Signal,
    config: &BankrollConfig,
) -> Option<(f64, BetSide)> {
    if signal.market_prob <= 0.0 || signal.market_prob >= 1.0 {
        return None; // 无法计算赔率
    }
    if signal.predicted_prob < 0.0 {
        return None; // 真正无效
    }
    let predicted = signal.predicted_prob.clamp(0.0, 1.0);
    if predicted <= 0.0 {
        return None;
    }
    if signal.confidence < config.min_confidence {
        return None;
    }
    let edge = signal.edge;
    if edge <= 0.0 {
        return None; // 没有可下注的 edge
    }
    let decimal_odds = 1.0 / signal.market_prob;
    let b = decimal_odds - 1.0; // 净赔率
    let p = predicted;
    let q = 1.0 - p;
    let f = (b * p - q) / b; // 完整凯利
    if f <= 0.0 {
        return None;
    }
    let side = if signal.edge > 0.0 { BetSide::Yes } else { BetSide::No };
    Some((f, side))
}

/// 按 `market_id` 对信号分组，每个市场保留 |edge| 最大的信号。
/// 返回合并后的信号（使用 max-edge 的值）和源 ID 列表。
/// **确定性顺序**：按 `market_id` 排序，使结果在 HashMap 哈希随机化下
/// 仍可复现。
fn group_by_market(signals: Vec<Signal>) -> Vec<(Signal, Vec<String>)> {
    let mut groups: HashMap<String, Signal> = HashMap::new();
    let mut ids: HashMap<String, Vec<String>> = HashMap::new();
    for s in signals {
        let id = s.computed_at.to_string(); // 由 computed_at 生成的合成 ID
        let entry = groups.entry(s.market_id.clone()).or_insert_with(|| s.clone());
        if s.edge.abs() > entry.edge.abs() {
            *entry = s.clone();
        }
        ids.entry(s.market_id.clone()).or_default().push(id);
    }
    let mut result: Vec<(String, Signal, Vec<String>)> = groups
        .into_iter()
        .map(|(k, v)| {
            let source_ids = ids.remove(&k).unwrap_or_default();
            (k, v, source_ids)
        })
        .collect();
    // 按 market_id 排序以保证确定性顺序
    result.sort_by(|a, b| a.0.cmp(&b.0));
    result
        .into_iter()
        .map(|(_, v, source_ids)| (v, source_ids))
        .collect()
}

/// 主入口：依据资金 + 配置 + 信号计算分配。
///
/// **算法**（参见设计文档 §1）：
/// 1. 过滤：|edge| < min_edge_pct → 丢弃
/// 2. 分组：同 market_id → 1 个信号（最大 |edge|）
/// 3. 单信号：raw_alloc = f_kelly * bankroll
/// 4. 上限约束：min(raw, max_per_signal_pct * bankroll, liquidity)
/// 5. 保留：total <= bankroll * (1 - reserve_pct)
/// 6. 若 total > max_total：按比例缩减所有项
/// 7. 保留两位小数
pub fn compute_allocation(input: &AllocationInput) -> AllocationResult {
    let bankroll = match parse_usdc(input.bankroll_usdc) {
        Some(b) if b > 0.0 => b,
        _ => {
            return AllocationResult {
                total_allocated_usdc: "0.00".to_string(),
                reserved_usdc: "0.00".to_string(),
                per_market: vec![],
                dropped_markets: vec![],
            };
        }
    };
    let config = input.config;
    let reserved = bankroll * config.reserve_pct;
    let max_total = bankroll * config.max_total_exposure_pct;
    let max_per_signal = bankroll * config.max_per_signal_pct;

    // 第 1 步：过滤信号
    let filtered: Vec<Signal> = input
        .signals
        .iter()
        .filter(|s| s.edge.abs() >= config.min_edge_pct)
        .cloned()
        .collect();

    if filtered.is_empty() {
        return AllocationResult {
            total_allocated_usdc: "0.00".to_string(),
            reserved_usdc: fmt_usdc(reserved),
            per_market: vec![],
            dropped_markets: vec![],
        };
    }

    // 第 2 步：按市场分组
    let grouped = group_by_market(filtered);

    // 第 3+4 步：单信号凯利 + 上限
    let mut items: Vec<AllocationItem> = Vec::new();
    let mut dropped: Vec<String> = vec![];
    for (signal, source_ids) in grouped {
        let kelly = match kelly_fraction(&signal, config) {
            Some((f, side)) => f * config.kelly_multiplier,
            None => continue,
        };
        let raw_alloc = kelly * bankroll;
        let mut alloc = raw_alloc;
        let mut capped: Option<CappedReason> = None;
        if alloc > max_per_signal {
            alloc = max_per_signal;
            capped = Some(CappedReason::PerSignalCap);
        }
        if let Some(liq) = input.market_liquidity {
            if let Some(liq_str) = liq.get(&signal.market_id) {
                if let Some(liq_f) = parse_usdc(liq_str) {
                    if alloc > liq_f {
                        alloc = liq_f;
                        capped = Some(CappedReason::Liquidity);
                    }
                }
            }
        }
        if alloc <= 0.0 {
            dropped.push(signal.market_id.clone());
            continue;
        }
        let side = if signal.edge > 0.0 { BetSide::Yes } else { BetSide::No };
        let expected_roi = signal.edge * signal.confidence;
        items.push(AllocationItem {
            market_id: signal.market_id.clone(),
            side,
            size_usdc: fmt_usdc(alloc),
            kelly_pct: kelly,
            capped_reason: capped,
            source_signal_ids: source_ids,
            model_version: signal.model_version.clone(),
            confidence: signal.confidence,
            expected_roi,
        });
    }

    // 第 5 步：total ≤ (bankroll - reserve)
    let total_raw: f64 = items
        .iter()
        .filter_map(|i| parse_usdc(&i.size_usdc))
        .sum();
    let cap_by_reserve = bankroll - reserved;
    let effective_cap = max_total.min(cap_by_reserve);

    // 第 6 步：若 total > effective_cap 则按比例缩减
    if total_raw > effective_cap && total_raw > 0.0 {
        let scale = effective_cap / total_raw;
        for item in &mut items {
            let cur = parse_usdc(&item.size_usdc).unwrap_or(0.0);
            let scaled = cur * scale;
            item.size_usdc = fmt_usdc(scaled);
            item.capped_reason = Some(CappedReason::TotalExposure);
        }
    }

    // 最终总额
    let total_alloc: f64 = items
        .iter()
        .filter_map(|i| parse_usdc(&i.size_usdc))
        .sum();

    AllocationResult {
        total_allocated_usdc: fmt_usdc(total_alloc),
        reserved_usdc: fmt_usdc(reserved),
        per_market: items,
        dropped_markets: dropped,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sig(market: &str, edge: f64, conf: f64) -> Signal {
        Signal {
            market_id: market.to_string(),
            computed_at: 1700000000000,
            model_version: "m1".to_string(),
            predicted_prob: 0.5 + edge,
            market_prob: 0.5,
            edge,
            confidence: conf,
            horizon_hours: 24,
            rationale: None,
        }
    }

    fn config() -> BankrollConfig {
        BankrollConfig::default()
    }

    // ============ 基础 API ============

    #[test]
    fn fmt_usdc_rounds_to_2_decimals() {
        assert_eq!(fmt_usdc(12.345), "12.35");
        assert_eq!(fmt_usdc(0.0), "0.00");
        assert_eq!(fmt_usdc(100.0), "100.00");
        assert_eq!(fmt_usdc(99.999), "100.00");
    }

    #[test]
    fn parse_usdc_handles_strings() {
        assert_eq!(parse_usdc("100"), Some(100.0));
        assert_eq!(parse_usdc("100.50"), Some(100.5));
        assert_eq!(parse_usdc("0"), Some(0.0));
        assert_eq!(parse_usdc("-5"), None);
        assert_eq!(parse_usdc("abc"), None);
        assert_eq!(parse_usdc("NaN"), None);
    }

    // ============ Kelly 边界情况 ============

    #[test]
    fn kelly_returns_none_for_market_prob_zero() {
        let mut s = sig("m1", 0.10, 0.8);
        s.market_prob = 0.0;
        assert!(kelly_fraction(&s, &config()).is_none());
    }

    #[test]
    fn kelly_returns_none_for_market_prob_one() {
        let mut s = sig("m1", 0.10, 0.8);
        s.market_prob = 1.0;
        assert!(kelly_fraction(&s, &config()).is_none());
    }

    #[test]
    fn kelly_returns_none_for_negative_edge() {
        let s = sig("m1", -0.10, 0.8);
        assert!(kelly_fraction(&s, &config()).is_none());
    }

    #[test]
    fn kelly_returns_none_for_zero_edge() {
        let s = sig("m1", 0.0, 0.8);
        assert!(kelly_fraction(&s, &config()).is_none());
    }

    #[test]
    fn kelly_returns_none_for_low_confidence() {
        let s = sig("m1", 0.10, 0.3); // 低于 0.6 最小值
        assert!(kelly_fraction(&s, &config()).is_none());
    }

    #[test]
    fn kelly_clamps_predicted_above_one() {
        let mut s = sig("m1", 0.10, 0.8);
        s.predicted_prob = 1.5;
        // 仍应能计算，但 predicted 被裁剪（=1.0）
        // edge 变为 0.5，market=0.5，p=1，q=0，b=1
        // f = (1*1-0)/1 = 1.0（完整凯利，将被上限约束）
        let (f, _side) = kelly_fraction(&s, &config()).unwrap();
        assert!(f > 0.0);
        assert!(f <= 1.0);
    }

    #[test]
    fn kelly_positive_for_high_confidence_edge() {
        // edge=0.20, conf=0.9, market=0.5 → decimal_odds=2, b=1（凯利输入参数）
        // p=0.7, q=0.3, f=(1*0.7-0.3)/1=0.4（凯利比例）
        let s = sig("m1", 0.20, 0.9);
        let (f, side) = kelly_fraction(&s, &config()).unwrap();
        assert!((f - 0.4).abs() < 0.01);
        assert_eq!(side, BetSide::Yes);
    }

    // ============ 分配：零资金 ============

    #[test]
    fn zero_bankroll_returns_empty() {
        let signals = vec![sig("m1", 0.10, 0.8)];
        let input = AllocationInput {
            bankroll_usdc: "0",
            config: &config(),
            signals: &signals,
            market_liquidity: None,
        };
        let r = compute_allocation(&input);
        assert_eq!(r.total_allocated_usdc, "0.00");
        assert_eq!(r.reserved_usdc, "0.00");
        assert!(r.per_market.is_empty());
    }

    #[test]
    fn empty_signals_returns_empty() {
        let input = AllocationInput {
            bankroll_usdc: "1000",
            config: &config(),
            signals: &[],
            market_liquidity: None,
        };
        let r = compute_allocation(&input);
        assert_eq!(r.total_allocated_usdc, "0.00");
        // 保留为 1000 的 20% = 200
        assert_eq!(r.reserved_usdc, "200.00");
    }

    #[test]
    fn all_signals_below_min_edge_dropped() {
        let signals = vec![sig("m1", 0.02, 0.8), sig("m2", 0.01, 0.8)];
        let input = AllocationInput {
            bankroll_usdc: "1000",
            config: &config(),
            signals: &signals,
            market_liquidity: None,
        };
        let r = compute_allocation(&input);
        assert!(r.per_market.is_empty());
    }

    // ============ 单信号上限 ============

    #[test]
    fn huge_kelly_capped_at_max_per_signal() {
        // 100% Kelly，0.25 倍数 = 每个信号占资金的 25%
        // max_per_signal_pct = 10% → 应被限制为 100
        let signals = vec![sig("m1", 0.30, 0.99)];
        let input = AllocationInput {
            bankroll_usdc: "1000",
            config: &config(),
            signals: &signals,
            market_liquidity: None,
        };
        let r = compute_allocation(&input);
        assert_eq!(r.per_market.len(), 1);
        assert_eq!(r.per_market[0].size_usdc, "100.00");
        assert_eq!(r.per_market[0].capped_reason, Some(CappedReason::PerSignalCap));
    }

    // ============ 流动性上限 ============

    #[test]
    fn liquidity_caps_allocation() {
        let mut liq = HashMap::new();
        liq.insert("m1".to_string(), "30".to_string()); // 仅 $30 流动性
        let signals = vec![sig("m1", 0.10, 0.8)];
        let input = AllocationInput {
            bankroll_usdc: "1000",
            config: &config(),
            signals: &signals,
            market_liquidity: Some(&liq),
        };
        let r = compute_allocation(&input);
        assert_eq!(r.per_market[0].size_usdc, "30.00");
        assert_eq!(r.per_market[0].capped_reason, Some(CappedReason::Liquidity));
    }

    // ============ 同市场合并 ============

    #[test]
    fn same_market_signals_grouped() {
        let signals = vec![
            Signal {
                market_id: "m1".to_string(),
                computed_at: 1,
                model_version: "m1".to_string(),
                predicted_prob: 0.55,
                market_prob: 0.5,
                edge: 0.05,
                confidence: 0.7,
                horizon_hours: 24,
                rationale: None,
            },
            Signal {
                market_id: "m1".to_string(),
                computed_at: 2,
                model_version: "m2".to_string(),
                predicted_prob: 0.70,
                market_prob: 0.5,
                edge: 0.20,
                confidence: 0.9,
                horizon_hours: 24,
                rationale: None,
            },
        ];
        let input = AllocationInput {
            bankroll_usdc: "1000",
            config: &config(),
            signals: &signals,
            market_liquidity: None,
        };
        let r = compute_allocation(&input);
        assert_eq!(r.per_market.len(), 1); // 已合并
        // 较高 |edge|（0.20）胜出
        assert_eq!(r.per_market[0].confidence, 0.9);
        assert_eq!(r.per_market[0].model_version, "m2");
    }

    // ============ 总敞口上限 ============

    #[test]
    fn total_exposure_caps_via_proportional_scale() {
        // 10 个 edge=0.30 conf=0.99 的信号 → 每个都希望大额分配
        // 没有总上限时：会超配。设置 80% 上限：按比例缩减。
        let signals: Vec<Signal> = (0..10)
            .map(|i| sig(&format!("m{}", i), 0.30, 0.99))
            .collect();
        let input = AllocationInput {
            bankroll_usdc: "1000",
            config: &config(),
            signals: &signals,
            market_liquidity: None,
        };
        let r = compute_allocation(&input);
        // 单信号上限 = $100（1000 的 10%）
        // 10 个信号 = $1000 原始。有效上限 = min(800, 800) = 800。
        // 比例 = 0.8 → 每个 = $80
        assert!(r.per_market.len() == 10);
        for item in &r.per_market {
            assert_eq!(item.size_usdc, "80.00");
            assert_eq!(item.capped_reason, Some(CappedReason::TotalExposure));
        }
        assert_eq!(r.total_allocated_usdc, "800.00");
    }

    // ============ 确定性 ============

    #[test]
    fn same_input_same_output() {
        let signals = vec![sig("m1", 0.10, 0.8), sig("m2", 0.15, 0.7)];
        let input1 = AllocationInput {
            bankroll_usdc: "1000",
            config: &config(),
            signals: &signals,
            market_liquidity: None,
        };
        let input2 = AllocationInput {
            bankroll_usdc: "1000",
            config: &config(),
            signals: &signals,
            market_liquidity: None,
        };
        let r1 = compute_allocation(&input1);
        let r2 = compute_allocation(&input2);
        assert_eq!(r1.total_allocated_usdc, r2.total_allocated_usdc);
        assert_eq!(r1.per_market.len(), r2.per_market.len());
        for (a, b) in r1.per_market.iter().zip(r2.per_market.iter()) {
            assert_eq!(a.size_usdc, b.size_usdc);
            assert_eq!(a.market_id, b.market_id);
        }
    }
}
