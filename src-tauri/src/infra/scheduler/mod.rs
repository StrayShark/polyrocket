//! L4 — Background schedulers — health probe + daily brief cron + anomaly detect.
//!
//! All three run as `tokio::spawn` tasks started in `lib.rs::run()` setup.
//! Each task is self-contained and recovers from DB / network errors
//! without crashing the process.
//!
//! Lifecycle
//! ---------
//! - `start(pool, http)` returns a [`SchedulerHandle`] that the caller
//!   can hold. Dropping the handle does NOT stop the tasks (they run
//!   for the process lifetime) — instead it provides a `shutdown()`
//!   signal for tests.
//!
//! Tunables (env vars)
//! -------------------
//! - `POLYROCKET_HEALTH_PROBE_INTERVAL_MIN` — default 5
//! - `POLYROCKET_DAILY_BRIEF_HOUR_UTC`      — default 0
//! - `POLYROCKET_DAILY_BRIEF_TZ_OFFSET_MIN` — default 0 (UTC)
//! - `POLYROCKET_ANOMALY_WINDOW_MIN`        — default 60
//! - `POLYROCKET_TELEMETRY`                 — when 1, also publishes
//!   in-process events (future: Sentry).
//!
//! Layer rules: this module depends on L3 (`domain::llm`) and L5
//! (`platform::keyring`) — it MUST NOT depend on L1 or L2.

use crate::platform::env::{env_u32, env_u64, env_i32};
use crate::platform::keyring;
use crate::domain::llm::{
    AnthropicClient, CostRate, CustomClient, DeepSeekClient, GoogleClient, LlmClient,
    OpenAIClient, ProviderKind,
};
use sqlx::SqlitePool;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Notify;

const DEFAULT_HEALTH_PROBE_MIN: u64 = 5;
const DEFAULT_BRIEF_HOUR_UTC: u32 = 0;
const DEFAULT_BRIEF_TZ_OFFSET_MIN: i32 = 0;
const DEFAULT_ANOMALY_WINDOW_MIN: u64 = 60;
const DEFAULT_MIRROR_TICK_SEC: u64 = 30;

#[derive(Clone)]
pub struct SchedulerConfig {
    pub health_probe_interval: Duration,
    pub daily_brief_hour_utc: u32,
    pub daily_brief_tz_offset_min: i32,
    pub anomaly_window: Duration,
    /// v0.6a — mirror executor tick interval
    pub mirror_tick: Duration,
}

impl SchedulerConfig {
    pub fn from_env() -> Self {
        Self {
            health_probe_interval: Duration::from_secs(
                60 * env_u64("POLYROCKET_HEALTH_PROBE_INTERVAL_MIN", DEFAULT_HEALTH_PROBE_MIN),
            ),
            daily_brief_hour_utc: env_u32("POLYROCKET_DAILY_BRIEF_HOUR_UTC", DEFAULT_BRIEF_HOUR_UTC),
            daily_brief_tz_offset_min: env_i32(
                "POLYROCKET_DAILY_BRIEF_TZ_OFFSET_MIN",
                DEFAULT_BRIEF_TZ_OFFSET_MIN,
            ),
            anomaly_window: Duration::from_secs(
                60 * env_u64("POLYROCKET_ANOMALY_WINDOW_MIN", DEFAULT_ANOMALY_WINDOW_MIN),
            ),
            mirror_tick: Duration::from_secs(
                env_u64("POLYROCKET_MIRROR_TICK_SEC", DEFAULT_MIRROR_TICK_SEC),
            ),
        }
    }
}

pub struct SchedulerHandle {
    pub shutdown: Arc<Notify>,
}

impl SchedulerHandle {
    pub fn shutdown(&self) {
        self.shutdown.notify_waiters();
    }
}

