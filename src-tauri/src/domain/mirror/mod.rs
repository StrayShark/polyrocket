//! L3 — Mirror queue executor (M5 auto-execution).
//!
//! Decides which pending mirrors should be submitted as Mode B bets
//! RIGHT NOW, given current capacity (one bet per cycle to avoid
//! overwhelming the user with auto-trades).
//!
//! State machine (already in domain::copy::MirrorStatus):
//!   Pending → Submitted (after executor picks it up)
//!   Pending → Rejected (insufficient capacity, market closed, etc.)
//!   Submitted → Filled | Rejected (later, when on-chain confirmation
//!                                       arrives — M5 phase 2)
//!
//! v0.6a — pure decision logic. The L2 scheduler (infra::scheduler)
//! is the runtime that calls `pick_next_mirror` every N seconds.

use crate::domain::copy::{MirrorOrder, MirrorStatus};
use crate::AppError;
use crate::AppResult;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Configurable caps for the auto-executor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutorConfig {
    /// Maximum total USDC exposure across all open mirrors.
    pub max_total_exposure_usdc: f64,
    /// Maximum number of mirrors to submit per cycle.
    pub max_per_cycle: usize,
    /// Minimum size to bother with (dust filter).
    pub min_size_usdc: f64,
    /// Don't submit mirrors for markets closing in less than this many
    /// hours (too risky to settle).
    pub min_horizon_hours: i64,
    /// v0.44a — paper mode. When true, picked
    /// mirrors go to the `paper_fills` table
    /// instead of `bets`, and the CLOB signing
    /// step is skipped. The decision logic
    /// (what to pick, what to reject) is
    /// unchanged — paper mode only changes the
    /// write path. This lets the user validate
    /// their config (sizing, exposure caps,
    /// frequency) without risking real money.
    ///
    /// Default false (live mode). Runtime
    /// override via `set_mirror_executor_paper_mode`
    /// IPC; env-var default via
    /// `POLYROCKET_MIRROR_PAPER_MODE=1`.
    pub paper_mode: bool,
}

impl Default for ExecutorConfig {
    fn default() -> Self {
        Self {
            max_total_exposure_usdc: 500.0,
            max_per_cycle: 1,
            min_size_usdc: 5.0,
            min_horizon_hours: 1,
            paper_mode: false,
        }
    }
}

impl ExecutorConfig {
    pub fn from_env() -> Self {
        let max_total = std::env::var("POLYROCKET_MIRROR_MAX_EXPOSURE_USDC")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(500.0);
        let max_per_cycle = std::env::var("POLYROCKET_MIRROR_MAX_PER_CYCLE")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(1);
        let min_size = std::env::var("POLYROCKET_MIRROR_MIN_SIZE_USDC")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(5.0);
        let min_horizon = std::env::var("POLYROCKET_MIRROR_MIN_HORIZON_HOURS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(1);
        let paper_mode = std::env::var("POLYROCKET_MIRROR_PAPER_MODE")
            .ok()
            .map(|s| s == "1" || s.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        Self {
            max_total_exposure_usdc: max_total,
            max_per_cycle,
            min_size_usdc: min_size,
            min_horizon_hours: min_horizon,
            paper_mode,
        }
    }
}

/// Reasons a mirror was rejected (for audit log + UI).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RejectReason {
    /// Size below min_size_usdc
    TooSmall,
    /// Market closes too soon
    TooShortHorizon,
    /// Total exposure would exceed cap
    OverExposure,
    /// Order is older than max_age_ms (stale)
    Stale,
    /// Internal: missing market_id
    Invalid,
}

impl RejectReason {
    pub fn as_str(self) -> &'static str {
        match self {
            RejectReason::TooSmall => "too_small",
            RejectReason::TooShortHorizon => "too_short_horizon",
            RejectReason::OverExposure => "over_exposure",
            RejectReason::Stale => "stale",
            RejectReason::Invalid => "invalid",
        }
    }
}

/// Compute current total USDC exposure across all in-flight mirrors.
pub fn current_exposure(orders: &[MirrorOrder]) -> f64 {
    orders
        .iter()
        .filter(|o| matches!(o.status, MirrorStatus::Pending | MirrorStatus::Submitted))
        .filter_map(|o| o.size.parse::<f64>().ok())
        .sum()
}

