//! L2 —— 系统通知（X2 治理）。
//!
//! 封装 tauri_plugin_notification。domain::notify 中放置纯辅助函数
//! （NotificationPayload 构建器）；本模块处理实际的系统通知发送 + 权限申请。

use crate::AppResult;
use crate::domain::notify::{NotificationKind, NotificationPayload};
use serde::Deserialize;
use tauri::AppHandle;
use tauri_plugin_notification::NotificationExt;

#[derive(Debug, Deserialize)]
pub struct SendNotificationArgs {
    pub kind: String,
    pub title: String,
    pub body: String,
    /// 用户是否全局开启了通知（L1 从 prefs-store 传过来；默认 true）。
    #[serde(default = "default_true")]
    pub prefs_enabled: bool,
}

fn default_true() -> bool { true }

/// 发送一条系统通知。操作系统接受则返回 1，因偏好设置被禁用则返回 0。
#[tauri::command]
pub async fn send_notification(
    app: AppHandle,
    args: SendNotificationArgs,
) -> AppResult<u32> {
    if !args.prefs_enabled {
        return Ok(0);
    }
    // 解析 kind（尽力解析；无法识别时回退到 Info）
    let _kind: NotificationKind = NotificationKind::new_from_str(&args.kind)
        .unwrap_or(NotificationKind::Info);

    app.notification()
        .builder()
        .title(args.title)
        .body(args.body)
        .show()
        .map_err(|e| crate::AppError::Internal(format!("notification send: {e}")))?;

    Ok(1)
}

/// 向操作系统申请通知权限。macOS / iOS 在发送任何通知之前都需要这一步。
#[tauri::command]
pub async fn request_notification_permission(app: AppHandle) -> AppResult<bool> {
    let granted = app
        .notification()
        .request_permission()
        .map_err(|e| crate::AppError::Internal(format!("permission request: {e}")))?;
    Ok(granted == tauri_plugin_notification::PermissionState::Granted)
}

/// 查询当前通知权限状态。
#[tauri::command]
pub async fn notification_permission_state(app: AppHandle) -> AppResult<String> {
    let state = app
        .notification()
        .permission_state()
        .map_err(|e| crate::AppError::Internal(format!("permission state: {e}")))?;
    use tauri_plugin_notification::PermissionState::*;
    Ok(match state {
        Granted => "granted",
        Denied => "denied",
        _ => "default",
    }
    .to_string())
}

impl NotificationKind {
    /// IPC 使用的宽松解析器 —— 无法识别时回退到 Info。
    pub fn new_from_str(s: &str) -> Option<Self> {
        match s {
            "new_signal" => Some(NotificationKind::NewSignal),
            "order_fill" => Some(NotificationKind::OrderFill),
            "keyring_ok" => Some(NotificationKind::KeyringOk),
            "keyring_error" => Some(NotificationKind::KeyringError),
            "provider_auto_disable" => Some(NotificationKind::ProviderAutoDisabled),
            "daily_brief" => Some(NotificationKind::DailyBrief),
            "mirror_decision" => Some(NotificationKind::MirrorDecision),
            "auto_promote" => Some(NotificationKind::AutoPromote),
            "info" => Some(NotificationKind::Info),
            _ => None,
        }
    }
}

// 为 L1 的 ipc.ts 重新导出 payload 结构体
pub use crate::domain::notify::NotificationPayload as OutgoingNotification;