/// Start all background schedulers. Returns a handle the caller can
/// use to signal shutdown (currently only for tests).
pub fn start(pool: SqlitePool, http: reqwest::Client) -> SchedulerHandle {
    let cfg = SchedulerConfig::from_env();
    let shutdown = Arc::new(Notify::new());

    {
        let pool = pool.clone();
        let http = http.clone();
        let cfg = cfg.clone();
        let shutdown = shutdown.clone();
        tokio::spawn(async move {
            run_health_probe_loop(pool, http, cfg, shutdown).await;
        });
    }
    {
        let pool = pool.clone();
        let cfg = cfg.clone();
        let shutdown = shutdown.clone();
        tokio::spawn(async move {
            run_daily_brief_loop(pool, cfg, shutdown).await;
        });
    }
    {
        let pool = pool.clone();
        let cfg = cfg.clone();
        let shutdown = shutdown.clone();
        let http = http.clone();
        tokio::spawn(async move {
            run_anomaly_loop(pool, http, cfg, shutdown).await;
        });
    }
    {
        // v0.6a — M5 auto-execution: poll mirror queue, submit pending as Mode B bets
        let pool = pool.clone();
        let cfg = cfg.clone();
        let shutdown = shutdown.clone();
        tokio::spawn(async move {
            run_mirror_executor_loop(pool, cfg, shutdown).await;
        });
    }
    {
        // v0.8c — audit log retention: purge old rows daily
        let pool = pool.clone();
        let cfg = cfg.clone();
        let shutdown = shutdown.clone();
        tokio::spawn(async move {
            run_audit_purge_loop(pool, cfg, shutdown).await;
        });
    }
    {
        // v0.10d — sidecar health probe: ping every 30s, write a
        // row to sidecar_health, and purge the table hourly.
        let pool = pool.clone();
        let cfg = cfg.clone();
        let shutdown = shutdown.clone();
        tokio::spawn(async move {
            run_sidecar_health_loop(pool, cfg, shutdown).await;
        });
    }

    tracing::info!(
        "scheduler started — health_probe={}min, brief_hour_utc={}, brief_tz_offset={}min, anomaly_window={}min",
        cfg.health_probe_interval.as_secs() / 60,
        cfg.daily_brief_hour_utc,
        cfg.daily_brief_tz_offset_min,
        cfg.anomaly_window.as_secs() / 60,
    );
    SchedulerHandle { shutdown }
}

// =================================================================
// ============== Health probe loop ===============================
// =================================================================

async fn run_health_probe_loop(
    pool: SqlitePool,
    http: reqwest::Client,
    cfg: SchedulerConfig,
    shutdown: Arc<Notify>,
) {
    // Stagger initial run to avoid thundering herd on cold start
    tokio::time::sleep(Duration::from_secs(5)).await;
    let mut ticker = tokio::time::interval(cfg.health_probe_interval);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            _ = ticker.tick() => {
                if let Err(e) = probe_all_providers(&pool, &http).await {
                    tracing::warn!("health probe sweep error: {e}");
                }
            }
            _ = shutdown.notified() => {
                tracing::info!("health probe loop: shutdown signal");
                return;
            }
        }
    }
}

async fn probe_all_providers(pool: &SqlitePool, http: &reqwest::Client) -> sqlx::Result<()> {
    // Pull enabled providers
    let providers: Vec<ProviderProbeRow> = sqlx::query_as(
        "SELECT id, display_name, api_base, default_model, timeout_ms, cost_per_1k_in, cost_per_1k_out, max_retries, key_alias
         FROM llm_providers WHERE enabled = 1",
    )
    .fetch_all(pool)
    .await?;
    tracing::debug!("health probe: {} enabled providers", providers.len());

    for p in providers {
        // Run probe in a separate task so one slow provider doesn't block others
        let pool2 = pool.clone();
        let http2 = http.clone();
        tokio::spawn(async move {
            if let Err(e) = probe_one_provider(&pool2, &http2, &p).await {
                tracing::warn!("probe for {} failed: {e}", p.id);
            }
        });
    }
    Ok(())
}

#[derive(sqlx::FromRow, Clone)]
struct ProviderProbeRow {
    id: String,
    display_name: String,
    api_base: Option<String>,
    default_model: String,
    timeout_ms: i64,
    cost_per_1k_in: Option<f64>,
    cost_per_1k_out: Option<f64>,
    max_retries: i64,
    key_alias: String,
}

