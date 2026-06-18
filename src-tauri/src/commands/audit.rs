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

/// 审计日志单条记录。L1 「Settings → Audit」页面渲染。
///
/// **`payload` 是 JSON 字符串**（不是结构化字段）：每个 `action` 写时携带不同
/// context（lengths / pk_len / old_value 等），保持 schema-free 便于扩展。
/// **`result`**：通常 `"ok"` / `"error: <reason>"`。
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

/// IPC: `list_audit_log` —— 拉最近 N 条审计日志（最新在前）。
///
/// **`limit` clamp 到 [1, 1000]**：避免一次拉太多（>1000 影响 UI 渲染）。
///
/// **不带 filter**：filter 走 `list_audit_log_impl` 内部版本。IPC 是简单
/// 「最近 N 条」用；filter 走 `domain::audit` 调内部版本。
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

/// IPC: `audit_count_for_actor` —— 统计某个 actor 的审计条目数。
///
/// **典型用法**：
///   - 测试：验证某个 action 触发了预期数量的 audit 事件
///   - 未来的 L1 仪表盘：每个 actor（user / system / provider）的事件数
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
/// IPC: `purge_audit_log_now` —— 手动触发一次 audit log retention sweep。
///
/// **调用方**：L1 「Settings → Audit → Purge now」按钮（v0.42b+ 暴露）。
/// **走 `infra/scheduler::run_audit_purge_now`**：逻辑跟 scheduler 24h cron 一致，
/// 避免 IPC 和 cron 写两份清理代码。
#[tauri::command]
pub async fn purge_audit_log_now(state: State<'_, AppState>) -> AppResult<usize> {
    let n = crate::infra::scheduler::run_audit_purge_now(&state.db)
        .await
        .map_err(crate::AppError::Db)?;
    Ok(n)
}

/// IPC: `get_audit_retention` —— 读用户当前 retention 策略。
///
/// **返回**：`AuditRetentionView` 是 `domain::audit::RetentionPolicy` 的 view DTO
/// （同样的 3 个字段，但 L1 直接消费，不暴露 domain 类型）。
#[tauri::command]
pub async fn get_audit_retention(state: State<'_, AppState>) -> AppResult<AuditRetentionView> {
    let policy = crate::infra::scheduler::read_user_retention(&state.db).await?;
    Ok(AuditRetentionView::from(&policy))
}

/// IPC: `set_audit_retention` —— 写 retention 策略 + 立即生效。
///
/// **3 个字段的默认**：
///   - `retain_recent_ms` = 90 天
///   - `max_rows` = 50,000
///   - `min_keep_rows` = 1,000（safety floor：无论多激进，DB 至少留 1000 行）
///
/// **立即 purge**：写完立刻 `run_audit_purge_now` 跑一次，让用户看到效果
/// （L1 弹「已清理 N 条」toast）。Scheduler 下一个 tick 看到新策略不会再动。
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

/// 内部 helper：拉某个 action 最近 N 条 entry。**非** IPC 入口 —— 只给同包
/// 的其他 command 用（例如 `anomaly_loop` 查 `anomaly.detected` 最近事件）。
///
/// **`limit` clamp 到 [1, 1000]**：同 `list_audit_log`。
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
/// L1 filter 入口的应用层版本。`list_audit_log_impl` SQL 没做 filter，filter
/// 在应用层做（方便 IPC 流式 + 减少 SQL 复杂度）。
///
/// **支持 2 个 filter**：
///   - `action_prefix` —— `action LIKE 'prefix%'`（如 `"pm."` 过滤 PM 相关）
///   - `actor` —— 精确匹配 `actor` 列
///
/// **未实现**：`target` / `result` 过滤 —— 留 v0.62+ 扩展。
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