/// Pick the next batch of mirrors to submit. Returns the IDs of
/// mirrors that should be promoted Pending → Submitted, in order.
///
/// Inputs:
/// - `orders` — current mirror queue (any status)
/// - `market_closes_at` — map of market_id → close timestamp (ms).
///   The L2 caller resolves this from the markets table.
/// - `now_ms` — current time
/// - `cfg` — executor config
pub fn pick_next_mirror(
    orders: &[MirrorOrder],
    market_closes_at: &std::collections::HashMap<String, i64>,
    now_ms: i64,
    cfg: &ExecutorConfig,
) -> Vec<String> {
    // Headroom counts only SUBMITTED mirrors (already locked-in exposure);
    // pending candidates are evaluated individually against the cap.
    let submitted_exposure: f64 = orders
        .iter()
        .filter(|o| o.status == MirrorStatus::Submitted)
        .filter_map(|o| o.size.parse::<f64>().ok())
        .sum();
    let headroom = (cfg.max_total_exposure_usdc - submitted_exposure).max(0.0);

    // Eligible = Pending AND size ≥ min AND not stale AND horizon OK
    let mut eligible: Vec<&MirrorOrder> = orders
        .iter()
        .filter(|o| o.status == MirrorStatus::Pending)
        .filter(|o| {
            o.size.parse::<f64>().map(|n| n >= cfg.min_size_usdc).unwrap_or(false)
        })
        .filter(|o| now_ms.saturating_sub(o.created_at) < 24 * 3600 * 1000) // < 24h
        .filter(|o| {
            match market_closes_at.get(&o.market_id) {
                Some(close_ms) => (close_ms - now_ms) / 3_600_000 >= cfg.min_horizon_hours,
                None => false, // unknown close → skip
            }
        })
        .collect();
    // Newest first (FIFO)
    eligible.sort_by_key(|o| std::cmp::Reverse(o.created_at));

    let mut out = Vec::new();
    let mut used = 0.0;
    for o in eligible {
        if out.len() >= cfg.max_per_cycle {
            break;
        }
        let size: f64 = o.size.parse().unwrap_or(0.0);
        if used + size > headroom {
            continue; // would bust the cap; skip but try next
        }
        out.push(o.id.clone());
        used += size;
    }
    out
}

/// Decide which pending mirrors to reject this cycle (and why).
/// Returns a vec of (mirror_id, reason).
pub fn find_rejections(
    orders: &[MirrorOrder],
    market_closes_at: &std::collections::HashMap<String, i64>,
    now_ms: i64,
    cfg: &ExecutorConfig,
) -> Vec<(String, RejectReason)> {
    let mut out = Vec::new();
    for o in orders {
        if o.status != MirrorStatus::Pending {
            continue;
        }
        let size: f64 = o.size.parse().unwrap_or(0.0);
        if size <= 0.0 {
            out.push((o.id.clone(), RejectReason::Invalid));
            continue;
        }
        if size < cfg.min_size_usdc {
            out.push((o.id.clone(), RejectReason::TooSmall));
            continue;
        }
        if now_ms.saturating_sub(o.created_at) > 24 * 3600 * 1000 {
            out.push((o.id.clone(), RejectReason::Stale));
            continue;
        }
        match market_closes_at.get(&o.market_id) {
            Some(close_ms) => {
                let hours = (close_ms - now_ms) / 3_600_000;
                if hours < cfg.min_horizon_hours {
                    out.push((o.id.clone(), RejectReason::TooShortHorizon));
                }
            }
            None => out.push((o.id.clone(), RejectReason::Invalid)),
        }
    }
    out
}

/// Pretty-print a timestamp for the audit log.
pub fn fmt_ts(ms: i64) -> String {
    DateTime::<Utc>::from_timestamp_millis(ms)
        .map(|d| d.format("%Y-%m-%dT%H:%M:%SZ").to_string())
        .unwrap_or_else(|| format!("invalid_ts_{ms}"))
}