async fn probe_one_provider(
    pool: &SqlitePool,
    http: &reqwest::Client,
    p: &ProviderProbeRow,
) -> sqlx::Result<()> {
    use crate::domain::llm::KeyHandle;
    let keys: Vec<KeyHandle> = {
        let rows: Vec<(String, String, String)> = sqlx::query_as(
            "SELECT id, alias, keyring_alias
             FROM llm_provider_keys
             WHERE provider_id = ? AND enabled = 1
             ORDER BY priority ASC LIMIT 3",
        )
        .bind(&p.id)
        .fetch_all(pool)
        .await?;
        rows.into_iter()
            .map(|(id, alias, keyring_alias)| KeyHandle { id, alias, keyring_alias })
            .collect()
    };
    if keys.is_empty() {
        tracing::debug!("probe: {} no keys configured, skip", p.id);
        return Ok(());
    }
    let secret = match keyring::get_key(&keys[0].keyring_alias) {
        Ok(s) => s,
        Err(_) => {
            // No secret — record failing probe and update health
            let _ = insert_health_check(
                pool, &p.id, Some(&keys[0].id), false, None, None,
                Some("auth".to_string()), Some("no keyring entry".to_string()),
            ).await;
            let _ = update_provider_health(pool, &p.id, false, 0, Some("no keyring entry")).await;
            return Ok(());
        }
    };
    let kind = provider_kind_from_id(&p.id);
    let client: Arc<dyn LlmClient> = match kind {
        ProviderKind::Openai => Arc::new(OpenAIClient::new(
            p.api_base.clone().unwrap_or_else(|| "https://api.openai.com/v1".into()),
        )),
        ProviderKind::Anthropic => Arc::new(AnthropicClient::with_base(
            p.api_base.clone().unwrap_or_else(|| "https://api.anthropic.com".into()),
        )),
        ProviderKind::Google => Arc::new(GoogleClient::with_base(
            p.api_base
                .clone()
                .unwrap_or_else(|| "https://generativelanguage.googleapis.com/v1beta".into()),
        )),
        ProviderKind::Deepseek => Arc::new(DeepSeekClient::new()),
        ProviderKind::OpenaiCompat | ProviderKind::AnthropicCompat => {
            let base = p
                .api_base
                .clone()
                .unwrap_or_else(|| "https://api.openai.com/v1".into());
            if matches!(kind, ProviderKind::AnthropicCompat) {
                Arc::new(CustomClient::new_anthropic_compat(base, &p.default_model))
            } else {
                Arc::new(CustomClient::new_openai_compat(base, &p.default_model))
            }
        }
    };
    let req = crate::domain::llm::CallRequest::new(&p.default_model)
        .max_tokens(1)
        .temperature(0.0);
    let cost = CostRate {
        per_1k_in_cents: p.cost_per_1k_in.unwrap_or(0.0),
        per_1k_out_cents: p.cost_per_1k_out.unwrap_or(0.0),
    };
    let started = std::time::Instant::now();
    let outcome = client.call(http, &secret, &req, cost).await;
    let latency = started.elapsed().as_millis() as u64;
    match outcome {
        Ok(oc) => {
            let status = Some(oc.http_status as i64);
            let _ = insert_health_check(
                pool, &p.id, Some(&keys[0].id), true,
                Some(latency), status, None, None,
            ).await;
            let _ = update_provider_health(pool, &p.id, true, latency, None).await;
        }
        Err(e) => {
            let _ = insert_health_check(
                pool, &p.id, Some(&keys[0].id), false,
                Some(latency), e.http_status.map(|s| s as i64),
                Some(e.code.to_string()), Some(e.message.clone()),
            ).await;
            let _ = update_provider_health(pool, &p.id, false, latency, Some(&e.message)).await;
            // Check 3-fail streak → auto-disable
            let _ = maybe_auto_disable(pool, &p.id).await;
        }
    }
    Ok(())
}

