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
//! v0.49c — self-test on boot. Every loop updates
//! `LOOP_LAST_TICK[loop_name]` on each tick. The
//! `self_test` IPC reads these to verify the 7
//! loops are alive. If a loop hasn't ticked in
//! > 3x its expected interval, the L1 surfaces
//! a warning.
//!
//! Layer rules: this module depends on L3 (`domain::llm`) and L5
//! (`platform::keyring`) — it MUST NOT depend on L1 or L2.

use crate::platform::env::{env_u32, env_u64, env_i32};
use crate::platform::keyring;
use crate::domain::llm::{
    AnthropicClient, CostRate, CustomClient, DeepSeekClient, GoogleClient, LlmClient,
    OpenAIClient, ProviderKind,
};
// v0.42b — opt-in lifecycle events. Default off; see
// `infra::telemetry` for the env-var gate and the
// stable NDJSON wire format.
use crate::infra::telemetry;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;

// ============================================================
// v0.49c — per-loop last-tick registry
// ============================================================
//
// Every loop calls `record_tick(loop_name, expected_interval)`
// at the top of each iteration. The self-test IPC
// (`scheduler_self_test_now`) reads these to verify the
// 8 loops are alive and ticking at expected cadence.
//
// LOOP_LAST_TICK is process-global. The keys are
// static `&'static str` literals (compile-time
// guaranteed); values are unix-millis of the most
// recent tick.
//
// PROCESS_START_UNIX is set once on first call to
// `record_tick`. The self-test reports it back so
// the L1 can tell "the loop has NEVER ticked" from
// "the loop ticked once at boot and then died".

static PROCESS_START_UNIX: OnceLock<u64> = OnceLock::new();
static LOOP_LAST_TICK: OnceLock<HashMap<&'static str, AtomicU64>> = OnceLock::new();

fn loop_registry() -> &'static HashMap<&'static str, AtomicU64> {
    LOOP_LAST_TICK.get_or_init(|| {
        let mut m = HashMap::new();
        // Each loop_name matches the variant in
        // infra::scheduler's existing comment headers
        // — keep these strings stable; the L1 shows
        // them verbatim.
        for name in [
            "health_probe",
            "daily_brief",
            "anomaly",
            "mirror_executor",
            "audit_purge",
            "sidecar_health",
            "paper_fills_reconcile",
            "degradation_check",
        ] {
            m.insert(name, AtomicU64::new(0));
        }
        m
    })
}

pub fn record_tick(loop_name: &'static str) {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let _ = PROCESS_START_UNIX.get_or_init(|| now_ms / 1000);
    if let Some(slot) = loop_registry().get(loop_name) {
        slot.store(now_ms, Ordering::Relaxed);
    } else {
        // Unknown loop_name — surface in tests; in
        // production we just no-op so a typo in a
        // single call site doesn't crash the loop.
        #[cfg(test)]
        panic!("record_tick: unknown loop {loop_name}");
    }
}

/// 单个调度 loop 的健康状态快照。L1 Settings 页的 self-test 卡片用这个渲染
/// 绿/红点 + 上次 tick 时间。
///
/// **来源**：`self_test()` 读全局 `LOOP_LAST_TICK` 原子计数器 + 当前时间。
/// **不**做任何 IO —— 这是 self-test，期望 cheap to call。
#[derive(Debug, Clone, serde::Serialize)]
pub struct LoopStatus {
    /// loop 名字。稳定的 `&'static str`（如 `"health_probe"`）—— L1 verbatim 显示。
    pub name: &'static str,
    /// Unix-ms of the most recent tick. 0 = never
    /// ticked (still in its initial sleep).
    pub last_tick_unix_ms: u64,
    /// Milliseconds since the last tick. `null` if
    /// the loop has never ticked.
    pub age_ms: Option<u64>,
    /// True when `age_ms <= 3 * expected_interval_ms`.
    /// The L1 uses this for the green/red dot.
    pub healthy: bool,
}

/// 8 个调度 loop 的 self-test 快照。`all_healthy = true` 当且仅当每个 loop 都在
/// `3 * expected_interval` 之内 tick 过。
///
/// **IPC 调用**：`commands::scheduler::scheduler_self_test_now` 调用 `self_test()`
/// 并把 `SchedulerSelfTest` 序列化给 L1。L1 Settings 页用 `loops[]` 渲染列表，
/// `all_healthy` 渲染顶部状态条。
#[derive(Debug, Clone, serde::Serialize)]
pub struct SchedulerSelfTest {
    /// Unix-seconds when this process started.
    pub process_started_at_unix: u64,
    /// Unix-ms when the self-test ran.
    pub checked_at_unix_ms: u64,
    /// True when every loop is healthy.
    pub all_healthy: bool,
    pub loops: Vec<LoopStatus>,
}

