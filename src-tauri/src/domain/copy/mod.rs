//! L3 — 跟单交易。
//!
//! 监听 `copy_targets`（巨鲸地址）的链上交易并将匹配
//! 记录到 `copy_events`。真实实现：依赖尚未实现的
//! `polymarket::fetch_trades_for_address`（按地址拉取交易）。
//!
//! **状态（v0.3c）：存根。** M7「Copy trading」里程碑。

use crate::domain::wallet::validate_address;
use crate::AppError;
use crate::AppResult;
use serde::{Deserialize, Serialize};

/// 一个被跟踪的「目标地址」（whale / trader）。`enabled = false` 时 `should_mirror`
/// 会跳过它。
///
/// **关键字段**：
///   - `min_edge` — model 算出的 edge 必须 ≥ 这个值才 mirror（过滤掉低确信度信号）
///   - `allocation_cap` — 单次 mirror 的最大 USDC（避免跟着 whale 把仓位下大）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopyTarget {
    pub id: String,
    pub address: String,
    pub label: Option<String>,
    pub enabled: bool,
    pub allocation_cap: Option<String>,
    pub min_edge: f64,
    pub created_at: i64,
}

/// 检测到的一次「目标地址成交」事件。Polymarket 链上 / 中心化 CLOB 数据源会
/// push 这种事件到 `copy_events` 表。
///
/// **dedup 字段**：`tx_hash` 唯一标识一次链上成交。同一个 tx 可能匹配多个
/// `copy_target`（多人跟同一个 whale），所以不是 unique on tx_hash。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopyEvent {
    pub id: i64,
    pub target_id: String,
    pub market_id: String,
    pub detected_at: i64,
    pub side: String,
    pub size: String,
    pub price: f64,
    pub tx_hash: String,
    pub matched_bet_id: Option<String>,
}

// ============================================================
// ============== 纯辅助函数 =================================
// ============================================================

/// 插入前校验 copy target。复用钱包地址校验器
///（EVM 0x + 40 位十六进制），因为 copy target 即钱包地址。
pub fn validate_target_args(address: &str, min_edge: Option<f64>, allocation_cap: Option<&str>) -> AppResult<()> {
    validate_address(address)?;
    if let Some(e) = min_edge {
        if !(0.0..=1.0).contains(&e) {
            return Err(AppError::Invalid(format!("min_edge {e} out of [0, 1]")));
        }
    }
    if let Some(c) = allocation_cap {
        let n: f64 = c.parse().map_err(|_| AppError::Invalid(format!("allocation_cap not a number: {c}")))?;
        if n < 0.0 {
            return Err(AppError::Invalid("allocation_cap cannot be negative".into()));
        }
    }
    Ok(())
}

/// 判断被观察地址的一笔成交是否应触发 mirror 订单。
/// 返回 mirror 方向（"YES"/"NO"）与金额；若 target 被禁用
/// 或 edge 太小则返回 None。
pub fn should_mirror(
    target: &CopyTarget,
    fill_side: &str,
    fill_size: &str,
    target_market_edge: f64,
) -> Option<MirrorDecision> {
    if !target.enabled {
        return None;
    }
    if target_market_edge.abs() < target.min_edge {
        return None;
    }
    let cap = target
        .allocation_cap
        .as_deref()
        .and_then(|c| c.parse::<f64>().ok())
        .unwrap_or(f64::INFINITY);
    let fill_size_n: f64 = fill_size.parse().unwrap_or(0.0);
    if fill_size_n <= 0.0 {
        return None;
    }
    let size = fill_size_n.min(cap);
    let mirror_side = if target_market_edge > 0.0 { "YES" } else { "NO" };
    Some(MirrorDecision {
        side: mirror_side.into(),
        size: size.to_string(),
        // 当 whale 方向与我们的 edge 不一致时仍 mirror
        //（即 model 看空而 whale 看多）
        flip: fill_side.to_uppercase() != mirror_side,
    })
}

