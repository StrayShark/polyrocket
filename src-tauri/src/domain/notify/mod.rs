//! L3 — Notification domain (X2 governance).
//!
//! Pure helpers for shaping the system-notification payload.
//! The actual OS send is done by L2 commands (commands::notify) which
//! wraps tauri_plugin_notification.
//!
//! Spec: docs/polyrocket-modules.md §3.X2

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
    /// New actionable signal appeared
    NewSignal,
    /// An order was filled
    OrderFill,
    /// Keyring access succeeded
    KeyringOk,
    /// Keyring access denied / errored
    KeyringError,
    /// Provider auto-disabled after 3 consecutive failures
    ProviderAutoDisabled,
    /// Daily brief refreshed
    DailyBrief,
    /// Mirror decided (whale filled, we considered copying)
    MirrorDecision,
    /// v0.39a — background auto-promote after train
    /// completed. The L1 calls this when
    /// `auto_promote:finished` event fires and the
    /// promote succeeded. Distinct from `Info` so
    /// the OS notification badge can be customized.
    AutoPromote,
    /// Generic info
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

    /// Stable default title (English). L1 may override via ipc arg.
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

/// Payload shape for one notification request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationPayload {
    pub kind: NotificationKind,
    pub title: String,
    pub body: String,
}

/// Build a payload for "new signal" with edge + market info.
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

/// Build a payload for "order filled" with side + size.
pub fn order_fill_payload(market_id: &str, side: &str, size: &str, price: f64) -> NotificationPayload {
    NotificationPayload {
        kind: NotificationKind::OrderFill,
        title: NotificationKind::OrderFill.default_title().to_string(),
        body: format!("{} {} @ {:.3} ({} USDC)", side, market_id, price, size),
    }
}

/// Build a payload for "keyring access denied".
pub fn keyring_error_payload(alias: &str, message: &str) -> NotificationPayload {
    NotificationPayload {
        kind: NotificationKind::KeyringError,
        title: NotificationKind::KeyringError.default_title().to_string(),
        body: format!("{}: {}", alias, message),
    }
}

/// Build a payload for "provider auto-disabled".
pub fn provider_disabled_payload(provider_id: &str, streak: i64) -> NotificationPayload {
    NotificationPayload {
        kind: NotificationKind::ProviderAutoDisabled,
        title: NotificationKind::ProviderAutoDisabled.default_title().to_string(),
        body: format!("{} disabled after {} consecutive probe failures", provider_id, streak),
    }
}

/// Build a payload for "mirror decision".
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

/// Decide whether a notification should be sent given user prefs.
/// `prefs_enabled` is the user's global toggle; `kind_enabled` is the
/// per-kind toggle (caller resolves from the settings store).
///
/// Returns true if the notification should be emitted.
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