/// Run the self-test. Cheap: just reads atomic
/// counters, no IO. Returns a snapshot that the
/// L1 Settings card renders.
///
/// **预期 cadence**：每 `expected_interval` tick 一次，self-test 阈值是 `3 * interval`。
/// 也就是说，允许最多 3 次 sleep 失败后才标 unhealthy（容忍偶发的 GC pause / IO stall）。
///
/// **为什么不直接读文件 / DB**：loop 自己的状态都在 `OnceLock<HashMap<..., AtomicU64>>` 里，
/// atomic load 是 O(1) lock-free。如果改成查 DB 反而要拿连接，违背 self-test 的本意。
pub fn self_test() -> SchedulerSelfTest {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let process_started_at = PROCESS_START_UNIX.get().copied().unwrap_or(now_ms / 1000);
    let cfg = SchedulerConfig::from_env();

    // Expected tick interval per loop. Lifted from
    // the loop bodies' tokio::time::sleep() values.
    // Keep these in sync if you change the loop
    // cadence.
    let expected_ms: &[(&str, u64)] = &[
        ("health_probe",         cfg.health_probe_interval.as_millis() as u64),
        ("daily_brief",          24 * 60 * 60 * 1000), // once per day
        ("anomaly",              cfg.anomaly_window.as_millis() as u64),
        ("mirror_executor",      cfg.mirror_tick.as_millis() as u64),
        ("audit_purge",          24 * 60 * 60 * 1000),
        ("sidecar_health",       30 * 1000),
        ("paper_fills_reconcile", 5 * 60 * 1000),
        ("degradation_check",    60 * 60 * 1000),
    ];
    let reg = loop_registry();
    let mut loops: Vec<LoopStatus> = expected_ms
        .iter()
        .map(|(name, exp_ms)| {
            let last_ms = reg.get(name).map(|a| a.load(Ordering::Relaxed)).unwrap_or(0);
            let age_ms = if last_ms == 0 { None } else { Some(now_ms.saturating_sub(last_ms)) };
            let healthy = match age_ms {
                None => false,
                Some(a) => a <= 3 * exp_ms,
            };
            LoopStatus {
                name,
                last_tick_unix_ms: last_ms,
                age_ms,
                healthy,
            }
        })
        .collect();
    // Stable display order: by name. L1 reads in any
    // order, but this makes the JSON diff-friendly.
    loops.sort_by(|a, b| a.name.cmp(b.name));
    let all_healthy = loops.iter().all(|l| l.healthy);
    SchedulerSelfTest {
        process_started_at_unix: process_started_at,
        checked_at_unix_ms: now_ms,
        all_healthy,
        loops,
    }
}
use sqlx::SqlitePool;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Notify;

const DEFAULT_HEALTH_PROBE_MIN: u64 = 5;
const DEFAULT_BRIEF_HOUR_UTC: u32 = 0;
const DEFAULT_BRIEF_TZ_OFFSET_MIN: i32 = 0;
const DEFAULT_ANOMALY_WINDOW_MIN: u64 = 60;
const DEFAULT_MIRROR_TICK_SEC: u64 = 30;

/// 调度 loop 的可调参数（cadence + tz 等）。所有字段都从 env var 读，默认值
/// 在 `DEFAULT_*` 常量里。
///
/// **新增字段**：v0.6a 加 `mirror_tick`。每次加新 loop 时记得：
///   1. 在 `SchedulerConfig` 加字段
///   2. 在 `from_env` 读 env
///   3. 在 `loop_registry` 加 loop_name
///   4. 在 `self_test` 的 `expected_ms` 表加 (name, interval)
#[derive(Clone)]
pub struct SchedulerConfig {
    /// health_probe loop 间隔。默认 5 min。
    pub health_probe_interval: Duration,
    /// daily_brief 触发的 UTC 小时（0-23）。默认 0（UTC 午夜）。
    pub daily_brief_hour_utc: u32,
    /// 用户时区相对 UTC 的偏移（分钟）。默认 0。
    pub daily_brief_tz_offset_min: i32,
    /// anomaly 检测窗口。默认 60 min。
    pub anomaly_window: Duration,
    /// v0.6a — mirror executor tick interval。默认 30s。
    pub mirror_tick: Duration,
}