/// `should_mirror` 的返回值。`flip = true` 表示 model 跟 whale 方向相反 —— 这种情况下
/// executor 仍然会下 mirror（按 model 自己的判断），但 L1 可以选择让用户在 Settings
/// 里关掉 "follow whale when disagreeing"。
///
/// **字段**：
///   - `side` — "YES" / "NO"（跟 model edge 方向一致）
///   - `size` — 已被 `allocation_cap` 截断后的 USDC
///   - `flip` — 当 fill_side 与 model side 不一致时为 true
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MirrorDecision {
    pub side: String,    // "YES" or "NO"
    pub size: String,    // decimal string
    pub flip: bool,      // 若模型方向与 whale 不一致则为 true
}

/// 检查 `tx_hash` 是否已出现在近期事件列表中（去重）。
pub fn is_duplicate_tx(events: &[CopyEvent], tx_hash: &str) -> bool {
    events.iter().any(|e| e.tx_hash == tx_hash)
}

// ============================================================
// ============== Mirror 队列状态机 ===============================
// ============================================================

/// 由 CopyEvent + market edge 派生的待执行 mirror 订单。
/// 持久化到 `copy_mirror_queue` 表；L2 调度器会选取
/// `Pending` 行并以 bet 形式提交。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MirrorStatus {
    /// 已被 should_mirror 决定；等待 executor 拉取
    Pending,
    /// 已作为 bet 提交；等待链上 fill 确认
    Submitted,
    /// 已成功 fill 并记录到 `bets`（matched_bet_id 已设置）
    Filled,
    /// 被 executor 拒绝（余额不足、网络错误等）
    Rejected,
    /// 已过期 —— 市场在 mirror 成交前已关盘
    Expired,
}

impl MirrorStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            MirrorStatus::Pending => "pending",
            MirrorStatus::Submitted => "submitted",
            MirrorStatus::Filled => "filled",
            MirrorStatus::Rejected => "rejected",
            MirrorStatus::Expired => "expired",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(MirrorStatus::Pending),
            "submitted" => Some(MirrorStatus::Submitted),
            "filled" => Some(MirrorStatus::Filled),
            "rejected" => Some(MirrorStatus::Rejected),
            "expired" => Some(MirrorStatus::Expired),
            _ => None,
        }
    }
    /// 合法的状态转移：Pending → (Submitted | Rejected | Expired)
    ///                Submitted → (Filled | Rejected)
    ///                （状态机合法转移路径）
    pub fn can_transition_to(self, next: MirrorStatus) -> bool {
        match (self, next) {
            (MirrorStatus::Pending, MirrorStatus::Submitted) => true,
            (MirrorStatus::Pending, MirrorStatus::Rejected) => true,
            (MirrorStatus::Pending, MirrorStatus::Expired) => true,
            (MirrorStatus::Submitted, MirrorStatus::Filled) => true,
            (MirrorStatus::Submitted, MirrorStatus::Rejected) => true,
            _ => false,
        }
    }
}

/// 根据决策与对应事件构建 MirrorOrder。
pub fn build_mirror(
    event: &CopyEvent,
    decision: &MirrorDecision,
    created_at: i64,
) -> MirrorOrder {
    MirrorOrder {
        id: format!("mir_{}_{}", event.tx_hash, event.market_id),
        event_id: event.id,
        target_id: event.target_id.clone(),
        market_id: event.market_id.clone(),
        side: decision.side.clone(),
        size: decision.size.clone(),
        flipped: decision.flip,
        status: MirrorStatus::Pending,
        created_at,
        submitted_at: None,
        filled_at: None,
        bet_id: None,
    }
}

