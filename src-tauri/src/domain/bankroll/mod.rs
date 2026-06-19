//! L3 — Bankroll allocation (M11, v0.78).
//!
//! **What this module owns**: the pure-function algorithm that turns a
//! list of `Signal` (with `edge`, `confidence`, `market_prob`) plus a
//! `BankrollConfig` into a `Vec<AllocationItem>` (per-market $ amount).
//!
//! **What this module does NOT own**: DB writes, IPC, UI. Those are
//! L2 (commands) and L1 (React) respectively. This module is **pure
//! functions + pure data** — no `AppState`, no SQL, no `tokio::spawn`.
//!
//! **Algorithm**: Fractional Kelly Criterion + multi-constraint cap.
//! See `docs/bankroll-allocation-design.md` §1 for the full design.
//!
//! Spec: docs/polyrocket-modules.md M11.

use crate::domain::signal::Signal;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::HashMap;

/// Per-wallet bankroll configuration. Stored in `bankroll_config` table.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BankrollConfig {
    /// Fraction of full Kelly to use. 0.25 = quarter-Kelly (default).
    /// Range: (0, 1]. >1.0 is allowed but discouraged.
    pub kelly_multiplier: f64,
    /// Hard cap on per-signal allocation as fraction of bankroll.
    /// Range: (0, 1]. Default 0.10 = 10%.
    pub max_per_signal_pct: f64,
    /// Reserve fraction never allocated. Default 0.20 = 20%.
    /// Range: [0, 1).
    pub reserve_pct: f64,
    /// Minimum |edge| to be considered. Default 0.05 = 5%.
    /// Signals below this are dropped at filter step 1.
    pub min_edge_pct: f64,
    /// Maximum total exposure as fraction of bankroll. Default 0.80.
    /// Range: (0, 1].
    pub max_total_exposure_pct: f64,
    /// Minimum confidence to consider. Default 0.6.
    /// Range: [0, 1].
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

/// Side of the bet (Yes/No token on a prediction market).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Type)]
pub enum BetSide {
    Yes,
    No,
}

/// One allocation for a single market. Multiple signals on the same
/// market are grouped into a single `AllocationItem` (with
/// `source_signal_ids` listing the merged signals).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Type)]
pub struct AllocationItem {
    pub market_id: String,
    pub side: BetSide,
    /// Final size in USDC, rounded to 2 decimals (string to avoid
    /// float precision issues — the DB column is TEXT).
    pub size_usdc: String,
    /// Raw Kelly fraction before any caps. Range: [0, 1].
    pub kelly_pct: f64,
    /// Why this allocation was capped, if at all.
    pub capped_reason: Option<CappedReason>,
    /// Original signal IDs that were merged into this allocation.
    pub source_signal_ids: Vec<String>,
    /// Model version (most recent among merged signals).
    pub model_version: String,
    /// Confidence (max across merged signals).
    pub confidence: f64,
    /// Expected ROI = edge × confidence. Used for display only.
    pub expected_roi: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Type)]
pub enum CappedReason {
    /// Allocation was capped by `max_per_signal_pct`.
    PerSignalCap,
    /// Allocation was capped by market liquidity.
    Liquidity,
    /// Total exposure > max_total_exposure_pct, scaled down.
    TotalExposure,
}

/// Final allocation result.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Type)]
pub struct AllocationResult {
    /// Total USDC allocated across all markets (sum of size_usdc).
    pub total_allocated_usdc: String,
    /// USDC held in reserve (not allocated).
    pub reserved_usdc: String,
    /// Per-market allocations.
    pub per_market: Vec<AllocationItem>,
    /// Markets dropped due to liquidity (kelly > 0 but couldn't fit).
    pub dropped_markets: Vec<String>,
}

/// Input to `compute_allocation`.
#[derive(Debug, Clone)]
pub struct AllocationInput<'a> {
    /// Total available USDC (string to match DB TEXT columns).
    /// Must parse to f64 > 0. Returns empty result if 0.
    pub bankroll_usdc: &'a str,
    /// Per-wallet configuration.
    pub config: &'a BankrollConfig,
    /// Signals to consider. Same-market signals are grouped.
    pub signals: &'a [Signal],
    /// Optional: market_id → max USDC allocatable (liquidity cap).
    pub market_liquidity: Option<&'a HashMap<String, String>>,
}

/// Parse USDC string → f64. Returns None on parse error or non-finite.
fn parse_usdc(s: &str) -> Option<f64> {
    let v: f64 = s.parse().ok()?;
    if !v.is_finite() || v < 0.0 {
        return None;
    }
    Some(v)
}

/// Format f64 → USDC string with 2 decimals (banker's round).
fn fmt_usdc(v: f64) -> String {
    // Round to 2 decimals. We use `* 100.0` + `round` / `100.0` for
    // cent precision. `f64::round` is banker's-round in IEEE 754.
    let cents = (v * 100.0).round();
    format!("{:.2}", cents / 100.0)
}

/// Compute the Kelly fraction for a single signal.
///
/// Returns None if:
/// - market_prob is 0 or 1 (can't compute odds)
/// - predicted_prob is invalid (< 0; > 1.5 is clamped, ≤ 0 returns None)
/// - confidence < min_confidence
/// - edge is non-positive (no edge to bet on)
///
/// Otherwise returns (kelly_fraction, side). predicted_prob is clamped
/// to [0, 1] before use to handle LLM output drift gracefully.
pub fn kelly_fraction(
    signal: &Signal,
    config: &BankrollConfig,
) -> Option<(f64, BetSide)> {
    if signal.market_prob <= 0.0 || signal.market_prob >= 1.0 {
        return None; // can't compute odds
    }
    if signal.predicted_prob < 0.0 {
        return None; // truly invalid
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
        return None; // no edge to bet on
    }
    let decimal_odds = 1.0 / signal.market_prob;
    let b = decimal_odds - 1.0; // net odds
    let p = predicted;
    let q = 1.0 - p;
    let f = (b * p - q) / b; // full Kelly
    if f <= 0.0 {
        return None;
    }
    let side = if signal.edge > 0.0 { BetSide::Yes } else { BetSide::No };
    Some((f, side))
}