fn provider_kind_from_id(id: &str) -> ProviderKind {
    match id {
        "openai" => ProviderKind::Openai,
        "anthropic" => ProviderKind::Anthropic,
        "google" => ProviderKind::Google,
        "deepseek" => ProviderKind::Deepseek,
        "openai_compat" | "custom" => ProviderKind::OpenaiCompat,
        "anthropic_compat" => ProviderKind::AnthropicCompat,
        _ => ProviderKind::Openai,
    }
}

async fn insert_health_check(
    pool: &SqlitePool,
    provider_id: &str,
    key_id: Option<&str>,
    success: bool,
    latency_ms: Option<u64>,
    http_status: Option<i64>,
    error_code: Option<String>,
    error_message: Option<String>,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO llm_health_checks
            (provider_id, key_id, checked_at, trigger, success, latency_ms, http_status, error_code, error_message, model_used)
         VALUES (?, ?, unixepoch() * 1000, 'background', ?, ?, ?, ?, ?, NULL)",
    )
    .bind(provider_id)
    .bind(key_id)
    .bind(success)
    .bind(latency_ms.map(|n| n as i64))
    .bind(http_status)
    .bind(error_code)
    .bind(error_message)
    .execute(pool)
    .await?;
    Ok(())
}

async fn update_provider_health(
    pool: &SqlitePool,
    provider_id: &str,
    success: bool,
    latency_ms: u64,
    last_error: Option<&str>,
) -> sqlx::Result<()> {
    let status = if !success { "failing" } else if latency_ms > 3_000 { "slow" } else { "ok" };
    let last_error_owned: Option<String> = if success { None } else { last_error.map(|s| s.to_string()) };
    sqlx::query(
        "UPDATE llm_providers SET
            health_status = ?,
            last_health_check_at = unixepoch() * 1000,
            health_latency_p50_ms = CASE WHEN ? = 1 THEN
                COALESCE(CAST((COALESCE(health_latency_p50_ms, 0) * 4 + ?) / 5 AS INTEGER), ?)
            ELSE health_latency_p50_ms END,
            last_health_error = COALESCE(?, last_health_error)
         WHERE id = ?",
    )
    .bind(status)
    .bind(if success { 1_i64 } else { 0 })
    .bind(latency_ms as i64)
    .bind(latency_ms as i64)
    .bind(last_error_owned)
    .bind(provider_id)
    .execute(pool)
    .await?;
    Ok(())
}

async fn maybe_auto_disable(pool: &SqlitePool, provider_id: &str) -> sqlx::Result<()> {
    // 3 consecutive failures → auto-disable
    let streak: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM (
            SELECT success FROM llm_health_checks
            WHERE provider_id = ?
            ORDER BY checked_at DESC LIMIT 3
         ) WHERE success = 0",
    )
    .bind(provider_id)
    .fetch_one(pool)
    .await?;
    if streak >= 3 {
        sqlx::query("UPDATE llm_providers SET enabled = 0 WHERE id = ? AND enabled = 1")
            .bind(provider_id)
            .execute(pool)
            .await?;
        sqlx::query(
            "INSERT INTO audit_log (actor, action, target, payload, result) VALUES ('system', 'llm.provider.auto_disable', ?, ?, 'ok')",
        )
        .bind(provider_id)
        .bind(serde_json::json!({"reason": "3 consecutive probe failures", "streak": streak}))
        .execute(pool)
        .await?;
        tracing::warn!("auto-disabled provider {provider_id} after {streak} consecutive probe failures");
    }
    Ok(())
}

// =================================================================
// ============== Daily brief cron ================================
// =================================================================

async fn run_daily_brief_loop(
    pool: SqlitePool,
    cfg: SchedulerConfig,
    shutdown: Arc<Notify>,
) {
    // Wait until next 00:00 UTC (adjusted by tz offset) then tick daily
    let initial = seconds_until_next_brief(cfg.daily_brief_hour_utc, cfg.daily_brief_tz_offset_min);
    tracing::info!("daily brief: first run in {}s", initial.as_secs());
    tokio::select! {
        _ = tokio::time::sleep(initial) => {}
        _ = shutdown.notified() => return,
    }
    loop {
        if let Err(e) = run_daily_brief_once(&pool).await {
            tracing::warn!("daily brief job error: {e}");
        }
        let dur = Duration::from_secs(24 * 3600);
        tokio::select! {
            _ = tokio::time::sleep(dur) => {}
            _ = shutdown.notified() => return,
        }
    }
}

