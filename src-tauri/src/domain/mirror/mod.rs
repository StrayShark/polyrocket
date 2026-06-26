//! L3 — Mirror 队列执行器 (M5 自动执行)。
//!
//! 根据当前容量,决定哪些 pending mirrors 应该作为 Mode B 投注
//! 立即提交(每周期一笔,避免自动交易压垮用户)。
//!
//! 状态机 (已存在于 domain::copy::MirrorStatus):
//!   Pending → Submitted (执行器拾取后)
//!   Pending → Rejected (容量不足、市场已关闭等)
//!   Submitted → Filled | Rejected (后续链上确认到达时 — M5 phase 2)
//!
//! v0.6a — 纯决策逻辑。L2 scheduler (infra::scheduler) 是运行时,
//! 每 N 秒调用 `pick_next_mirror`。

use crate::domain::copy::{MirrorOrder, MirrorStatus};
use crate::AppError;
use crate::AppResult;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 自动执行器的可配置上限。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutorConfig {
    /// 所有未平仓 mirror 的最大总 USDC 敞口。
    pub max_total_exposure_usdc: f64,
    /// 每周期提交的最大 mirror 数量。
    pub max_per_cycle: usize,
    /// 值得处理的最小尺寸(dust filter)。
    pub min_size_usdc: f64,
    /// 不要提交距收市时间少于这么多小时的 mirror
    /// (结算风险太高)。
    pub min_horizon_hours: i64,
    /// v0.44a — paper mode（纸面模式）。开启时，被选中的 mirror
    /// 写入 `paper_fills` 表而不是 `bets`，且跳过 CLOB 签名步骤。
    /// 决策逻辑（选什么、拒绝什么）保持不变 —— paper mode
    /// 只改变写入路径。这让用户能验证自己的配置（仓位、暴露上限、
    /// 频率）而不冒真实资金的风险。
    ///
    /// 默认 false（实盘模式）。运行时可通过
    /// `set_mirror_executor_paper_mode` IPC 覆盖；
    /// 环境变量默认通过 `POLYROCKET_MIRROR_PAPER_MODE=1`。
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

/// Mirror 被拒绝的原因(用于 audit log + UI)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RejectReason {
    /// Size 低于 min_size_usdc
    TooSmall,
    /// Market 收市时间过早
    TooShortHorizon,
    /// 总敞口将超过上限
    OverExposure,
    /// Order 超过 max_age_ms (陈旧)
    Stale,
    /// 内部: 缺失 market_id
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

/// 计算所有进行中 mirror 的当前总 USDC 敞口。
pub fn current_exposure(orders: &[MirrorOrder]) -> f64 {
    orders
        .iter()
        .filter(|o| matches!(o.status, MirrorStatus::Pending | MirrorStatus::Submitted))
        .filter_map(|o| o.size.parse::<f64>().ok())
        .sum()
}

/// 挑选下一批要提交的 mirrors。返回应被提升 Pending → Submitted 的
/// mirror ID,按顺序排列。
///
/// 输入:
/// - `orders` — 当前 mirror 队列(任何状态)
/// - `market_closes_at` — market_id → 收市时间戳 (ms) 的映射。
///   L2 调用方从 markets 表解析这个。
/// - `now_ms` — 当前时间
/// - `cfg` — executor 配置
pub fn pick_next_mirror(
    orders: &[MirrorOrder],
    market_closes_at: &std::collections::HashMap<String, i64>,
    now_ms: i64,
    cfg: &ExecutorConfig,
) -> Vec<String> {
    // Headroom 只计算 SUBMITTED mirrors (已锁定的敞口);
    // pending candidates 单独与上限对比评估。
    let submitted_exposure: f64 = orders
        .iter()
        .filter(|o| o.status == MirrorStatus::Submitted)
        .filter_map(|o| o.size.parse::<f64>().ok())
        .sum();
    let headroom = (cfg.max_total_exposure_usdc - submitted_exposure).max(0.0);

    // 满足条件 = Pending 且 size ≥ min 且未过期 且 horizon OK
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
    // 最新优先 (FIFO)
    eligible.sort_by_key(|o| std::cmp::Reverse(o.created_at));

    let mut out = Vec::new();
    let mut used = 0.0;
    for o in eligible {
        if out.len() >= cfg.max_per_cycle {
            break;
        }
        let size: f64 = o.size.parse().unwrap_or(0.0);
        if used + size > headroom {
            continue; // 会突破上限;跳过但尝试下一个
        }
        out.push(o.id.clone());
        used += size;
    }
    out
}

/// 决定本周期要拒绝哪些 pending mirrors(及原因)。
/// 返回 (mirror_id, reason) 的 vec。
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

/// 为 audit log 美化打印时间戳。
pub fn fmt_ts(ms: i64) -> String {
    DateTime::<Utc>::from_timestamp_millis(ms)
        .map(|d| d.format("%Y-%m-%dT%H:%M:%SZ").to_string())
        .unwrap_or_else(|| format!("invalid_ts_{ms}"))
}

/// 运行一次性的 executor pass。纯函数——调用方负责 IO。
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
        // 第三个是 filled → 不计入
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
        // 现有敞口: 30 (一个 submitted)。Headroom: 20。
        let mut existing = mk_order("mx", "30", now() - 5000);
        existing.status = MirrorStatus::Submitted;
        let candidates = vec![
            mk_order("m1", "15", now() - 1000), // 能装下 (30+15=45)
            mk_order("m2", "25", now() - 2000), // 突破上限 (30+25=55)
        ];
        let mut orders = vec![existing];
        orders.extend(candidates);
        let closes = std::collections::HashMap::from([closes("m1", 24), closes("m2", 24)]);
        let picks = pick_next_mirror(&orders, &closes, now(), &cfg);
        assert_eq!(picks.len(), 1);
        // id 格式 = mir_{tx_hash}_{market} = mir_0xm1_tx_m1
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
        // id 格式 = mir_{event.tx_hash}_{market} = mir_0xsmall_tx_small
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

    // v0.44a — paper_mode 默认值 + env 覆盖。
    // 每个测试开始时清除 env var,这样
    // 测试顺序无所谓 (cargo 并行跑测试;
    // 不这么做的话,`paper_mode_1` 测试
    // 会把 env var 泄漏给同 binary 中的
    // 其他测试)。`serial_test` crate 是最干净的
    // 修复方案,但为避免新增 dep,我们把这些
    // 整合到一个测试中(组合成一个串行测试),
    // 一次性断言所有情况。
    #[test]
    fn executor_config_paper_mode_all_cases() {
        // 默认关闭
        std::env::remove_var("POLYROCKET_MIRROR_PAPER_MODE");
        let c = ExecutorConfig::default();
        assert!(!c.paper_mode);
        let c = ExecutorConfig::from_env();
        assert!(!c.paper_mode);

        // 1 = on
        std::env::set_var("POLYROCKET_MIRROR_PAPER_MODE", "1");
        let c = ExecutorConfig::from_env();
        assert!(c.paper_mode);

        // "TRUE" 大小写不敏感 = 开启
        std::env::set_var("POLYROCKET_MIRROR_PAPER_MODE", "TRUE");
        let c = ExecutorConfig::from_env();
        assert!(c.paper_mode);

        // 0 = 关闭
        std::env::set_var("POLYROCKET_MIRROR_PAPER_MODE", "0");
        let c = ExecutorConfig::from_env();
        assert!(!c.paper_mode);

        // 未设置 = 关闭
        std::env::remove_var("POLYROCKET_MIRROR_PAPER_MODE");
        let c = ExecutorConfig::from_env();
        assert!(!c.paper_mode);
    }
}
