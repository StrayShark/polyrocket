//! L2 — Scheduler manual triggers.
//!
//! Thin IPC layer over L4 `infra::scheduler` (the 3 background loops
//! run independently — these IPCs let the UI force a run).
//!
//! These IPCs let the UI force a health-probe sweep or daily-brief run
//! without waiting for the cron tick. Useful in three cases:
//!  1. user clicks "Refresh now" on LLM Management or Daily Brief page
//!  2. just-pasted LLM key, want immediate connectivity verification
//!  3. dev / QA: simulate a cron run

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
// ============== v0.48a — model degradation manual trigger ======
// =================================================================

/// v0.48a — args for the `degradation_check_now`
/// IPC. No fields today; the struct is a future
/// hook (e.g. for a custom sample size or
/// threshold override).
#[derive(Debug, Clone, Deserialize, Default)]
pub struct DegradationCheckNowArgs {}

/// v0.48a — manual trigger for the model
/// degradation check. Useful for the L1
/// "Check now" button on the ModelLab page or
/// after a big market sync. The actual telemetry
/// event is emitted by the underlying scheduler
/// helper.
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
// ============== v0.49c — scheduler self-test =====================
// =================================================================

/// v0.49c — return a snapshot of the 7 background
/// scheduler loops' liveness. Each loop's last
/// tick timestamp is captured in a process-global
/// atomic (see `infra::scheduler::self_test`).
///
/// The L1 Settings card renders this as a row of
/// green/red dots per loop. "Healthy" means the
/// loop ticked within 3x its expected interval.
/// If a loop has never ticked (process just
/// started and the stagger sleep hasn't elapsed),
/// it's marked unhealthy with `age_ms = None`.
#[tauri::command]
pub fn scheduler_self_test_now() -> SchedulerSelfTest {
    scheduler::self_test()
}
