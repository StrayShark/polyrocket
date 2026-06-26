//! L3 — 通知域（X2 governance）。
//!
//! 用于构造系统通知负载的纯辅助函数。
//! 实际的 OS 发送由 L2 命令（`commands::notify`）完成，
//! 它包装了 tauri_plugin_notification。
//!
//! 规范：docs/polyrocket-modules.md §3.X2

use serde::{Deserialize, Serialize};

/// 系统通知类型。Spec: docs/polyrocket-modules.md §3.X2
///
/// **9 种 kind**（+ 1 个 `Info` 兜底）：
///   - `NewSignal` / `OrderFill` / `MirrorDecision` — 业务事件
///   - `KeyringOk` / `KeyringError` — OS keyring 状态（用户首次授权后会通知）
///   - `ProviderAutoDisabled` — 3 次连续失败后 LLM provider 自动禁用
///   - `DailyBrief` — 每日简报
///   - `AutoPromote` — train 后自动 promote
///   - `Info` — 通用
///
/// **L1 怎么用**：每个 kind 在 L1 settings 都有独立开关（`setNotificationPref` IPC），
/// 默认开。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NotificationKind {
    /// 出现新的可执行信号
    NewSignal,
    /// 订单已成交
    OrderFill,
    /// Keyring 访问成功
    KeyringOk,
    /// Keyring 访问被拒绝/出错
    KeyringError,
    /// 连续失败 3 次后 Provider 被自动禁用
    ProviderAutoDisabled,
    /// 每日简报已刷新
    DailyBrief,
    /// 镜像决策（巨鲸成交,考虑了是否跟单）
    MirrorDecision,
    /// v0.39a —— train 完成后的后台自动 promote。
    /// L1 在 `auto_promote:finished` 事件触发且 promote
    /// 成功时调用。与 `Info` 区分是为了支持自定义
    /// OS 通知徽章。
    AutoPromote,
    /// 通用信息
    Info,
}

impl NotificationKind {
    pub fn as_str(self) -> &'static str {
        match self {
            NotificationKind::NewSignal => "new_signal",
            NotificationKind::OrderFill => "order_fill",
            NotificationKind::KeyringOk => "keyring_ok",
            NotificationKind::KeyringError => "keyring_error",
            NotificationKind::ProviderAutoDisabled => "provider_auto_disable",
            NotificationKind::DailyBrief => "daily_brief",
            NotificationKind::MirrorDecision => "mirror_decision",
            NotificationKind::AutoPromote => "auto_promote",
            NotificationKind::Info => "info",
        }
    }

    /// 稳定的默认标题（英文）。L1 可以通过 IPC 参数覆盖。
    pub fn default_title(self) -> &'static str {
        match self {
            NotificationKind::NewSignal => "New signal",
            NotificationKind::OrderFill => "Order filled",
            NotificationKind::KeyringOk => "Keyring access",
            NotificationKind::KeyringError => "Keyring error",
            NotificationKind::ProviderAutoDisabled => "Provider disabled",
            NotificationKind::DailyBrief => "Daily brief ready",
            NotificationKind::MirrorDecision => "Mirror decision",
            NotificationKind::AutoPromote => "Auto-promote",
            NotificationKind::Info => "polyrocket",
        }
    }
}

/// 单条通知请求的负载结构。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationPayload {
    pub kind: NotificationKind,
    pub title: String,
    pub body: String,
}

/// 为「新信号」构建负载，包含 edge + 市场信息。
pub fn new_signal_payload(market_id: &str, edge: f64, market_question: Option<&str>) -> NotificationPayload {
    let pct = edge * 100.0;
    let sign = if edge > 0.0 { "↑" } else { "↓" };
    let label = match market_question {
        Some(q) if q.len() <= 60 => q.to_string(),
        Some(q) => format!("{}…", &q[..60]),
        None => market_id.to_string(),
    };
    NotificationPayload {
        kind: NotificationKind::NewSignal,
        title: NotificationKind::NewSignal.default_title().to_string(),
        body: format!("{} {}{:.1}% on {}", sign, sign, pct.abs(), label),
    }
}

/// 为「订单成交」构建负载，包含 side + size。
pub fn order_fill_payload(market_id: &str, side: &str, size: &str, price: f64) -> NotificationPayload {
    NotificationPayload {
        kind: NotificationKind::OrderFill,
        title: NotificationKind::OrderFill.default_title().to_string(),
        body: format!("{} {} @ {:.3} ({} USDC)", side, market_id, price, size),
    }
}