/// 一个 mirror 订单的完整状态对象。`id = mir_{tx_hash}_{market_id}` 是 dedup key
/// —— 同一个 (whale_tx, market) 只 mirror 一次。
///
/// **生命周期**：`Pending` → executor pick up → `Submitted` → 等待 on-chain 确认
/// → `Filled`（成功）或 `Rejected`（失败）/ `Expired`（市场关闭前没填）。
///
/// **`flipped = true` 的语义**：model 跟 whale 方向相反但仍然 mirror；这条订单
/// 的 audit log 会有 `flip = true` 标记，方便事后分析 model 的独立判断质量。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MirrorOrder {
    pub id: String,
    pub event_id: i64,
    pub target_id: String,
    pub market_id: String,
    pub side: String,
    pub size: String,
    pub flipped: bool,
    pub status: MirrorStatus,
    pub created_at: i64,
    pub submitted_at: Option<i64>,
    pub filled_at: Option<i64>,
    pub bet_id: Option<String>,
}

/// Mirror 订单的统计聚合。L1 「Mirror」面板顶部用这个显示「Pending: 3, Filled: 12, Rejected: 2」。
///
/// **调用方**：`commands::mirror::list_mirrors` 调 `mirror_stats(orders)` 把 stats
/// 跟 `orders: Vec<MirrorOrder>` 一起返回。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MirrorStats {
    pub n_total: usize,
    pub n_pending: usize,
    pub n_submitted: usize,
    pub n_filled: usize,
    pub n_rejected: usize,
    pub n_expired: usize,
}

