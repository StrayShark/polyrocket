//! L2 — Audit log read-side (M9 + X1).
//!
//! Writes already happen in every other L2 command (see infra/error.rs
//! and the audit_log INSERTs scattered across the codebase). This module
//! is the read-side: list entries for the Audit page.

use crate::AppError;
use crate::AppResult;
use crate::infra::state::AppState;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use tauri::State;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AuditEntry {
    pub id: i64,
    pub at: i64,
    pub actor: String,
    pub action: String,
    pub target: Option<String>,
    pub payload: Option<String>,
    pub result: String,
}

#[derive(Debug, Deserialize)]
pub struct ListAuditArgs {
    pub limit: Option<i64>,
    pub action_prefix: Option<String>,
    pub actor: Option<String>,
}

/// List recent audit log entries, newest first.
#[tauri::command]
pub async fn list_audit_log(
    state: State<'_, AppState>,
    limit: Option<i64>,
) -> AppResult<Vec<AuditEntry>> {
    list_audit_log_impl(&state, ListAuditArgs { limit, action_prefix: None, actor: None }).await
}

pub async fn list_audit_log_impl(state: &AppState, args: ListAuditArgs) -> AppResult<Vec<AuditEntry>> {
    let limit = args.limit.unwrap_or(200).clamp(1, 1000);
    let rows: Vec<AuditEntry> = sqlx::query_as(
        "SELECT id, at, actor, action, target, payload, result
         FROM audit_log
         ORDER BY at DESC
         LIMIT ?",
    )
    .bind(limit)
    .fetch_all(&state.db)
    .await?;
    Ok(rows)
}

/// Count entries for an actor (used by tests / future dashboards).
#[tauri::command]
pub async fn audit_count_for_actor(
    state: State<'_, AppState>,
    actor: String,
) -> AppResult<i64> {
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM audit_log WHERE actor = ?")
        .bind(&actor)
        .fetch_one(&state.db)
        .await?;
    Ok(n)
}

/// v0.8c — Run a single audit-log retention purge. Returns the number
/// of rows deleted. Idempotent: a second call with the same clock
/// returns 0. The scheduler also runs this daily at 03:00 UTC.
#[tauri::command]
pub async fn purge_audit_log_now(state: State<'_, AppState>) -> AppResult<usize> {
    let n = crate::infra::scheduler::run_audit_purge_now(&state.db)
        .await
        .map_err(crate::AppError::Db)?;
    Ok(n)
}

/// v0.13c — return the user's current retention policy.
#[tauri::command]
pub async fn get_audit_retention(state: State<'_, AppState>) -> AppResult<AuditRetentionView> {
    let policy = crate::infra::scheduler::read_user_retention(&state.db).await?;
    Ok(AuditRetentionView::from(&policy))
}

/// v0.13c — set the user's retention policy. Triggers an immediate
/// purge so the new policy takes effect on the existing data.
#[tauri::command]
pub async fn set_audit_retention(
    state: State<'_, AppState>,
    args: SetAuditRetentionArgs,
) -> AppResult<usize> {
    let policy = crate::domain::audit::RetentionPolicy {
        retain_recent_ms: args.retain_recent_ms.unwrap_or(90 * 86_400_000),
        max_rows: args.max_rows.unwrap_or(50_000),
        min_keep_rows: args.min_keep_rows.unwrap_or(1_000),
    };
    crate::infra::scheduler::write_user_retention(&state.db, &policy).await?;
    // Apply immediately so the user sees the effect
    let n = crate::infra::scheduler::run_audit_purge_now(&state.db)
        .await
        .map_err(crate::AppError::Db)?;
    Ok(n)
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct AuditRetentionView {
    pub retain_recent_ms: i64,
    pub max_rows: i64,
    pub min_keep_rows: i64,
}

impl From<&crate::domain::audit::RetentionPolicy> for AuditRetentionView {
    fn from(p: &crate::domain::audit::RetentionPolicy) -> Self {
        Self {
            retain_recent_ms: p.retain_recent_ms,
            max_rows: p.max_rows,
            min_keep_rows: p.min_keep_rows,
        }
    }
}

#[derive(Debug, serde::Deserialize)]
pub struct SetAuditRetentionArgs {
    /// Defaults to 90 days.
    pub retain_recent_ms: Option<i64>,
    /// Defaults to 50,000.
    pub max_rows: Option<i64>,
    /// Defaults to 1,000.
    pub min_keep_rows: Option<i64>,
}

/// Look up the most recent N entries for a specific action.
pub async fn recent_for_action(
    state: &AppState,
    action: &str,
    limit: i64,
) -> AppResult<Vec<AuditEntry>> {
    let limit = limit.clamp(1, 1000);
    let rows: Vec<AuditEntry> = sqlx::query_as(
        "SELECT id, at, actor, action, target, payload, result
         FROM audit_log
         WHERE action = ?
         ORDER BY at DESC
         LIMIT ?",
    )
    .bind(action)
    .bind(limit)
    .fetch_all(&state.db)
    .await?;
    Ok(rows)
}

/// Filter helper used by both IPC and tests.
pub fn filter_entries(entries: Vec<AuditEntry>, args: ListAuditArgs) -> Vec<AuditEntry> {
    entries
        .into_iter()
        .filter(|e| {
            if let Some(prefix) = &args.action_prefix {
                if !e.action.starts_with(prefix) {
                    return false;
                }
            }
            if let Some(actor) = &args.actor {
                if e.actor != *actor {
                    return false;
                }
            }
            true
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_by_action_prefix() {
        let entries = vec![
            AuditEntry { id: 1, at: 0, actor: "user".into(), action: "bet.place".into(), target: None, payload: None, result: "ok".into() },
            AuditEntry { id: 2, at: 0, actor: "system".into(), action: "brief.refresh".into(), target: None, payload: None, result: "ok".into() },
            AuditEntry { id: 3, at: 0, actor: "user".into(), action: "wallet.add".into(), target: None, payload: None, result: "ok".into() },
        ];
        let args = ListAuditArgs { limit: None, action_prefix: Some("bet".into()), actor: None };
        let filtered = filter_entries(entries, args);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].action, "bet.place");
    }

    #[test]
    fn filter_by_actor() {
        let entries = vec![
            AuditEntry { id: 1, at: 0, actor: "user".into(), action: "bet.place".into(), target: None, payload: None, result: "ok".into() },
            AuditEntry { id: 2, at: 0, actor: "system".into(), action: "brief.refresh".into(), target: None, payload: None, result: "ok".into() },
        ];
        let args = ListAuditArgs { limit: None, action_prefix: None, actor: Some("system".into()) };
        let filtered = filter_entries(entries, args);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].actor, "system");
    }

    #[test]
    fn filter_combined() {
        let entries = vec![
            AuditEntry { id: 1, at: 0, actor: "user".into(), action: "bet.place".into(), target: None, payload: None, result: "ok".into() },
            AuditEntry { id: 2, at: 0, actor: "system".into(), action: "bet.settle".into(), target: None, payload: None, result: "ok".into() },
        ];
        let args = ListAuditArgs { limit: None, action_prefix: Some("bet".into()), actor: Some("user".into()) };
        let filtered = filter_entries(entries, args);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, 1);
    }
}