/// 构建“keyring access denied”通知负载。
pub fn keyring_error_payload(alias: &str, message: &str) -> NotificationPayload {
    NotificationPayload {
        kind: NotificationKind::KeyringError,
        title: NotificationKind::KeyringError.default_title().to_string(),
        body: format!("{}: {}", alias, message),
    }
}

/// 为「provider 自动禁用」构建负载。
pub fn provider_disabled_payload(provider_id: &str, streak: i64) -> NotificationPayload {
    NotificationPayload {
        kind: NotificationKind::ProviderAutoDisabled,
        title: NotificationKind::ProviderAutoDisabled.default_title().to_string(),
        body: format!("{} disabled after {} consecutive probe failures", provider_id, streak),
    }
}

/// 为「mirror 决策」构建负载。
pub fn mirror_decision_payload(
    market_id: &str,
    side: &str,
    size: &str,
    flipped: bool,
) -> NotificationPayload {
    let flip_note = if flipped { " (flipped)" } else { "" };
    NotificationPayload {
        kind: NotificationKind::MirrorDecision,
        title: NotificationKind::MirrorDecision.default_title().to_string(),
        body: format!("{} {} {} USDC{}", market_id, side, size, flip_note),
    }
}

/// 根据用户偏好判断是否应发送通知。
/// `prefs_enabled` 是用户的全局开关；`kind_enabled` 是
/// 按类型划分的开关（由调用方从设置存储中解析）。
///
/// 当通知应被发出时返回 true。
pub fn should_send(prefs_enabled: bool, kind_enabled: bool) -> bool {
    prefs_enabled && kind_enabled
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_signal_title_positive_edge() {
        let p = new_signal_payload("m1", 0.12, Some("Will BTC hit 100k?"));
        assert_eq!(p.kind, NotificationKind::NewSignal);
        assert!(p.body.contains("↑"));
        assert!(p.body.contains("12.0%"));
    }

    #[test]
    fn new_signal_title_negative_edge() {
        let p = new_signal_payload("m1", -0.07, Some("Will SpaceX launch?"));
        assert!(p.body.contains("↓"));
        assert!(p.body.contains("7.0%"));
    }

    #[test]
    fn new_signal_truncates_long_question() {
        let long = "x".repeat(100);
        let p = new_signal_payload("m1", 0.05, Some(&long));
        assert!(p.body.contains('…'));
        assert!(p.body.len() < 100);
    }

    #[test]
    fn order_fill_includes_all_fields() {
        let p = order_fill_payload("m1", "YES", "50", 0.42);
        assert!(p.body.contains("YES"));
        assert!(p.body.contains("0.420"));
        assert!(p.body.contains("50"));
    }

    #[test]
    fn keyring_error_includes_alias_and_msg() {
        let p = keyring_error_payload("polyrocket/llm/openai/prod-1", "access denied");
        assert!(p.body.contains("polyrocket/llm/openai/prod-1"));
        assert!(p.body.contains("access denied"));
    }

    #[test]
    fn provider_disabled_includes_streak() {
        let p = provider_disabled_payload("openai", 3);
        assert!(p.body.contains("openai"));
        assert!(p.body.contains("3"));
    }

    #[test]
    fn mirror_decision_includes_flip_note_when_flipped() {
        let p = mirror_decision_payload("m1", "NO", "100", true);
        assert!(p.body.contains("(flipped)"));
    }

    #[test]
    fn mirror_decision_no_flip_note_when_aligned() {
        let p = mirror_decision_payload("m1", "YES", "50", false);
        assert!(!p.body.contains("(flipped)"));
    }

    #[test]
    fn should_send_requires_both_toggles() {
        assert!(!should_send(false, true));
        assert!(!should_send(true, false));
        assert!(should_send(true, true));
    }

    #[test]
    fn kind_as_str_all_unique() {
        let kinds = [
            NotificationKind::NewSignal,
            NotificationKind::OrderFill,
            NotificationKind::KeyringOk,
            NotificationKind::KeyringError,
            NotificationKind::ProviderAutoDisabled,
            NotificationKind::DailyBrief,
            NotificationKind::MirrorDecision,
            NotificationKind::Info,
        ];
        let mut strings: Vec<_> = kinds.iter().map(|k| k.as_str()).collect();
        strings.sort();
        strings.dedup();
        assert_eq!(strings.len(), kinds.len());
    }
}
