//! L2 — System notifications (X2 governance).
//!
//! Wraps tauri_plugin_notification. Pure helper functions live in
//! domain::notify (NotificationPayload builders); this module handles
//! the actual OS send + permission request.

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
    /// Whether the user has globally enabled notifications
    /// (L1 sends this from prefs-store; default true).
    #[serde(default = "default_true")]
    pub prefs_enabled: bool,
}

fn default_true() -> bool { true }

/// Send a system notification. Returns 1 if the OS accepted the
/// request, 0 if disabled by prefs.
#[tauri::command]
pub async fn send_notification(
    app: AppHandle,
    args: SendNotificationArgs,
) -> AppResult<u32> {
    if !args.prefs_enabled {
        return Ok(0);
    }
    // Parse kind (best-effort; falls back to Info)
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

/// Request OS permission for notifications. macOS / iOS require this
/// before any notification can be sent.
#[tauri::command]
pub async fn request_notification_permission(app: AppHandle) -> AppResult<bool> {
    let granted = app
        .notification()
        .request_permission()
        .map_err(|e| crate::AppError::Internal(format!("permission request: {e}")))?;
    Ok(granted == tauri_plugin_notification::PermissionState::Granted)
}

/// Check current notification permission state.
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
    /// Lenient parser used by IPC — falls back to Info on unknown.
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

// Re-export the payload struct for L1 ipc.ts
pub use crate::domain::notify::NotificationPayload as OutgoingNotification;