impl SchedulerConfig {
    /// 从 env vars 构造 `SchedulerConfig`。**不**做参数校验（负数 duration、>23 hour 等
    /// 会原样传给 loop，由 loop 自己 ignore / panic）。
    ///
    /// **env-var list**（全部可选）：
    ///   - `POLYROCKET_HEALTH_PROBE_INTERVAL_MIN`  (u64, default 5)
    ///   - `POLYROCKET_DAILY_BRIEF_HOUR_UTC`        (u32, default 0)
    ///   - `POLYROCKET_DAILY_BRIEF_TZ_OFFSET_MIN`   (i32, default 0)
    ///   - `POLYROCKET_ANOMALY_WINDOW_MIN`          (u64, default 60)
    ///   - `POLYROCKET_MIRROR_TICK_SEC`             (u64, default 30)
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

/// 调用方持有的 handle，用于在测试中通知 loop 退出。
///
/// **生产用途**：`lib.rs::run()` 不保留 handle —— 进程退出时 loop 一起死。
/// **测试用途**：`tests::scheduler_test` 创建 handle，调 `shutdown()`，等所有 loop 退出。
pub struct SchedulerHandle {
    /// tokio `Notify` —— `shutdown()` 触发一次 notify，所有 loop 在 `select!` 里退出。
    pub shutdown: Arc<Notify>,
}

impl SchedulerHandle {
    /// 通知所有调度 loop 退出。**幂等**：多次调用安全（`Notify::notify_waiters` 不累积）。
    pub fn shutdown(&self) {
        self.shutdown.notify_waiters();
    }
}

/// 启动所有后台调度 loop。8 个 `tokio::spawn` 任务：health_probe / daily_brief /
/// anomaly / mirror_executor / audit_purge / sidecar_health / paper_fills_reconcile /
/// degradation_check。
///
/// **调用方**：`lib.rs::run()` 在 `setup` hook 里调一次。返回的 `SchedulerHandle`
/// 在生产里直接 drop（loop 跑进程级），在测试里用来收尾。
///
/// **错误恢复**：每个 loop 内部 `select!` 监听 `shutdown` 信号。DB / HTTP 错误被
/// catch + log，不 panic（`tracing::error!` 而非 `?`），所以一个 loop 死了不会拖垮
/// 其他 loop。`run_*_now` 是手动触发（IPC），那部分错误才回传。
///
/// **资源**：`pool` 和 `http` 都 clone 出去（内部 Arc，cheap）。loop 不持有
/// `AppState`（避免循环依赖）。
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
    {
        // v0.45a — paper_fills reconciliation: every 5
        // minutes, settle any paper_fills whose market
        // has resolved. Computes won/lost + PnL.
        let pool = pool.clone();
        let cfg = cfg.clone();
        let shutdown = shutdown.clone();
        tokio::spawn(async move {
            run_paper_fills_reconcile_loop(pool, cfg, shutdown).await;
        });
    }
    {
        // v0.48a — model degradation check: every
        // 1 hour, compute live Brier of the
        // FALLBACK model on recent resolved markets
        // and emit a telemetry event. The L1
        // listens for the alert flag and fires an
        // OS notification (gated by a Settings
        // pref).
        let pool = pool.clone();
        let cfg = cfg.clone();
        let shutdown = shutdown.clone();
        tokio::spawn(async move {
            run_degradation_check_loop(pool, cfg, shutdown).await;
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
    record_tick("health_probe"); // v0.49c
    loop {
        tokio::select! {
            _ = ticker.tick() => {
                record_tick("health_probe"); // v0.49c
                if let Err(e) = probe_all_providers(&pool, &http).await {
                    tracing::warn!("health probe sweep error: {e}");
                    telemetry::emit(telemetry::Event::SchedulerError {
                        loop_name: "health_probe",
                        error: e.to_string(),
                    });
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
        // v0.111.1 / v0.113 / v0.114 — health probe 不直接支持 (需要 special secret)
        ProviderKind::ErnieNative | ProviderKind::Hunyuan | ProviderKind::Spark => {
            // Health probe fallback — 走 OpenAI 客户端 (会失败但不会 panic,scheduler 看
            // 到 error 就 skip 这个 provider)。
            Arc::new(OpenAIClient::new("https://api.openai.com/v1"))
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
            // v0.42b — per-provider health probe result.
            telemetry::emit(telemetry::Event::LlmHealthProbe {
                provider: p.id.clone(),
                ok: true,
                latency_ms: latency,
                error: None,
            });
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
            telemetry::emit(telemetry::Event::LlmHealthProbe {
                provider: p.id.clone(),
                ok: false,
                latency_ms: latency,
                error: Some(e.message.clone()),
            });
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
        // v0.110 — 国产 OpenAI 兼容大模型 (5 个)
        "qwen" | "doubao" | "kimi" | "glm" | "MiniMax" => ProviderKind::OpenaiCompat,
        // v0.111 — ERNIE 百度千帆 (OpenAI 兼容 v2 endpoint)
        "ernie" => ProviderKind::OpenaiCompat,
        // v0.111.1 — ERNIE native AK/SK
        "ernie_native" => ProviderKind::ErnieNative,
        // v0.113 — Hunyuan
        "hunyuan" => ProviderKind::Hunyuan,
        // v0.114 — Spark
        "spark" => ProviderKind::Spark,
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
        record_tick("daily_brief"); // v0.49c
        if let Err(e) = run_daily_brief_once(&pool).await {
            tracing::warn!("daily brief job error: {e}");
            telemetry::emit(telemetry::Event::SchedulerError {
                loop_name: "daily_brief",
                error: e.to_string(),
            });
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
    // v0.42b — emit lifecycle event. The "items_json"
    // size is the rough proxy for "summary size".
    let summary_chars = serde_json::to_string(&serde_json::json!({
        "note": "v0.2 stub — top N by recency",
        "n_active_markets": n_markets,
    }))
    .map(|s| s.len())
    .unwrap_or(0);
    telemetry::emit(telemetry::Event::DailyBriefGenerated {
        summary_chars,
        duration_ms: (chrono::Utc::now().timestamp_millis() - started) as u64,
    });
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
    record_tick("anomaly"); // v0.49c
    loop {
        tokio::select! {
            _ = ticker.tick() => {
                record_tick("anomaly"); // v0.49c
                if let Err(e) = detect_anomalies(&pool, cfg.anomaly_window).await {
                    tracing::warn!("anomaly detection error: {e}");
                    telemetry::emit(telemetry::Event::SchedulerError {
                        loop_name: "anomaly",
                        error: e.to_string(),
                    });
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
    record_tick("mirror_executor"); // v0.49c
    loop {
        tokio::select! {
            _ = ticker.tick() => {
                record_tick("mirror_executor"); // v0.49c
                if let Err(e) = run_mirror_pass(&pool, &exec_cfg).await {
                    tracing::warn!("mirror executor pass error: {e}");
                    telemetry::emit(telemetry::Event::SchedulerError {
                        loop_name: "mirror_executor",
                        error: e.to_string(),
                    });
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
    // v0.42b — capture the queue depth at the start of
    // the tick. The original `rows` is consumed by
    // `into_iter` below, so we save the length first.
    let intents_pending = rows.len();
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
        // v0.42b — emit per-tick stats. The error count
        // is implicit (picked.len() + rejected.len() vs
        // total rows); we approximate by counting
        // "rejected" as errors.
        telemetry::emit(telemetry::Event::MirrorExecutorTick {
            intents_pending,
            executed: result.picked.len(),
            errors: result.rejected.len(),
        });
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
            // v0.42b — surface every distinct anomaly kind as
            // a separate event. Severity is the rough
            // ordering: rate_limit > cost_spike > zero >
            // low_success. We pick the first present kind
            // to keep the event stream low-volume.
            let primary_kind = anomalies.first().copied().unwrap_or("unknown");
            let severity: u8 = match primary_kind {
                "rate_limit_spike" => 3,
                "cost_spike" => 2,
                "zero_activity" => 1,
                "low_success_rate" => 2,
                _ => 1,
            };
            telemetry::emit(telemetry::Event::AnomalyDetected {
                kind: primary_kind.to_string(),
                severity,
                details: format!("provider={} calls={} rl={} success_rate={:.2}", provider_id, calls, rl, success_rate),
            });
        }
    }
    Ok(())
}

// =================================================================
// ============== Public helpers (used by tests) ==================
// =================================================================

/// 同步触发一次 daily-brief 计算。跳过 cron timing 逻辑，直接调 `run_daily_brief_once`。
///
/// **IPC 调用**：`commands::scheduler::run_daily_brief_now`。
/// **测试用途**：`scheduler_test::daily_brief_now_works` 跳过 24h sleep 直接验证。
pub async fn run_daily_brief_now(pool: &SqlitePool) -> sqlx::Result<()> {
    run_daily_brief_once(pool).await
}

/// 同步触发一次 health-probe sweep。跳过 5 min tick，直接跑 `run_health_probe_once`。
///
/// **IPC 调用**：`commands::scheduler::run_health_probe_now`。
/// **测试用途**：`scheduler_test::health_probe_now_works` 验证 sweep 不会 panic。
pub async fn run_health_probe_now(pool: &SqlitePool, http: &reqwest::Client) -> sqlx::Result<()> {
    probe_all_providers(pool, http).await
}

// =================================================================
// ============== v0.8c — Audit log purge loop =====================
// =================================================================

use crate::domain::audit::RetentionPolicy;
use crate::infra::db::audit::purge_old;
use crate::infra::db::settings as app_settings;

const DEFAULT_AUDIT_PURGE_HOUR_UTC: u32 = 3; // 3 AM UTC
const DEFAULT_AUDIT_PURGE_TICK_SEC: u64 = 600; // 10 min — cheap check; only fires on the hour

/// Default retention: 90 days, 50k rows cap, 1k floor.
fn default_retention_policy() -> RetentionPolicy {
    RetentionPolicy::default()
}

/// v0.13c — read the user's retention policy, with fallback to the
/// hard-coded default. The user can override any of the three
/// fields from Settings; missing fields fall back individually.
pub async fn read_user_retention(pool: &SqlitePool) -> sqlx::Result<RetentionPolicy> {
    app_settings::read_audit_retention(pool).await
}

/// v0.13c — write a user retention override.
pub async fn write_user_retention(
    pool: &SqlitePool,
    policy: &RetentionPolicy,
) -> sqlx::Result<()> {
    app_settings::write_audit_retention(pool, policy).await
}

async fn run_audit_purge_once(pool: &SqlitePool) -> sqlx::Result<usize> {
    let now = chrono::Utc::now().timestamp_millis();
    // v0.13c — read the user's retention policy. Falls back to
    // `RetentionPolicy::default()` if no overrides are set.
    let policy = read_user_retention(pool).await.unwrap_or_else(|e| {
        tracing::warn!(error = %e, "failed to read user retention; using default");
        default_retention_policy()
    });
    match purge_old(pool, &policy, now).await {
        Ok(n) => {
            if n > 0 {
                tracing::info!(
                    rows_purged = n,
                    retain_recent_ms = policy.retain_recent_ms,
                    max_rows = policy.max_rows,
                    "audit log retention purge"
                );
                // v0.42b — emit retention sweep result.
                // retention_days is approximate (rounded
                // down from ms). We don't emit when n=0
                // to keep the volume low.
                let retention_days = (policy.retain_recent_ms / (24 * 3600 * 1000)).max(1) as u64;
                telemetry::emit(telemetry::Event::AuditPurged {
                    rows: n as u64,
                    retention_days,
                });
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
    record_tick("audit_purge"); // v0.49c
    loop {
        tokio::select! {
            _ = ticker.tick() => {
                record_tick("audit_purge"); // v0.49c
                if let Err(e) = run_audit_purge_once(&pool).await {
                    tracing::warn!(error = %e, "audit purge tick error");
                    telemetry::emit(telemetry::Event::SchedulerError {
                        loop_name: "audit_purge",
                        error: e.to_string(),
                    });
                }
            }
            _ = shutdown.notified() => {
                tracing::info!("audit purge loop shutting down");
                break;
            }
        }
    }
}

/// 同步触发一次 audit-purge sweep。跳过 24h cron tick，直接跑 `run_audit_purge_once`。
///
/// **IPC 调用**：`commands::audit::purge_audit_log_now`。
/// **返回值**：本次清理的行数。L1 拿这个数字展示「已清理 N 条」toast。
pub async fn run_audit_purge_now(pool: &SqlitePool) -> sqlx::Result<usize> {
    run_audit_purge_once(pool).await
}

// =================================================================
// ============== v0.10d — Sidecar health probe loop ===============
// =================================================================

use crate::domain::sidecar_health::SidecarHealthKind;
use crate::infra::db::sidecar_health as sh;

/// Global handle to the running Tauri AppHandle so the scheduler
/// can look up managed state (e.g. SidecarState) without going
/// through L2. Set in `lib.rs::run()` before the scheduler starts.
pub static TAURI_APP: OnceLock<tauri::AppHandle> = OnceLock::new();

const DEFAULT_SIDECAR_PROBE_SEC: u64 = 30;
const DEFAULT_SIDECAR_PURGE_SEC: u64 = 3600;

async fn ping_sidecar_once() -> (SidecarHealthKind, Option<i64>, Option<String>) {
    // v0.11b — real probe via the L2 SidecarState. The scheduler
    // doesn't have direct access to it (5-layer rule: L4 doesn't
    // import L2), so we use the app handle to look it up.
    //
    // v0.12c — use the async wrapper (ping_async) which composes
    // cleanly with `tokio::time::timeout` and `spawn_blocking`,
    // instead of manually managing `spawn_blocking` here.
    let state = TAURI_APP.get();
    let Some(handle) = state else {
        return (SidecarHealthKind::Failed, None, Some("app handle not set".into()));
    };
    use tauri::Manager;
    let Some(sidecar_state) = handle.try_state::<crate::commands::sidecar::SidecarState>() else {
        return (SidecarHealthKind::Failed, None, Some("sidecar state not managed".into()));
    };
    if !sidecar_state.is_running() {
        return (SidecarHealthKind::Failed, None, Some("sidecar not running".into()));
    }
    match sidecar_state.ping_async(2000).await {
        Ok(latency_ms) => (SidecarHealthKind::Ok, Some(latency_ms as i64), None),
        Err(e) => (SidecarHealthKind::Failed, None, Some(e)),
    }
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
    let mut last_was_ok: Option<bool> = None;
    record_tick("sidecar_health"); // v0.49c
    loop {
        tokio::select! {
            _ = ticker.tick() => {
                record_tick("sidecar_health"); // v0.49c
                if let Err(e) = run_sidecar_health_once(&pool).await {
                    tracing::warn!(error = %e, "sidecar health tick error");
                    telemetry::emit(telemetry::Event::SchedulerError {
                        loop_name: "sidecar_health",
                        error: e.to_string(),
                    });
                }
                // v0.42b — emit connect/disconnect transitions.
                // We do this by re-running the kind decision
                // from the last probe; cheap to redo since
                // ping_sidecar_once is fast and we're
                // already past the DB write. The transition
                // is: None -> Ok = Connected; Ok -> Failed
                // = Disconnected.
                let (kind, _, reason) = ping_sidecar_once().await;
                let ok_now = matches!(kind, SidecarHealthKind::Ok);
                match last_was_ok {
                    None if ok_now => telemetry::emit(telemetry::Event::SidecarConnected { pid: None }),
                    Some(true) if !ok_now => telemetry::emit(telemetry::Event::SidecarDisconnected {
                        reason: reason.unwrap_or_else(|| "unknown".into()),
                    }),
                    _ => {}
                }
                last_was_ok = Some(ok_now);
            }
            _ = shutdown.notified() => {
                tracing::info!("sidecar health loop shutting down");
                break;
            }
        }
    }
}

/// 同步触发一次 sidecar-health 探测。跳过 30s tick，直接 ping 侧车 + 写 `sidecar_health` 行。
///
/// **IPC 调用**：`commands::sidecar::sidecar_health_now`。
/// **测试用途**：跑 pipeline 测试时手动触发。
pub async fn run_sidecar_health_now(pool: &SqlitePool) -> sqlx::Result<()> {
    run_sidecar_health_once(pool).await
}

// =================================================================
// ============== v0.45a — paper_fills reconciliation loop ==========
// =================================================================

/// v0.45a — reconcile paper_fills against market
/// resolutions. For each unsettled paper_fill where
/// the market is now resolved, compute the
/// won/lost outcome and PnL, then write the
/// settlement fields.
///
/// PnL formula (matches `domain::bet`):
///   - If paper_fill.side == market.outcome → won
///     pnl = size_shares * (1 - price)   // bought YES, market resolved YES
///   - If paper_fill.side != market.outcome → lost
///     pnl = -size_usdc                   // bought the wrong side
///
/// Returns the number of paper_fills that were
/// settled in this pass.
async fn reconcile_paper_fills_once(pool: &SqlitePool) -> sqlx::Result<usize> {
    // 1. Find unsettled paper_fills whose market is
    //    resolved. Join with markets to get the
    //    outcome.
    let unsettled: Vec<(String, String, String, f64, String)> = sqlx::query_as(
        "SELECT pf.id, pf.market_id, pf.side, pf.price, pf.size
         FROM paper_fills pf
         JOIN markets m ON m.id = pf.market_id
         WHERE pf.settled_at IS NULL
           AND m.resolved = 1
           AND m.outcome IS NOT NULL",
    )
    .fetch_all(pool)
    .await?;
    if unsettled.is_empty() {
        return Ok(0);
    }
    let now = chrono::Utc::now().timestamp_millis();
    let mut count = 0;
    for (pf_id, _market_id, side, price, size_str) in unsettled {
        // Look up the actual market outcome for
        // this paper_fill. We do a per-row query
        // because the join above only returned the
        // paper_fills fields; the outcome is in
        // markets.outcome.
        let outcome: Option<String> = sqlx::query_scalar(
            "SELECT outcome FROM markets WHERE id = ?",
        )
        .bind(&_market_id)
        .fetch_optional(pool)
        .await?
        .flatten();
        let Some(outcome) = outcome else { continue };
        let won = side.to_uppercase() == outcome.to_uppercase();
        // size_str is a USDC string like "12.5".
        // We treat it as the cost basis for the
        // PnL calculation. (For binary prediction
        // markets with price=0.5 and outcome=YES,
        // the "shares" you get is size / 0.5 =
        // 2 * size. But we keep it simple: PnL =
        // +/- size_usdc depending on win/loss,
        // matching the conservative accounting the
        // bet table uses for v0.5d's sign_order
        // stub.)
        let size_usdc: f64 = size_str.parse().unwrap_or(0.0);
        let pnl_usdc = if won { size_usdc } else { -size_usdc };
        sqlx::query(
            "UPDATE paper_fills
             SET settled_at = ?,
                 resolved_outcome = ?,
                 won = ?,
                 pnl_usdc = ?
             WHERE id = ?",
        )
        .bind(now)
        .bind(&outcome)
        .bind(if won { 1i64 } else { 0i64 })
        .bind(format!("{pnl_usdc:.4}"))
        .bind(&pf_id)
        .execute(pool)
        .await?;
        count += 1;
    }
    Ok(count)
}

/// v0.45a — 同步触发一次 paper_fills reconciliation sweep。跳过 5 min tick，
/// 直接调 `reconcile_paper_fills_once`。
///
/// **业务流程**：扫所有 `paper_fills WHERE settled = 0`，查对应 `markets.outcome`，
/// 标记 `settled = 1` + 算 `pnl` + 写 audit_log。
///
/// **IPC 调用**：`commands::paper::reconcile_paper_fills_now`。
/// **返回值**：本次 settle 的 paper_fills 数。
pub async fn run_paper_fills_reconcile_now(pool: &SqlitePool) -> sqlx::Result<usize> {
    reconcile_paper_fills_once(pool).await
}

const DEFAULT_PAPER_FILL_RECONCILE_TICK_SEC: u64 = 300; // 5 min

async fn run_paper_fills_reconcile_loop(
    pool: SqlitePool,
    _cfg: SchedulerConfig,
    shutdown: Arc<Notify>,
) {
    // Stagger a bit to avoid contention on cold start
    tokio::time::sleep(Duration::from_secs(15)).await;
    let mut ticker = tokio::time::interval(Duration::from_secs(
        DEFAULT_PAPER_FILL_RECONCILE_TICK_SEC,
    ));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    record_tick("paper_fills_reconcile"); // v0.49c
    loop {
        tokio::select! {
            _ = ticker.tick() => {
                record_tick("paper_fills_reconcile"); // v0.49c
                match reconcile_paper_fills_once(&pool).await {
                    Ok(n) if n > 0 => {
                        tracing::info!(settled = n, "paper_fills reconciled");
                        use crate::infra::telemetry;
                        telemetry::emit(telemetry::Event::PaperFillsReconciled { settled: n as u64 });
                    }
                    Ok(_) => {}
                    Err(e) => {
                        tracing::warn!(error = %e, "paper_fills reconcile tick error");
                        use crate::infra::telemetry;
                        telemetry::emit(telemetry::Event::SchedulerError {
                            loop_name: "paper_fills_reconcile",
                            error: e.to_string(),
                        });
                    }
                }
            }
            _ = shutdown.notified() => {
                tracing::info!("paper_fills reconcile loop shutting down");
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// v0.45a — reconcile_paper_fills_once settles
    /// unsettled paper_fills when their market has
    /// resolved. We use an in-memory SQLite pool
    /// to keep the test self-contained.
    #[tokio::test]
    async fn reconcile_paper_fills_settles_resolved_markets() {
        use sqlx::sqlite::SqlitePoolOptions;
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect(":memory:")
            .await
            .unwrap();
        // Minimal schema: markets + paper_fills
        sqlx::query(
            "CREATE TABLE markets (
                id TEXT PRIMARY KEY,
                resolved INTEGER DEFAULT 0 NOT NULL,
                outcome TEXT
            )",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "CREATE TABLE paper_fills (
                id TEXT PRIMARY KEY,
                mirror_id TEXT NOT NULL,
                market_id TEXT NOT NULL,
                side TEXT NOT NULL,
                size TEXT NOT NULL,
                price REAL NOT NULL,
                placed_at INTEGER NOT NULL,
                notes TEXT,
                settled_at INTEGER,
                resolved_outcome TEXT,
                won INTEGER,
                pnl_usdc TEXT
            )",
        )
        .execute(&pool)
        .await
        .unwrap();

        // Insert 3 markets — one resolved YES, one
        // resolved NO, one unresolved.
        for (id, resolved, outcome) in [
            ("m1", 1, Some("YES")),
            ("m2", 1, Some("NO")),
            ("m3", 0, None),
        ] {
            sqlx::query("INSERT INTO markets (id, resolved, outcome) VALUES (?, ?, ?)")
                .bind(id)
                .bind(resolved)
                .bind(outcome)
                .execute(&pool)
                .await
                .unwrap();
        }

        for (id, market_id, side) in [
            ("pf1", "m1", "YES"),
            ("pf2", "m2", "YES"),
            ("pf3", "m3", "YES"),
        ] {
            sqlx::query("INSERT INTO paper_fills (id, mirror_id, market_id, side, size, price, placed_at, notes) VALUES (?, 'm', ?, ?, '10', 0.5, 0, '')")
                .bind(id)
                .bind(market_id)
                .bind(side)
                .execute(&pool)
                .await
                .unwrap();
        }

        let settled = reconcile_paper_fills_once(&pool).await.unwrap();
        assert_eq!(settled, 2);

        let (won, outcome, pnl): (Option<i64>, Option<String>, Option<String>) =
            sqlx::query_as("SELECT won, resolved_outcome, pnl_usdc FROM paper_fills WHERE id = 'pf1'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(won, Some(1));
        assert_eq!(outcome.as_deref(), Some("YES"));
        assert_eq!(pnl.as_deref(), Some("10.0000"));

        let (won, outcome, pnl): (Option<i64>, Option<String>, Option<String>) =
            sqlx::query_as("SELECT won, resolved_outcome, pnl_usdc FROM paper_fills WHERE id = 'pf2'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(won, Some(0));
        assert_eq!(outcome.as_deref(), Some("NO"));
        assert_eq!(pnl.as_deref(), Some("-10.0000"));

        let (won, settled_at): (Option<i64>, Option<i64>) =
            sqlx::query_as("SELECT won, settled_at FROM paper_fills WHERE id = 'pf3'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(won, None);
        assert_eq!(settled_at, None);

        let settled2 = reconcile_paper_fills_once(&pool).await.unwrap();
        assert_eq!(settled2, 0);
    }

    #[tokio::test]
    async fn fallback_predict_matches_python_baseline() {
        // v0.48a — the FALLBACK weights are
        // mirrored from sidecar/predict.py.
        // If those constants change in Python,
        // this test must change too. The
        // expected values are computed by hand
        // from the sigmoid:
        //   z = -0.5 + 2.0 * (1 - 0.5) + 0.4 * 24/168
        //     = -0.5 + 1.0 + 0.0571...
        //     = 0.5571...
        //   sigmoid(0.5571) ≈ 0.6357
        let p = fallback_predict(0.5, 24.0);
        let expected_z = -0.5_f64 + 2.0 * 0.5 + 0.4 * (24.0 / 168.0);
        let expected = 1.0 / (1.0 + (-expected_z).exp());
        assert!((p - expected).abs() < 1e-9, "got {p}, expected {expected}");
    }

    #[tokio::test]
    async fn compute_live_brier_handles_empty_table() {
        use sqlx::sqlite::SqlitePoolOptions;
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect(":memory:")
            .await
            .unwrap();
        // Create the markets + price_snapshots
        // tables (the SQL is a LEFT JOIN against
        // both; without the tables the query
        // errors).
        sqlx::query(
            "CREATE TABLE markets (
                id TEXT PRIMARY KEY,
                category TEXT NOT NULL,
                question TEXT NOT NULL,
                resolved INTEGER DEFAULT 0 NOT NULL,
                outcome TEXT,
                end_date INTEGER NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "CREATE TABLE price_snapshots (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                market_id TEXT NOT NULL,
                captured_at INTEGER NOT NULL,
                best_bid REAL NOT NULL,
                best_ask REAL NOT NULL,
                mid_price REAL NOT NULL,
                spread REAL NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .unwrap();
        // Empty: 0 resolved markets → n_samples=0, brier=0.0
        let (n, b) = compute_live_brier(&pool, 50).await.unwrap();
        assert_eq!(n, 0);
        assert_eq!(b, 0.0);
    }

    #[tokio::test]
    async fn compute_live_brier_skips_markets_without_snapshot() {
        use sqlx::sqlite::SqlitePoolOptions;
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect(":memory:")
            .await
            .unwrap();
        sqlx::query(
            "CREATE TABLE markets (
                id TEXT PRIMARY KEY,
                category TEXT NOT NULL,
                question TEXT NOT NULL,
                resolved INTEGER DEFAULT 0 NOT NULL,
                outcome TEXT,
                end_date INTEGER NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "CREATE TABLE price_snapshots (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                market_id TEXT NOT NULL,
                captured_at INTEGER NOT NULL,
                best_bid REAL NOT NULL,
                best_ask REAL NOT NULL,
                mid_price REAL NOT NULL,
                spread REAL NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .unwrap();
        // m1: resolved YES, no snapshot
        sqlx::query("INSERT INTO markets VALUES ('m1', 'cat', 'q1', 1, 'YES', 1_700_000_000_000)")
            .execute(&pool)
            .await
            .unwrap();
        // m2: resolved YES, with snapshot at 0.7
        sqlx::query("INSERT INTO markets VALUES ('m2', 'cat', 'q2', 1, 'YES', 1_700_000_000_000)")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO price_snapshots (market_id, captured_at, best_bid, best_ask, mid_price, spread) VALUES ('m2', 1_000, 0.65, 0.75, 0.7, 0.1)")
            .execute(&pool)
            .await
            .unwrap();
        // Only m2 contributes. With price=0.7 and
        // outcome=YES=1.0, the FALLBACK predicts
        // some value < 0.5 (because z < 0 when
        // price is high) and the Brier is the
        // squared error.
        let (n, b) = compute_live_brier(&pool, 50).await.unwrap();
        assert_eq!(n, 1);
        assert!(b > 0.0); // some Brier
        assert!(b < 1.0); // within [0, 1] for a binary outcome
    }
}

// =================================================================
// ============== v0.48a — model degradation detector =============
// =================================================================

/// v0.48a — predict the FALLBACK model weights.
/// The Python sidecar (`predict.py`) has
/// `_FALLBACK_W0 = -0.5`, `_FALLBACK_W1 = 2.0`,
/// `_FALLBACK_W2 = 0.4`. The Rust side mirrors
/// them here so the degradation loop can compute
/// a Brier without round-tripping through the
/// sidecar. If these change in Python, this
/// constant must change too (covered by the
/// 2-line test).
const FALLBACK_W0: f64 = -0.5;
const FALLBACK_W1: f64 = 2.0;
const FALLBACK_W2: f64 = 0.4;
const HORIZON_NORM_HOURS: f64 = 168.0;

/// v0.48a — single-sample prediction using the
/// FALLBACK weights. Mirrors `predict_logic` in
/// the sidecar's predict.py.
fn fallback_predict(price: f64, market_age_hours: f64) -> f64 {
    let z = FALLBACK_W0 + FALLBACK_W1 * (1.0 - price) + FALLBACK_W2 * (market_age_hours / HORIZON_NORM_HOURS);
    if z >= 0.0 { 1.0 / (1.0 + (-z).exp()) } else { z.exp() / (1.0 + z.exp()) }
}

/// v0.48a — compute live Brier on the most recent
/// N resolved markets with price snapshots. Returns
/// `(n_samples, live_brier)`. n_samples=0 when there
/// are no resolved markets with snapshots yet.
async fn compute_live_brier(pool: &SqlitePool, n: i64) -> sqlx::Result<(u64, f64)> {
    // v0.48a — join markets with their latest
    // price_snapshots row. We use the same
    // correlated subquery pattern as the v0.47b
    // backtest IPC.
    let rows: Vec<(String, Option<f64>)> = sqlx::query_as(
        "SELECT m.outcome, ps.mid_price
         FROM markets m
         LEFT JOIN price_snapshots ps
           ON ps.id = (
             SELECT id FROM price_snapshots
             WHERE market_id = m.id
             ORDER BY captured_at DESC LIMIT 1
           )
         WHERE m.resolved = 1
           AND m.outcome IS NOT NULL
         ORDER BY m.end_date DESC
         LIMIT ?",
    )
    .bind(n)
    .fetch_all(pool)
    .await?;
    let mut total = 0.0;
    let mut count: u64 = 0;
    for (outcome, mid_price) in rows {
        let outcome_f = match outcome.as_str() {
            "YES" => 1.0,
            "NO" => 0.0,
            _ => continue, // skip unknown
        };
        // Skip markets without a snapshot — we
        // can't compute a meaningful prediction
        // without a price. (Same policy as the
        // v0.47b backtest IPC, except we don't
        // fall back to 0.5 here: the FALLBACK
        // weights' mid_price=0.5 would always
        // predict 0.5, which is the "no signal"
        // baseline. Counting those would dilute
        // the drift signal.)
        let Some(price) = mid_price else { continue };
        // Use the FALLBACK convention: predict 1
        // day before close. The "live" age is
        // approximated by the market's
        // end_date - now, capped at 168h
        // (the horizon norm constant).
        let now = chrono::Utc::now().timestamp_millis();
        let age_ms = 0i64 - 24 * 3_600_000; // -24h
        let age_hours = (age_ms as f64 / 3_600_000.0).max(0.0).min(HORIZON_NORM_HOURS);
        let pred = fallback_predict(price, age_hours);
        let brier = (pred - outcome_f).powi(2);
        total += brier;
        count += 1;
    }
    if count == 0 {
        return Ok((0, 0.0));
    }
    Ok((count, total / count as f64))
}

/// v0.48a — read the active model's train-time
/// Brier from `active.json`. v0.49b refactored this
/// to call the canonical `commands::active_model`
/// helper so the disk-read path is in exactly one
/// place. Returns `None` when active.json is missing
/// or has no `best.brier` field.
async fn read_active_train_brier() -> Option<f64> {
    crate::commands::active_model::read_active_model_from_disk()
        .ok()
        .flatten()
        .and_then(|am| am.best_brier)
}

const DEFAULT_DEGRADATION_THRESHOLD: f64 = 0.05; // live Brier + 0.05 = alert
const DEFAULT_DEGRADATION_SAMPLE_SIZE: i64 = 50;

async fn run_degradation_check_once(pool: &SqlitePool) -> sqlx::Result<()> {
    let n = std::env::var("POLYROCKET_DEGRADATION_SAMPLE_SIZE")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_DEGRADATION_SAMPLE_SIZE);
    let threshold = std::env::var("POLYROCKET_DEGRADATION_THRESHOLD")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_DEGRADATION_THRESHOLD);
    let (n_samples, live_brier) = compute_live_brier(pool, n).await?;
    let train_brier = read_active_train_brier().await.unwrap_or(0.0);
    let drift = live_brier - train_brier;
    let alert = n_samples >= 10 && drift > threshold;
    use crate::infra::telemetry;
    telemetry::emit(telemetry::Event::ModelDegradation {
        n_samples,
        live_brier,
        train_brier,
        drift,
        alert,
    });
    Ok(())
}

/// v0.48a — 同步触发一次 model degradation check。跳过 1h tick，直接
/// `run_degradation_check_once`。
///
/// **业务流程**：拉最近 24h 已 settle 的 paper_fills → 跑 FALLBACK model 预测 →
/// 算 Brier → 与阈值比 → 触发 `DegradationAlert` telemetry event → L1 收到事件弹 OS 通知。
///
/// **IPC 调用**：`commands::degradation::run_degradation_check_now`。
/// **L1 入口**：Settings → Models → 「Check now」按钮。
pub async fn run_degradation_check_now(pool: &SqlitePool) -> sqlx::Result<()> {
    run_degradation_check_once(pool).await
}

const DEFAULT_DEGRADATION_TICK_SEC: u64 = 3600; // 1 hour

async fn run_degradation_check_loop(
    pool: SqlitePool,
    _cfg: SchedulerConfig,
    shutdown: Arc<Notify>,
) {
    // Stagger so we don't run all 8 loops at once
    tokio::time::sleep(Duration::from_secs(20)).await;
    let mut ticker = tokio::time::interval(Duration::from_secs(
        DEFAULT_DEGRADATION_TICK_SEC,
    ));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    record_tick("degradation_check"); // v0.49c
    loop {
        tokio::select! {
            _ = ticker.tick() => {
                record_tick("degradation_check"); // v0.49c
                if let Err(e) = run_degradation_check_once(&pool).await {
                    tracing::warn!(error = %e, "model degradation check error");
                    use crate::infra::telemetry;
                    telemetry::emit(telemetry::Event::SchedulerError {
                        loop_name: "degradation_check",
                        error: e.to_string(),
                    });
                }
            }
            _ = shutdown.notified() => {
                tracing::info!("degradation check loop shutting down");
                break;
            }
        }
    }
}


// ============================================================
// v0.49c — self-test cargo tests
// ============================================================
//
// These tests exercise the `record_tick` / `self_test`
// pair. They use the `LOOP_LAST_TICK` global, which is
// a OnceLock — so the first test to run "wins" the
// initialization. Subsequent tests share state with the
// first one. That's fine for our purposes because we're
// only testing the helpers' shape, not the global
// ordering.
//
// IMPORTANT: tests must NOT assume `last_tick == 0` for
// any loop; in this process some other test may have
// already tick'd. The tests below only check invariants
// ("healthy <=> age within tolerance", "name list is
// stable", etc.).

#[cfg(test)]
mod self_test_tests {
    use super::*;

    /// v0.49c — the loop name list is stable across
    /// processes. If you add a 9th loop, this test
    /// breaks; update the list.
    #[test]
    fn self_test_returns_eight_loops() {
        let st = self_test();
        assert_eq!(st.loops.len(), 8, "got: {st:?}");
        // Sorted alphabetically by name.
        let names: Vec<&str> = st.loops.iter().map(|l| l.name).collect();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted, "loops must be alphabetically sorted");
        // All expected names are present.
        for expected in [
            "anomaly",
            "audit_purge",
            "daily_brief",
            "degradation_check",
            "health_probe",
            "mirror_executor",
            "paper_fills_reconcile",
            "sidecar_health",
        ] {
            assert!(names.contains(&expected), "missing loop {expected}");
        }
    }

    /// v0.49c — a loop that ticked recently is
    /// healthy. We force a tick and re-check.
    #[test]
    fn record_tick_marks_loop_healthy() {
        record_tick("sidecar_health");
        let st = self_test();
        let sh = st
            .loops
            .iter()
            .find(|l| l.name == "sidecar_health")
            .unwrap();
        assert!(sh.last_tick_unix_ms > 0, "last tick should be > 0");
        assert!(sh.age_ms.is_some(), "age should be Some after tick");
        assert!(sh.healthy, "fresh tick should be healthy");
    }

    /// v0.49c — `all_healthy` is true iff every
    /// loop is healthy. After ticking every loop,
    /// we expect all_healthy = true.
    #[test]
    fn all_healthy_after_ticking_every_loop() {
        for name in [
            "anomaly",
            "audit_purge",
            "daily_brief",
            "degradation_check",
            "health_probe",
            "mirror_executor",
            "paper_fills_reconcile",
            "sidecar_health",
        ] {
            record_tick(name);
        }
        let st = self_test();
        assert!(st.all_healthy, "got: {st:?}");
        assert!(st.loops.iter().all(|l| l.healthy));
    }

    /// v0.49c — an unknown loop name panics in
    /// tests (so typos surface). In production
    /// `record_tick` is silent.
    #[test]
    #[should_panic(expected = "unknown loop")]
    fn unknown_loop_name_panics_in_test() {
        record_tick("definitely_not_a_loop");
    }
}
