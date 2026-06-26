//! L2 —— 调度器手动触发。
//!
//! 位于 L4 `infra::scheduler` 之上的薄 IPC 层（3 个后台循环独立运行 ——
//! 这些 IPC 让 UI 可以强制触发一次运行）。
//!
//! 这些 IPC 让 UI 无需等待定时触发就能强制跑一次健康探测或日报生成。
//! 适用于以下三种场景：
//!  1. 用户在 LLM 管理或日报页面点击「立即刷新」
//!  2. 刚粘贴完 LLM key，希望立刻验证连通性
//!  3. 开发 / QA：模拟一次定时任务运行

use crate::AppResult;
use crate::infra::error::AppError;
use crate::infra::scheduler::{self, SchedulerSelfTest};
use crate::infra::state::AppState;
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Debug, Serialize)]
pub struct SchedulerStatus {
    pub health_probe_interval_sec: u64,
    pub daily_brief_hour_utc: u32,
    pub daily_brief_tz_offset_min: i32,
    pub anomaly_window_sec: u64,
    pub next_brief_run_at_unix_ms: i64,
}

#[tauri::command]
pub async fn scheduler_status(_state: State<'_, AppState>) -> AppResult<SchedulerStatus> {
    let cfg = crate::infra::scheduler::SchedulerConfig::from_env();
    let now_ms = chrono::Utc::now().timestamp_millis();
    let next = next_brief_unix_ms(cfg.daily_brief_hour_utc, cfg.daily_brief_tz_offset_min, now_ms);
    Ok(SchedulerStatus {
        health_probe_interval_sec: cfg.health_probe_interval.as_secs(),
        daily_brief_hour_utc: cfg.daily_brief_hour_utc,
        daily_brief_tz_offset_min: cfg.daily_brief_tz_offset_min,
        anomaly_window_sec: cfg.anomaly_window.as_secs(),
        next_brief_run_at_unix_ms: next,
    })
}

#[derive(Debug, Serialize)]
pub struct TriggerResult {
    pub triggered_at_unix_ms: i64,
    pub kind: String, // 'health_probe' | 'daily_brief'
    pub ok: bool,
    pub error: Option<String>,
}

#[tauri::command]
pub async fn scheduler_run_health_probe_now(state: State<'_, AppState>) -> AppResult<TriggerResult> {
    let http = crate::domain::llm::new_http_client();
    let res = crate::infra::scheduler::run_health_probe_now(&state.db, &http).await;
    Ok(TriggerResult {
        triggered_at_unix_ms: chrono::Utc::now().timestamp_millis(),
        kind: "health_probe".into(),
        ok: res.is_ok(),
        error: res.err().map(|e| e.to_string()),
    })
}

#[tauri::command]
pub async fn scheduler_run_daily_brief_now(state: State<'_, AppState>) -> AppResult<TriggerResult> {
    let res = crate::infra::scheduler::run_daily_brief_now(&state.db).await;
    Ok(TriggerResult {
        triggered_at_unix_ms: chrono::Utc::now().timestamp_millis(),
        kind: "daily_brief".into(),
        ok: res.is_ok(),
        error: res.err().map(|e| e.to_string()),
    })
}

fn next_brief_unix_ms(hour_utc: u32, tz_offset_min: i32, now_ms: i64) -> i64 {
    use chrono::{DateTime, Utc};
    let now = DateTime::<Utc>::from_timestamp_millis(now_ms).unwrap_or_else(Utc::now);
    let local = now + chrono::Duration::minutes(tz_offset_min as i64);
    let target_today = local
        .date_naive()
        .and_hms_opt(hour_utc, 0, 0)
        .map(|naive| naive.and_utc())
        .unwrap_or_else(Utc::now);
    let target = target_today - chrono::Duration::minutes(tz_offset_min as i64);
    let secs = if target > now {
        (target - now).num_seconds()
    } else {
        ((target + chrono::Duration::hours(24)) - now).num_seconds()
    };
    now_ms + (secs.max(0) * 1000)
}

// =================================================================
// ============== v0.48a —— 模型衰减手动触发 =================
// =================================================================

/// v0.48a —— `degradation_check_now` IPC 的入参。
/// 当前没有字段；该结构是预留扩展点（例如将来
/// 自定义样本量或阈值覆盖）。
#[derive(Debug, Clone, Deserialize, Default)]
pub struct DegradationCheckNowArgs {}

/// v0.48a —— 模型衰减检测的手动触发。可供 L1
/// ModelLab 页面的「立即检测」按钮使用，也可在
/// 完成大批量市场同步后调用。实际的遥测事件
/// 由底层的调度器辅助函数发出。
#[tauri::command]
pub async fn degradation_check_now(
    state: State<'_, AppState>,
    _args: DegradationCheckNowArgs,
) -> AppResult<()> {
    crate::infra::scheduler::run_degradation_check_now(&state.db).await
        .map_err(|e| AppError::Internal(format!("degradation check: {e}")))?;
    Ok(())
}

// =================================================================
// ============== v0.49c —— 调度器自检 ==========================
// =================================================================

/// v0.49c —— 返回 7 个后台调度循环活跃度的快照。
/// 每个循环上一次 tick 的时间戳记录在进程全局原子量中
/// （见 `infra::scheduler::self_test`）。
///
/// L1 设置页的卡片按循环渲染为一行绿 / 红圆点。
/// 「健康」意味着循环在预期间隔的 3 倍之内完成了 tick。
/// 若某个循环从未 tick（进程刚启动且启动错峰休眠
/// 尚未结束），则会标记为不健康且 `age_ms = None`。
#[tauri::command]
pub fn scheduler_self_test_now() -> SchedulerSelfTest {
    scheduler::self_test()
}