fn seconds_until_next_brief(hour_utc: u32, tz_offset_min: i32) -> Duration {
    use chrono::{DateTime, TimeZone, Utc};
    let now = Utc::now();
    // Apply tz offset to "wall clock" computation, then back to UTC instant.
    let local = now + chrono::Duration::minutes(tz_offset_min as i64);
    let target_today = local
        .date_naive()
        .and_hms_opt(hour_utc, 0, 0)
        .map(|naive| naive.and_utc())
        .unwrap_or_else(|| Utc::now());
    let target = target_today - chrono::Duration::minutes(tz_offset_min as i64);
    let secs = if target > now {
        (target - now).num_seconds().max(1)
    } else {
        // already past today's target; schedule for tomorrow
        ((target + chrono::Duration::hours(24)) - now).num_seconds().max(1)
    };
    Duration::from_secs(secs as u64)
}

async fn run_daily_brief_once(pool: &SqlitePool) -> sqlx::Result<()> {
    let started = chrono::Utc::now().timestamp_millis();
    tracing::info!("daily brief job: starting");

    // Lazy init: ensure today's row exists. We compute top-N with a simple
    // heuristic — for v0.2 we just count active markets and write a stub.
    // v0.3+ will plug in the full scoring formula from M12.
    let n_markets: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM markets WHERE closes_at > unixepoch() * 1000",
    )
    .fetch_one(pool)
    .await?;
    let top_n: i64 = std::env::var("DAILY_BRIEF_TOP_N").ok().and_then(|v| v.parse().ok()).unwrap_or(8);
    let expires_at = started + 24 * 3600 * 1000;

    let today = chrono::Utc::now().date_naive().format("%Y-%m-%d").to_string();
    sqlx::query(
        "INSERT INTO daily_briefs (brief_date, generated_at, top_n, items_json, expires_at, triggered_by, status)
         VALUES (?, ?, ?, ?, ?, 'cron', 'ok')
         ON CONFLICT(brief_date) DO UPDATE SET
            generated_at = excluded.generated_at,
            top_n = excluded.top_n,
            items_json = excluded.items_json,
            expires_at = excluded.expires_at,
            triggered_by = 'cron',
            status = 'ok'",
    )
    .bind(&today)
    .bind(started)
    .bind(top_n)
    .bind(serde_json::json!({
        "note": "v0.2 stub — top N by recency; v0.3 uses M12 scoring",
        "n_active_markets": n_markets,
    }))
    .bind(expires_at)
    .execute(pool)
    .await?;

    sqlx::query(
        "INSERT INTO audit_log (actor, action, target, payload, result) VALUES ('system', 'brief.refresh', 'daily', ?, 'ok')",
    )
    .bind(serde_json::json!({"trigger": "cron", "n_markets": n_markets, "top_n": top_n, "date": today}))
    .execute(pool)
    .await?;
    tracing::info!("daily brief job: done (n_markets={}, top_n={})", n_markets, top_n);
    Ok(())
}

// =================================================================
// ============== Anomaly detector loop ============================
// =================================================================

async fn run_anomaly_loop(
    pool: SqlitePool,
    _http: reqwest::Client,
    cfg: SchedulerConfig,
    shutdown: Arc<Notify>,
) {
    // Stagger 30s after startup so first probe has data
    tokio::time::sleep(Duration::from_secs(30)).await;
    let mut ticker = tokio::time::interval(cfg.anomaly_window);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            _ = ticker.tick() => {
                if let Err(e) = detect_anomalies(&pool, cfg.anomaly_window).await {
                    tracing::warn!("anomaly detection error: {e}");
                }
            }
            _ = shutdown.notified() => {
                tracing::info!("anomaly loop: shutdown signal");
                return;
            }
        }
    }
}