pub fn mirror_stats(orders: &[MirrorOrder]) -> MirrorStats {
    let mut s = MirrorStats {
        n_total: orders.len(),
        n_pending: 0,
        n_submitted: 0,
        n_filled: 0,
        n_rejected: 0,
        n_expired: 0,
    };
    for o in orders {
        match o.status {
            MirrorStatus::Pending => s.n_pending += 1,
            MirrorStatus::Submitted => s.n_submitted += 1,
            MirrorStatus::Filled => s.n_filled += 1,
            MirrorStatus::Rejected => s.n_rejected += 1,
            MirrorStatus::Expired => s.n_expired += 1,
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(enabled: bool, min_edge: f64, cap: Option<&str>) -> CopyTarget {
        CopyTarget {
            id: "t1".into(),
            address: "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045".into(),
            label: None,
            enabled,
            allocation_cap: cap.map(String::from),
            min_edge,
            created_at: 0,
        }
    }

    #[test]
    fn target_min_edge_defaults_to_zero() {
        let t = target(true, 0.0, None);
        assert!(t.enabled);
        assert_eq!(t.min_edge, 0.0);
    }

    #[test]
    fn validate_target_args_rejects_bad_address() {
        assert!(validate_target_args("not-an-address", None, None).is_err());
    }

    #[test]
    fn validate_target_args_rejects_bad_edge() {
        let addr = "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045";
        assert!(validate_target_args(addr, Some(1.5), None).is_err());
        assert!(validate_target_args(addr, Some(-0.1), None).is_err());
    }

    #[test]
    fn validate_target_args_rejects_bad_cap() {
        let addr = "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045";
        assert!(validate_target_args(addr, None, Some("not-a-number")).is_err());
        assert!(validate_target_args(addr, None, Some("-5")).is_err());
    }

    #[test]
    fn validate_target_args_ok() {
        let addr = "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045";
        assert!(validate_target_args(addr, Some(0.05), Some("100")).is_ok());
        assert!(validate_target_args(addr, None, None).is_ok());
    }

    #[test]
    fn mirror_disabled_returns_none() {
        let t = target(false, 0.05, None);
        assert!(should_mirror(&t, "YES", "100", 0.10).is_none());
    }

    #[test]
    fn mirror_edge_too_small() {
        let t = target(true, 0.05, None);
        assert!(should_mirror(&t, "YES", "100", 0.02).is_none());
    }

    #[test]
    fn mirror_respects_cap() {
        let t = target(true, 0.05, Some("50"));
        let m = should_mirror(&t, "YES", "100", 0.10).unwrap();
        assert_eq!(m.size, "50");
        assert!(!m.flip);
    }

    #[test]
    fn mirror_detects_direction_disagreement() {
        let t = target(true, 0.05, None);
        // Whale 买入 YES，但我们的 edge 为负（我们看 NO）
        let m = should_mirror(&t, "YES", "100", -0.10).unwrap();
        assert_eq!(m.side, "NO");
        assert!(m.flip);
    }

    #[test]
    fn mirror_zero_size_returns_none() {
        let t = target(true, 0.0, None);
        assert!(should_mirror(&t, "YES", "0", 0.10).is_none());
    }

    #[test]
    fn is_duplicate_tx_detects_match() {
        let events = vec![CopyEvent {
            id: 1,
            target_id: "t1".into(),
            market_id: "m1".into(),
            detected_at: 0,
            side: "YES".into(),
            size: "100".into(),
            price: 0.5,
            tx_hash: "0xabc".into(),
            matched_bet_id: None,
        }];
        assert!(is_duplicate_tx(&events, "0xabc"));
        assert!(!is_duplicate_tx(&events, "0xdef"));
    }

    fn ev(tx: &str) -> CopyEvent {
        CopyEvent {
            id: 42,
            target_id: "t1".into(),
            market_id: "m1".into(),
            detected_at: 0,
            side: "YES".into(),
            size: "100".into(),
            price: 0.5,
            tx_hash: tx.into(),
            matched_bet_id: None,
        }
    }

    #[test]
    fn mirror_status_round_trip() {
        for s in [
            MirrorStatus::Pending,
            MirrorStatus::Submitted,
            MirrorStatus::Filled,
            MirrorStatus::Rejected,
            MirrorStatus::Expired,
        ] {
            assert_eq!(MirrorStatus::parse(s.as_str()), Some(s));
        }
        assert_eq!(MirrorStatus::parse("bogus"), None);
    }

    #[test]
    fn mirror_status_legal_transitions() {
        assert!(MirrorStatus::Pending.can_transition_to(MirrorStatus::Submitted));
        assert!(MirrorStatus::Pending.can_transition_to(MirrorStatus::Rejected));
        assert!(MirrorStatus::Pending.can_transition_to(MirrorStatus::Expired));
        assert!(MirrorStatus::Submitted.can_transition_to(MirrorStatus::Filled));
        assert!(MirrorStatus::Submitted.can_transition_to(MirrorStatus::Rejected));
        // 非法转移
        assert!(!MirrorStatus::Filled.can_transition_to(MirrorStatus::Pending));
        assert!(!MirrorStatus::Expired.can_transition_to(MirrorStatus::Submitted));
        assert!(!MirrorStatus::Submitted.can_transition_to(MirrorStatus::Expired));
    }

    #[test]
    fn build_mirror_uses_event_data() {
        let d = MirrorDecision { side: "NO".into(), size: "50".into(), flip: true };
        let m = build_mirror(&ev("0xdeadbeef"), &d, 1700000000000);
        assert_eq!(m.id, "mir_0xdeadbeef_m1");
        assert_eq!(m.market_id, "m1");
        assert_eq!(m.side, "NO");
        assert_eq!(m.size, "50");
        assert!(m.flipped);
        assert_eq!(m.status, MirrorStatus::Pending);
        assert!(m.submitted_at.is_none());
        assert!(m.bet_id.is_none());
    }

    #[test]
    fn mirror_stats_aggregates() {
        let mut orders = Vec::new();
        for (i, s) in [
            MirrorStatus::Pending,
            MirrorStatus::Pending,
            MirrorStatus::Submitted,
            MirrorStatus::Filled,
            MirrorStatus::Filled,
            MirrorStatus::Filled,
            MirrorStatus::Rejected,
            MirrorStatus::Expired,
        ]
        .iter()
        .enumerate()
        {
            let d = MirrorDecision { side: "YES".into(), size: "10".into(), flip: false };
            let mut m = build_mirror(&ev(&format!("0x{:x}", i)), &d, 0);
            m.status = s.clone();
            orders.push(m);
        }
        let s = mirror_stats(&orders);
        assert_eq!(s.n_total, 8);
        assert_eq!(s.n_pending, 2);
        assert_eq!(s.n_submitted, 1);
        assert_eq!(s.n_filled, 3);
        assert_eq!(s.n_rejected, 1);
        assert_eq!(s.n_expired, 1);
    }
}