/// Run a one-shot executor pass. Pure function — caller does the IO.
pub fn execute_pass(
    orders: &[MirrorOrder],
    market_closes_at: &std::collections::HashMap<String, i64>,
    now_ms: i64,
    cfg: &ExecutorConfig,
) -> AppResult<ExecutorPassResult> {
    let picks = pick_next_mirror(orders, market_closes_at, now_ms, cfg);
    let rejects = find_rejections(orders, market_closes_at, now_ms, cfg);
    Ok(ExecutorPassResult {
        picked: picks,
        rejected: rejects,
        current_exposure: current_exposure(orders),
        headroom: (cfg.max_total_exposure_usdc - current_exposure(orders)).max(0.0),
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutorPassResult {
    pub picked: Vec<String>,
    pub rejected: Vec<(String, RejectReason)>,
    pub current_exposure: f64,
    pub headroom: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::copy::{MirrorDecision, build_mirror, CopyEvent};

    fn mk_order(market: &str, size: &str, created_at: i64) -> MirrorOrder {
        let ev = CopyEvent {
            id: 1,
            target_id: "t".into(),
            market_id: market.into(),
            detected_at: created_at,
            side: "YES".into(),
            size: size.into(),
            price: 0.5,
            tx_hash: format!("0x{market}_tx"),
            matched_bet_id: None,
        };
        let d = MirrorDecision { side: "YES".into(), size: size.into(), flip: false };
        let mut o = build_mirror(&ev, &d, created_at);
        o.status = MirrorStatus::Pending;
        o
    }

    fn now() -> i64 {
        1_700_000_000_000
    }

    fn closes(market: &str, hours_from_now: i64) -> (String, i64) {
        (market.into(), now() + hours_from_now * 3_600_000)
    }

    #[test]
    fn default_config() {
        let c = ExecutorConfig::default();
        assert_eq!(c.max_per_cycle, 1);
        assert_eq!(c.min_size_usdc, 5.0);
    }

    #[test]
    fn current_exposure_sums_in_flight() {
        let mut orders = vec![
            mk_order("m1", "100", now() - 1000),
            mk_order("m2", "50", now() - 2000),
        ];
        // third one is filled → not counted
        let mut o3 = mk_order("m3", "999", now() - 3000);
        o3.status = MirrorStatus::Filled;
        orders.push(o3);
        assert!((current_exposure(&orders) - 150.0).abs() < 1e-9);
    }

    #[test]
    fn pick_respects_min_size() {
        let cfg = ExecutorConfig { min_size_usdc: 10.0, ..Default::default() };
        let orders = vec![mk_order("m1", "5", now())];
        let closes = std::collections::HashMap::from([closes("m1", 24)]);
        let picks = pick_next_mirror(&orders, &closes, now(), &cfg);
        assert!(picks.is_empty());
    }

    #[test]
    fn pick_respects_min_horizon() {
        let cfg = ExecutorConfig { min_horizon_hours: 12, ..Default::default() };
        let orders = vec![mk_order("m1", "50", now())];
        let closes = std::collections::HashMap::from([closes("m1", 2)]);
        let picks = pick_next_mirror(&orders, &closes, now(), &cfg);
        assert!(picks.is_empty());
    }

    #[test]
    fn pick_skips_stale_orders() {
        let cfg = ExecutorConfig { min_horizon_hours: 0, ..Default::default() };
        let old = mk_order("m1", "50", now() - 48 * 3600 * 1000);
        let closes = std::collections::HashMap::from([closes("m1", 24)]);
        let picks = pick_next_mirror(&[old], &closes, now(), &cfg);
        assert!(picks.is_empty());
    }

    #[test]
    fn pick_caps_per_cycle() {
        let cfg = ExecutorConfig { max_per_cycle: 2, ..Default::default() };
        let orders = vec![
            mk_order("m1", "10", now() - 1000),
            mk_order("m2", "10", now() - 2000),
            mk_order("m3", "10", now() - 3000),
        ];
        let closes = std::collections::HashMap::from([
            closes("m1", 24), closes("m2", 24), closes("m3", 24),
        ]);
        let picks = pick_next_mirror(&orders, &closes, now(), &cfg);
        assert_eq!(picks.len(), 2);
    }

    #[test]
    fn pick_respects_exposure_cap() {
        let cfg = ExecutorConfig { max_total_exposure_usdc: 50.0, ..Default::default() };
        // Existing exposure: 30 (one submitted). Headroom: 20.
        let mut existing = mk_order("mx", "30", now() - 5000);
        existing.status = MirrorStatus::Submitted;
        let candidates = vec![
            mk_order("m1", "15", now() - 1000), // would fit (30+15=45)
            mk_order("m2", "25", now() - 2000), // busts cap (30+25=55)
        ];
        let mut orders = vec![existing];
        orders.extend(candidates);
        let closes = std::collections::HashMap::from([closes("m1", 24), closes("m2", 24)]);
        let picks = pick_next_mirror(&orders, &closes, now(), &cfg);
        assert_eq!(picks.len(), 1);
        // id format = mir_{tx_hash}_{market} = mir_0xm1_tx_m1
        assert_eq!(picks[0], "mir_0xm1_tx_m1");
    }

    #[test]
    fn find_rejections_classifies_correctly() {
        let cfg = ExecutorConfig { min_size_usdc: 10.0, min_horizon_hours: 6, ..Default::default() };
        let orders = vec![
            mk_order("small", "5", now()),    // TooSmall
            mk_order("short", "50", now()),   // TooShortHorizon
            mk_order("ok", "50", now()),      // no rejection
            mk_order("stale", "50", now() - 48 * 3600 * 1000), // Stale
        ];
        let closes = std::collections::HashMap::from([
            closes("short", 1),
            closes("ok", 24),
            closes("stale", 24),
        ]);
        let rejects = find_rejections(&orders, &closes, now(), &cfg);
        assert_eq!(rejects.len(), 3);
        let by_id: std::collections::HashMap<_, _> = rejects.into_iter().collect();
        // id format = mir_{event.tx_hash}_{market} = mir_0xsmall_tx_small
        assert_eq!(by_id["mir_0xsmall_tx_small"], RejectReason::TooSmall);
        assert_eq!(by_id["mir_0xshort_tx_short"], RejectReason::TooShortHorizon);
        assert_eq!(by_id["mir_0xstale_tx_stale"], RejectReason::Stale);
    }

    #[test]
    fn execute_pass_returns_full_summary() {
        let cfg = ExecutorConfig::default();
        let orders = vec![mk_order("m1", "50", now())];
        let closes = std::collections::HashMap::from([closes("m1", 24)]);
        let r = execute_pass(&orders, &closes, now(), &cfg).unwrap();
        assert_eq!(r.picked.len(), 1);
        assert!(r.headroom > 0.0);
        assert!(r.current_exposure < cfg.max_total_exposure_usdc);
    }

    #[test]
    fn fmt_ts_round_trip() {
        let s = fmt_ts(1_700_000_000_000);
        assert!(s.starts_with("2023-"));
        assert!(s.ends_with('Z'));
    }

    // v0.44a — paper_mode default + env override.
    // Each test starts by clearing the env var so
    // the test order doesn't matter (cargo runs
    // tests in parallel; without this, the
    // `paper_mode_1` test would leak its env var
    // to other tests in the same binary). The
    // `serial_test` crate would be the cleanest
    // fix, but to avoid adding a new dep, we just
    // run these as part of the same test (combined
    // into one sequential test) and assert all the
    // cases at once.
    #[test]
    fn executor_config_paper_mode_all_cases() {
        // Default off
        std::env::remove_var("POLYROCKET_MIRROR_PAPER_MODE");
        let c = ExecutorConfig::default();
        assert!(!c.paper_mode);
        let c = ExecutorConfig::from_env();
        assert!(!c.paper_mode);

        // 1 = on
        std::env::set_var("POLYROCKET_MIRROR_PAPER_MODE", "1");
        let c = ExecutorConfig::from_env();
        assert!(c.paper_mode);

        // "TRUE" case-insensitive = on
        std::env::set_var("POLYROCKET_MIRROR_PAPER_MODE", "TRUE");
        let c = ExecutorConfig::from_env();
        assert!(c.paper_mode);

        // 0 = off
        std::env::set_var("POLYROCKET_MIRROR_PAPER_MODE", "0");
        let c = ExecutorConfig::from_env();
        assert!(!c.paper_mode);

        // Unset = off
        std::env::remove_var("POLYROCKET_MIRROR_PAPER_MODE");
        let c = ExecutorConfig::from_env();
        assert!(!c.paper_mode);
    }
}