// =================================================================
// ============== Mirror executor loop (v0.6a) =====================
// =================================================================

async fn run_mirror_executor_loop(pool: SqlitePool, cfg: SchedulerConfig, shutdown: Arc<Notify>) {
    // Stagger 10s after startup so other loops settle first
    tokio::time::sleep(Duration::from_secs(10)).await;
    let mut ticker = tokio::time::interval(cfg.mirror_tick);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let exec_cfg = crate::domain::mirror::ExecutorConfig::from_env();
    loop {
        tokio::select! {
            _ = ticker.tick() => {
                if let Err(e) = run_mirror_pass(&pool, &exec_cfg).await {
                    tracing::warn!("mirror executor pass error: {e}");
                }
            }
            _ = shutdown.notified() => {
                tracing::info!("mirror executor loop: shutdown signal");
                return;
            }
        }
    }
}

async fn run_mirror_pass(
    pool: &SqlitePool,
    cfg: &crate::domain::mirror::ExecutorConfig,
) -> crate::AppResult<()> {
    use crate::commands::mirror_executor;
    use sqlx::Row;
    let now_ms = chrono::Utc::now().timestamp_millis();
    let rows: Vec<mirror_executor::MirrorRow> = sqlx::query_as(
        "SELECT * FROM copy_mirror_queue WHERE status IN ('pending','submitted')",
    )
    .fetch_all(pool)
    .await?;
    let orders: Vec<crate::domain::copy::MirrorOrder> = rows.into_iter().map(Into::into).collect();
    let market_ids: Vec<String> = orders.iter().map(|o| o.market_id.clone()).collect();
    let market_closes = if market_ids.is_empty() {
        std::collections::HashMap::new()
    } else {
        let placeholders = market_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let query = format!("SELECT id, end_date FROM markets WHERE id IN ({})", placeholders);
        let mut q = sqlx::query(&query);
        for id in &market_ids {
            q = q.bind(id);
        }
        let rows = q.fetch_all(pool).await?;
        let mut map = std::collections::HashMap::new();
        for row in rows {
            let id: String = row.try_get("id")?;
            let end: i64 = row.try_get("end_date")?;
            map.insert(id, end);
        }
        map
    };
    let result = crate::domain::mirror::execute_pass(&orders, &market_closes, now_ms, cfg)
        .map_err(|e| {
            tracing::warn!("mirror pass logic error: {e}");
            e
        })?;
    if !result.rejected.is_empty() || !result.picked.is_empty() {
        tracing::info!(
            "mirror executor pass: picked={} rejected={} exposure={:.2} headroom={:.2}",
            result.picked.len(),
            result.rejected.len(),
            result.current_exposure,
            result.headroom
        );
    }
    Ok(())
}