/// Group signals by `market_id`, keeping the one with max |edge| per
/// market. Returns the merged signal (using max-edge values) + a list
/// of source IDs. **Deterministic order**: sorted by `market_id` to
/// make the result reproducible across HashMap hash randomization.
fn group_by_market(signals: Vec<Signal>) -> Vec<(Signal, Vec<String>)> {
    let mut groups: HashMap<String, Signal> = HashMap::new();
    let mut ids: HashMap<String, Vec<String>> = HashMap::new();
    for s in signals {
        let id = s.computed_at.to_string(); // synthetic ID from computed_at
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
    // Sort by market_id for deterministic order
    result.sort_by(|a, b| a.0.cmp(&b.0));
    result
        .into_iter()
        .map(|(_, v, source_ids)| (v, source_ids))
        .collect()
}

/// Main entry: compute allocation given bankroll + config + signals.
///
/// **Algorithm** (see design doc §1):
/// 1. Filter: |edge| < min_edge_pct → drop
/// 2. Group: same market_id → 1 signal (max |edge|)
/// 3. Per-signal: raw_alloc = f_kelly * bankroll
/// 4. Cap each: min(raw, max_per_signal_pct * bankroll, liquidity)
/// 5. Reserve: total <= bankroll * (1 - reserve_pct)
/// 6. If total > max_total: scale all proportionally
/// 7. Round to 2 decimals
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

    // Step 1: filter signals
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

    // Step 2: group by market
    let grouped = group_by_market(filtered);

    // Step 3+4: per-signal kelly + cap
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

    // Step 5: total ≤ (bankroll - reserve)
    let total_raw: f64 = items
        .iter()
        .filter_map(|i| parse_usdc(&i.size_usdc))
        .sum();
    let cap_by_reserve = bankroll - reserved;
    let effective_cap = max_total.min(cap_by_reserve);

    // Step 6: scale down if total > effective_cap
    if total_raw > effective_cap && total_raw > 0.0 {
        let scale = effective_cap / total_raw;
        for item in &mut items {
            let cur = parse_usdc(&item.size_usdc).unwrap_or(0.0);
            let scaled = cur * scale;
            item.size_usdc = fmt_usdc(scaled);
            item.capped_reason = Some(CappedReason::TotalExposure);
        }
    }

    // Final total
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

    // ============ Basic API ============

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

    // ============ Kelly edge cases ============

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
        let s = sig("m1", 0.10, 0.3); // below 0.6 min
        assert!(kelly_fraction(&s, &config()).is_none());
    }

    #[test]
    fn kelly_clamps_predicted_above_one() {
        let mut s = sig("m1", 0.10, 0.8);
        s.predicted_prob = 1.5;
        // Should still compute, but with clamped predicted (=1.0)
        // edge becomes 0.5, market=0.5, p=1, q=0, b=1
        // f = (1*1-0)/1 = 1.0 (full Kelly, will be capped)
        let (f, _side) = kelly_fraction(&s, &config()).unwrap();
        assert!(f > 0.0);
        assert!(f <= 1.0);
    }

    #[test]
    fn kelly_positive_for_high_confidence_edge() {
        // edge=0.20, conf=0.9, market=0.5 → decimal_odds=2, b=1
        // p=0.7, q=0.3, f=(1*0.7-0.3)/1=0.4
        let s = sig("m1", 0.20, 0.9);
        let (f, side) = kelly_fraction(&s, &config()).unwrap();
        assert!((f - 0.4).abs() < 0.01);
        assert_eq!(side, BetSide::Yes);
    }

    // ============ Allocation: zero bankroll ============

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
        // Reserve is 20% of 1000 = 200
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

    // ============ Per-signal cap ============

    #[test]
    fn huge_kelly_capped_at_max_per_signal() {
        // 100% Kelly with 0.25 multiplier = 25% of bankroll per signal
        // max_per_signal_pct = 10% → should cap at 100
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

    // ============ Liquidity cap ============

    #[test]
    fn liquidity_caps_allocation() {
        let mut liq = HashMap::new();
        liq.insert("m1".to_string(), "30".to_string()); // only $30 liquidity
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

    // ============ Same market grouping ============

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
        assert_eq!(r.per_market.len(), 1); // grouped
        // Higher |edge| (0.20) wins
        assert_eq!(r.per_market[0].confidence, 0.9);
        assert_eq!(r.per_market[0].model_version, "m2");
    }

    // ============ Total exposure cap ============

    #[test]
    fn total_exposure_caps_via_proportional_scale() {
        // 10 signals with edge=0.30 conf=0.99 → each wants huge allocation
        // Without total cap: would over-allocate. With 80% cap: scale down.
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
        // Per-signal cap = $100 (10% of 1000)
        // 10 signals = $1000 raw. Effective cap = min(800, 800) = 800.
        // Scale = 0.8 → each = $80
        assert!(r.per_market.len() == 10);
        for item in &r.per_market {
            assert_eq!(item.size_usdc, "80.00");
            assert_eq!(item.capped_reason, Some(CappedReason::TotalExposure));
        }
        assert_eq!(r.total_allocated_usdc, "800.00");
    }

    // ============ Determinism ============

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