async fn detect_anomalies(pool: &SqlitePool, window: Duration) -> sqlx::Result<()> {
    let now = chrono::Utc::now().timestamp_millis();
    let cutoff = now - window.as_millis() as i64;
    let prev_cutoff = cutoff - window.as_millis() as i64;

    // Per provider: counts + rate limit hits in this window and the previous window
    let rows: Vec<(String, i64, i64, i64)> = sqlx::query_as(
        "SELECT provider_id,
                COUNT(*) as calls,
                SUM(CASE WHEN error_code = 'rate_limit' OR http_status = 429 THEN 1 ELSE 0 END) as rl,
                SUM(cost_cents) as cost
         FROM llm_call_logs
         WHERE called_at >= ? AND called_at < ?
         GROUP BY provider_id",
    )
    .bind(cutoff)
    .bind(now)
    .fetch_all(pool)
    .await?;
    let prev: std::collections::HashMap<String, (i64, f64)> = sqlx::query_as::<_, (String, i64, Option<f64>)>(
        "SELECT provider_id, COUNT(*), SUM(cost_cents)
         FROM llm_call_logs WHERE called_at >= ? AND called_at < ?
         GROUP BY provider_id",
    )
    .bind(prev_cutoff)
    .bind(cutoff)
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|(p, c, co)| (p, (c, co.unwrap_or(0.0))))
    .collect();

    for (provider_id, calls, rl, cost) in rows {
        let (prev_calls, prev_cost) = prev.get(&provider_id).cloned().unwrap_or((0, 0.0));
        let cost_v: f64 = cost as f64;

        let mut anomalies: Vec<&str> = Vec::new();
        // 1. rate limit spike
        if rl >= 5 { anomalies.push("rate_limit_spike"); }
        // 2. cost spike (>3x previous window)
        if prev_cost > 0.0 && cost_v > prev_cost * 3.0 { anomalies.push("cost_spike"); }
        // 3. zero calls but was active
        if calls == 0 && prev_calls > 0 { anomalies.push("zero_activity"); }
        // 4. sudden drop in success rate
        let success_rate: f64 = sqlx::query_scalar(
            "SELECT CAST(SUM(CASE WHEN success = 1 THEN 1 ELSE 0 END) AS REAL) / NULLIF(COUNT(*), 0)
             FROM llm_call_logs WHERE provider_id = ? AND called_at >= ?",
        )
        .bind(&provider_id)
        .bind(cutoff)
        .fetch_one(pool)
        .await
        .unwrap_or(1.0);
        if success_rate < 0.7 && calls >= 3 { anomalies.push("low_success_rate"); }

        if !anomalies.is_empty() {
            tracing::warn!(
                "anomalies detected provider={} calls={} rl={} cost={:.4} prev_cost={:.4} success_rate={:.2} kind={:?}",
                provider_id, calls, rl, cost_v, prev_cost, success_rate, anomalies
            );
            sqlx::query(
                "INSERT INTO audit_log (actor, action, target, payload, result) VALUES ('system', 'anomaly.detected', ?, ?, 'ok')",
            )
            .bind(&provider_id)
            .bind(serde_json::json!({
                "window_min": window.as_secs() / 60,
                "kinds": anomalies,
                "calls": calls,
                "rate_limit_hits": rl,
                "cost_cents": cost_v,
                "success_rate": success_rate,
            }))
            .execute(pool)
            .await?;
        }
    }
    Ok(())
}

// =================================================================
// ============== Public helpers (used by tests) ==================
// =================================================================

/// Run a single daily-brief job synchronously. Useful for tests and
/// manual triggers (already exposed via IPC).
pub async fn run_daily_brief_now(pool: &SqlitePool) -> sqlx::Result<()> {
    run_daily_brief_once(pool).await
}

/// Run a single health-probe sweep synchronously. Useful for tests.
pub async fn run_health_probe_now(pool: &SqlitePool, http: &reqwest::Client) -> sqlx::Result<()> {
    probe_all_providers(pool, http).await
}

// =================================================================
// ============== v0.8c — Audit log purge loop =====================
// =================================================================

use crate::domain::audit::RetentionPolicy;
use crate::infra::db::audit::purge_old;

const DEFAULT_AUDIT_PURGE_HOUR_UTC: u32 = 3; // 3 AM UTC
const DEFAULT_AUDIT_PURGE_TICK_SEC: u64 = 600; // 10 min — cheap check; only fires on the hour

/// Default retention: 90 days, 50k rows cap, 1k floor.
fn default_retention_policy() -> RetentionPolicy {
    RetentionPolicy::default()
}

async fn run_audit_purge_once(pool: &SqlitePool) -> sqlx::Result<usize> {
    let now = chrono::Utc::now().timestamp_millis();
    let policy = default_retention_policy();
    match purge_old(pool, &policy, now).await {
        Ok(n) => {
            if n > 0 {
                tracing::info!(rows_purged = n, "audit log retention purge");
            }
            Ok(n)
        }
        Err(e) => {
            tracing::warn!(error = %e, "audit log retention purge failed");
            Ok(0)
        }
    }
}

async fn run_audit_purge_loop(
    pool: SqlitePool,
    _cfg: SchedulerConfig,
    shutdown: Arc<Notify>,
) {
    // Wait a bit on boot to avoid contention with other loops.
    tokio::time::sleep(Duration::from_secs(30)).await;
    let mut ticker = tokio::time::interval(Duration::from_secs(DEFAULT_AUDIT_PURGE_TICK_SEC));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let _ = DEFAULT_AUDIT_PURGE_HOUR_UTC; // reserved for future hour-gated firing
    loop {
        tokio::select! {
            _ = ticker.tick() => {
                if let Err(e) = run_audit_purge_once(&pool).await {
                    tracing::warn!(error = %e, "audit purge tick error");
                }
            }
            _ = shutdown.notified() => {
                tracing::info!("audit purge loop shutting down");
                break;
            }
        }
    }
}

/// Run a single audit-purge sweep synchronously. Useful for tests +
/// the IPC `purge_audit_log_now` trigger.
pub async fn run_audit_purge_now(pool: &SqlitePool) -> sqlx::Result<usize> {
    run_audit_purge_once(pool).await
}

// =================================================================
// ============== v0.10d — Sidecar health probe loop ===============
// =================================================================

use crate::domain::sidecar_health::SidecarHealthKind;
use crate::infra::db::sidecar_health as sh;

const DEFAULT_SIDECAR_PROBE_SEC: u64 = 30;
const DEFAULT_SIDECAR_PURGE_SEC: u64 = 3600;

async fn ping_sidecar_once() -> (SidecarHealthKind, Option<i64>, Option<String>) {
    // We don't have a handle to the sidecar process here, but the
    // sidecar.rs commands expose `sidecar_request`. To keep this
    // scheduler decoupled from the L2 module, we do a no-op: in
    // v0.10d+ we'll add a "ping" coordinator that the L1 UI can
    // call directly. For now, log "unknown" — the L1 UI still
    // works because the user can ping from the palette.
    //
    // In a follow-up commit, this should be replaced with a real
    // pipe-write of `{"id":"sweeper","method":"ping","params":{}}`
    // and a 2-second read timeout. For now, the schema + writer
    // plumbing is in place.
    let now = chrono::Utc::now().timestamp_millis();
    let _ = now;
    (SidecarHealthKind::Unknown, None, Some("probe not yet wired (v0.10d+ follow-up)".into()))
}

async fn run_sidecar_health_once(pool: &SqlitePool) -> sqlx::Result<()> {
    let at = chrono::Utc::now().timestamp_millis();
    let started = std::time::Instant::now();
    let (kind, _latency, err) = ping_sidecar_once().await;
    let _ = started;
    sh::record_probe(pool, at, kind, None, err.as_deref())
        .await
        .ok();
    // Purge ~once per hour, not every tick
    if at % DEFAULT_SIDECAR_PURGE_SEC as i64 * 1000 < DEFAULT_SIDECAR_PROBE_SEC as i64 * 1000 {
        let n = sh::purge_old(pool, at).await.unwrap_or(0);
        if n > 0 {
            tracing::info!(rows_purged = n, "sidecar_health retention purge");
        }
    }
    Ok(())
}

async fn run_sidecar_health_loop(
    pool: SqlitePool,
    _cfg: SchedulerConfig,
    shutdown: Arc<Notify>,
) {
    // Stagger a bit to avoid contention on cold start
    tokio::time::sleep(Duration::from_secs(20)).await;
    let mut ticker = tokio::time::interval(Duration::from_secs(DEFAULT_SIDECAR_PROBE_SEC));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            _ = ticker.tick() => {
                if let Err(e) = run_sidecar_health_once(&pool).await {
                    tracing::warn!(error = %e, "sidecar health tick error");
                }
            }
            _ = shutdown.notified() => {
                tracing::info!("sidecar health loop shutting down");
                break;
            }
        }
    }
}

/// Run a single sidecar-health probe synchronously. Useful for tests
/// + the IPC `sidecar_health_now` trigger.
pub async fn run_sidecar_health_now(pool: &SqlitePool) -> sqlx::Result<()> {
    run_sidecar_health_once(pool).await
}
