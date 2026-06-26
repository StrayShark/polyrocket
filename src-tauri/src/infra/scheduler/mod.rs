//! L4 —— 后台调度器 —— 健康探测 + 每日简报 cron + 异常检测。
//!
//! 三个都以 `tokio::spawn` 任务运行,在 `lib.rs::run()` 的 setup 中启动。
//! 每个任务自包含,DB / 网络错误能自我恢复,
//! 不会让进程崩溃。
//!
//! 生命周期
//! ---------
//! - `start(pool, http)` 返回一个 [`SchedulerHandle`] 给调用方持有。
//!   drop handle 不会停止任务（它们跑进程级）—— handle
//!   仅提供 `shutdown()` 信号,供测试使用。
//!
//! 可调参数(环境变量)
//! -------------------
//! - `POLYROCKET_HEALTH_PROBE_INTERVAL_MIN` —— 默认 5
//! - `POLYROCKET_DAILY_BRIEF_HOUR_UTC`      —— 默认 0
//! - `POLYROCKET_DAILY_BRIEF_TZ_OFFSET_MIN` —— 默认 0(UTC)
//! - `POLYROCKET_ANOMALY_WINDOW_MIN`        —— 默认 60
//! - `POLYROCKET_TELEMETRY`                 —— 设为 1 时,也发布
//!   进程内事件(未来:Sentry)。
//!
//! v0.49c —— 启动时 self-test。每个 loop 在每个 tick
//! 上更新 `LOOP_LAST_TICK[loop_name]`。`self_test`
//! IPC 读取这些值来验证 7 个 loop 都还活着。
//! 如果某个 loop 的 tick 间隔超过预期 3 倍,
//! L1 表面会显示一条警告。
//!
//! 分层规则:本模块依赖 L3(`domain::llm`)和 L5
//!（`platform::keyring`）—— **不**得依赖 L1 或 L2。

use crate::platform::env::{env_u32, env_u64, env_i32};
use crate::platform::keyring;
use crate::domain::llm::{
    AnthropicClient, CostRate, CustomClient, DeepSeekClient, GoogleClient, LlmClient,
    OpenAIClient, ProviderKind,
};
// v0.42b —— opt-in 生命周期事件。默认关闭;参见
// `infra::telemetry` 了解环境变量开关与
// 稳定的 NDJSON wire 格式。
use crate::infra::telemetry;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;

// ============================================================
// v0.49c —— 每 loop 的 last-tick 注册表
// ============================================================
//
// 每个 loop 在每次迭代开头调用 `record_tick(loop_name, expected_interval)`。
// self-test IPC（`scheduler_self_test_now`）读取这些
// 来验证 8 个 loop 都还活着,且以预期 cadence tick。
//
// LOOP_LAST_TICK 是进程级的。key 是静态 `&'static str`
// 字面量（编译期保证）;值是最近一次 tick 的
// unix 毫秒。
//
// PROCESS_START_UNIX 在首次 `record_tick` 调用时设置一次。
// self-test 将其回传,让 L1 区分 "loop 从未 tick 过" 和
// "loop 启动时 tick 过一次之后死了"。

static PROCESS_START_UNIX: OnceLock<u64> = OnceLock::new();
static LOOP_LAST_TICK: OnceLock<HashMap<&'static str, AtomicU64>> = OnceLock::new();

fn loop_registry() -> &'static HashMap<&'static str, AtomicU64> {
    LOOP_LAST_TICK.get_or_init(|| {
        let mut m = HashMap::new();
        // 每个 loop_name 与 infra::scheduler
        // 中现有注释头里的变体一致 —— 保持这些
        // 字符串稳定;L1 verbatim 显示它们。
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
        // 未知 loop_name —— 在测试里 panic 暴露;
        // 生产环境我们只是 no-op,这样单点调用的
        // 笔误不会让 loop 崩溃。
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
    /// 最近一次 tick 的 Unix 毫秒。0 = 从未
    /// tick（仍处于初始 sleep 中）。
    pub last_tick_unix_ms: u64,
    /// 自上次 tick 起的毫秒数。若 loop 从未 tick 过则为 `null`。
    pub age_ms: Option<u64>,
    /// 当 `age_ms <= 3 * expected_interval_ms` 时为 true。
    /// L1 用来渲染绿/红点。
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
    /// 进程启动时的 Unix 秒。
    pub process_started_at_unix: u64,
    /// self-test 运行时的 Unix 毫秒。
    pub checked_at_unix_ms: u64,
    /// 每个 loop 都健康时为 true。
    pub all_healthy: bool,
    pub loops: Vec<LoopStatus>,
}

/// 运行 self-test。轻量:仅读 atomic
/// 计数器,无 IO。返回供 L1 Settings 卡片
/// 渲染的快照。
///
/// **预期 cadence**：每 `expected_interval` tick 一次,self-test 阈值是 `3 * interval`。
/// 也就是说，允许最多 3 次 sleep 失败后才标 unhealthy(容忍偶发的 GC pause / IO stall)。
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

    // 每个 loop 的预期 tick 间隔。来源于
    // 各 loop body 中的 tokio::time::sleep() 值。
    // 若修改 loop cadence,请同步这里。
    let expected_ms: &[(&str, u64)] = &[
        ("health_probe",         cfg.health_probe_interval.as_millis() as u64),
        ("daily_brief",          24 * 60 * 60 * 1000), // 每天一次
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
    // 稳定的显示顺序:按 name 排序。L1 按任意顺序
    // 读都行,这里让 JSON 利于做 diff。
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
    /// v0.6a —— mirror executor tick 间隔。默认 30s。
    pub mirror_tick: Duration,
}

impl SchedulerConfig {
    /// 从 env vars 构造 `SchedulerConfig`。**不**做参数校验(负数 duration、>23 hour 等
    /// 会原样传给 loop,由 loop 自己 ignore / panic)。
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
    /// tokio `Notify` —— `shutdown()` 触发一次 notify,所有 loop 在 `select!` 里退出。
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
        // v0.6a —— M5 auto-execution:轮询 mirror 队列,把 pending 的提交为 Mode B 注单
        let pool = pool.clone();
        let cfg = cfg.clone();
        let shutdown = shutdown.clone();
        tokio::spawn(async move {
            run_mirror_executor_loop(pool, cfg, shutdown).await;
        });
    }
    {
        // v0.8c —— 审计日志保留:每日清理旧行
        let pool = pool.clone();
        let cfg = cfg.clone();
        let shutdown = shutdown.clone();
        tokio::spawn(async move {
            run_audit_purge_loop(pool, cfg, shutdown).await;
        });
    }
    {
        // v0.10d —— 侧车健康探测:每 30s ping 一次,
        // 写一行 sidecar_health,每小时清理一次表。
        let pool = pool.clone();
        let cfg = cfg.clone();
        let shutdown = shutdown.clone();
        tokio::spawn(async move {
            run_sidecar_health_loop(pool, cfg, shutdown).await;
        });
    }
    {
        // v0.45a —— paper_fills 对账:每 5
        // 分钟,settle 所有 market 已 resolved 的
        // paper_fills。计算 won/lost + PnL。
        let pool = pool.clone();
        let cfg = cfg.clone();
        let shutdown = shutdown.clone();
        tokio::spawn(async move {
            run_paper_fills_reconcile_loop(pool, cfg, shutdown).await;
        });
    }
    {
        // v0.48a —— 模型劣化检查:每 1
        // 小时,基于最近已 resolved 的 market
        // 算 FALLBACK 模型的实时 Brier 并
        // 发出 telemetry 事件。L1 监听
        // alert 标志,并触发 OS 通知
        // (由 Settings 偏好控制)。
        let pool = pool.clone();
        let cfg = cfg.clone();
        let shutdown = shutdown.clone();
        tokio::spawn(async move {
            run_degradation_check_loop(pool, cfg, shutdown).await;
        });
    }
    {
        // Phase 1.5 —— 市场异常扫描器:周期扫描
        // 活跃的 football market,查找价格尖峰
        // (统计型)和均值回归机会,写入
        // `market_anomalies` 表供 UI 消费。
        let pool = pool.clone();
        let cfg = cfg.clone();
        let shutdown = shutdown.clone();
        tokio::spawn(async move {
            run_market_anomaly_loop(pool, cfg, shutdown).await;
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
// ============== 健康探测 loop ===============================
// =================================================================

async fn run_health_probe_loop(
    pool: SqlitePool,
    http: reqwest::Client,
    cfg: SchedulerConfig,
    shutdown: Arc<Notify>,
) {
    // 错开初始运行,避免冷启动时的惊群效应
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
    // 拉取已启用的 provider
    let providers: Vec<ProviderProbeRow> = sqlx::query_as(
        "SELECT id, display_name, api_base, default_model, timeout_ms, cost_per_1k_in, cost_per_1k_out, max_retries, key_alias
         FROM llm_providers WHERE enabled = 1",
    )
    .fetch_all(pool)
    .await?;
    tracing::debug!("health probe: {} enabled providers", providers.len());

    for p in providers {
        // 在独立 task 中跑探测,避免慢 provider 阻塞其他
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
            // 没有 secret —— 记录失败探测并更新健康
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
        // v0.111.1 / v0.113 / v0.114 —— health probe 不直接支持(需要 special secret)
        ProviderKind::ErnieNative | ProviderKind::Hunyuan | ProviderKind::Spark => {
            // Health probe 回退 —— 走 OpenAI 客户端(会失败但不会 panic,scheduler 看
            // 到 error 就 skip 这个 provider)。
            Arc::new(OpenAIClient::new("https://api.openai.com/v1"))
        }
    };
    // v0.119 —— health probe 必须至少包含一条 message。某些
    // provider(MiniMax 等)对空 `messages` 数组返回 HTTP 400
    //（"invalid params, messages is empty"）。改为发送一个
    // 最小的单 token 探测 prompt。
    let req = crate::domain::llm::CallRequest::new(&p.default_model)
        .system("health probe")
        .user("ok")
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
            // v0.42b —— 单 provider 健康探测结果。
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
            // 检查 3 次连续失败 → 自动禁用
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
        // v0.110 —— 国产 OpenAI 兼容大模型(5 个)
        "qwen" | "doubao" | "kimi" | "glm" | "MiniMax" => ProviderKind::OpenaiCompat,
        // v0.111 —— ERNIE 百度千帆(OpenAI 兼容 v2 endpoint)
        "ernie" => ProviderKind::OpenaiCompat,
        // v0.111.1 —— ERNIE 原生 AK/SK
        "ernie_native" => ProviderKind::ErnieNative,
        // v0.113 —— Hunyuan
        "hunyuan" => ProviderKind::Hunyuan,
        // v0.114 —— Spark
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
    // 3 次连续失败 → 自动禁用
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
// ============== 每日简报 cron ================================
// =================================================================

async fn run_daily_brief_loop(
    pool: SqlitePool,
    cfg: SchedulerConfig,
    shutdown: Arc<Notify>,
) {
    // 等到下一个 00:00 UTC(按 tz 偏移调整)后每日 tick
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
    // 把 tz 偏移应用到 "wall clock" 计算,再换回 UTC 时刻。
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
        // 今天的目标已过;调度到明天
        ((target + chrono::Duration::hours(24)) - now).num_seconds().max(1)
    };
    Duration::from_secs(secs as u64)
}

async fn run_daily_brief_once(pool: &SqlitePool) -> sqlx::Result<()> {
    let started = chrono::Utc::now().timestamp_millis();
    tracing::info!("daily brief job: starting");

    // 懒初始化:确保今日 row 存在。我们用一个简单
    // 启发式算 top-N —— v0.2 仅统计 active market 数量
    // 然后写一个桩。v0.3+ 会接入 M12 的完整评分公式。
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
    // v0.42b —— 发出 lifecycle 事件。"items_json"
    // 的大小是 "summary 大小" 的粗略代理。
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
// ============== 异常检测 loop ============================
// =================================================================

async fn run_anomaly_loop(
    pool: SqlitePool,
    _http: reqwest::Client,
    cfg: SchedulerConfig,
    shutdown: Arc<Notify>,
) {
    // 启动后错开 30s,让首次探测有数据
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
// ============== 镜像执行器 loop (v0.6a) =====================
// =================================================================

async fn run_mirror_executor_loop(pool: SqlitePool, cfg: SchedulerConfig, shutdown: Arc<Notify>) {
    // 启动后错开 10s,让其他 loop 先稳定
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
    // v0.42b —— 在 tick 开头捕获队列深度。
    // 下面 `into_iter` 会消耗原始 `rows`,
    // 所以我们先保存长度。
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
        // v0.42b —— 发出每次 tick 的统计。错误数
        // 是隐式的(picked.len() + rejected.len() vs
        // 总行数);我们用 "rejected" 数近似错误数。
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

    // 按 provider 维度:本窗口与上一窗口的调用数与限流命中数
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
        // 1. 限流激增
        if rl >= 5 { anomalies.push("rate_limit_spike"); }
        // 2. 成本激增(>3 倍上一窗口)
        if prev_cost > 0.0 && cost_v > prev_cost * 3.0 { anomalies.push("cost_spike"); }
        // 3. 零调用但原本活跃
        if calls == 0 && prev_calls > 0 { anomalies.push("zero_activity"); }
        // 4. 成功率突然下降
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
            // v0.42b —— 把每种不同的异常 kind 暴露为单独事件。
            // 严重度大致排序:rate_limit > cost_spike > zero >
            // low_success。我们挑选第一个出现的 kind
            // 以保持事件流低流量。
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
// ============== 公共辅助函数（供测试使用） ==================
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
// ============== v0.8c —— 审计日志清理 loop =====================
// =================================================================

use crate::domain::audit::RetentionPolicy;
use crate::infra::db::audit::purge_old;
use crate::infra::db::settings as app_settings;

const DEFAULT_AUDIT_PURGE_HOUR_UTC: u32 = 3; // 3 AM UTC
const DEFAULT_AUDIT_PURGE_TICK_SEC: u64 = 600; // 10 min — cheap check; only fires on the hour

/// 默认保留策略:90 天 / 5 万行上限 / 1k 行下限。
fn default_retention_policy() -> RetentionPolicy {
    RetentionPolicy::default()
}

/// v0.13c —— 读取用户保留策略,回退到硬编码默认。
/// 用户可从 Settings 覆盖任一字段;缺失的字段各自回退。
pub async fn read_user_retention(pool: &SqlitePool) -> sqlx::Result<RetentionPolicy> {
    app_settings::read_audit_retention(pool).await
}

/// v0.13c —— 写入用户保留覆盖。
pub async fn write_user_retention(
    pool: &SqlitePool,
    policy: &RetentionPolicy,
) -> sqlx::Result<()> {
    app_settings::write_audit_retention(pool, policy).await
}

async fn run_audit_purge_once(pool: &SqlitePool) -> sqlx::Result<usize> {
    let now = chrono::Utc::now().timestamp_millis();
    // v0.13c —— 读取用户保留策略。若无覆盖,
    // 回退到 `RetentionPolicy::default()`。
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
                // v0.42b —— 发出保留扫描结果。
                // retention_days 是近似的(从 ms 向下取整)。
                // n=0 时不发,保持低流量。
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
    // 启动时稍等,避免与其他 loop 抢占。
    tokio::time::sleep(Duration::from_secs(30)).await;
    let mut ticker = tokio::time::interval(Duration::from_secs(DEFAULT_AUDIT_PURGE_TICK_SEC));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let _ = DEFAULT_AUDIT_PURGE_HOUR_UTC; // 保留,供未来按小时门控触发
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
// ============== v0.10d —— 侧车健康探测 loop ===============
// =================================================================

use crate::domain::sidecar_health::SidecarHealthKind;
use crate::infra::db::sidecar_health as sh;

/// 指向运行中 Tauri AppHandle 的全局句柄,让调度器
/// 可以查找 managed state(例如 SidecarState)而无需
/// 经过 L2。在 `lib.rs::run()` 中调度器启动前设置。
pub static TAURI_APP: OnceLock<tauri::AppHandle> = OnceLock::new();

const DEFAULT_SIDECAR_PROBE_SEC: u64 = 30;
const DEFAULT_SIDECAR_PURGE_SEC: u64 = 3600;

async fn ping_sidecar_once() -> (SidecarHealthKind, Option<i64>, Option<String>) {
    // v0.11b —— 通过 L2 SidecarState 做真实探测。调度器
    // 不能直接访问它(5 层规则:L4 不 import L2),
    // 所以我们用 app handle 查。
    //
    // v0.12c —— 使用 async 包装(ping_async),它能与
    // `tokio::time::timeout` 和 `spawn_blocking`
    // 干净地组合,不用在这里手动管理 `spawn_blocking`。
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
    // 大约每小时清理一次,而不是每个 tick 都清理
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
    // 错开一些,避免冷启动时的争抢
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
                // v0.42b —— 发出 connect/disconnect 状态变化。
                // 我们用最近一次探测重新判断 kind;成本
                // 低,因为 ping_sidecar_once 很快,
                // 且我们已经写完 DB。状态变化为:
                // None -> Ok = 已连接;Ok -> Failed
                // = 已断开。
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
// ============== v0.45a —— paper_fills 对账 loop ==========
// =================================================================

/// v0.45a —— 对账 paper_fills 与 market
/// resolutions。对每个未结算的 paper_fill,
/// 当其 market 已 resolved 时,计算
/// won/lost 与 PnL,然后写入结算字段。
///
/// PnL 公式(与 `domain::bet` 一致):
///   - 若 paper_fill.side == market.outcome → won
///     pnl = size_shares * (1 - price)   // 买入 YES,market 结算为 YES
///   - 若 paper_fill.side != market.outcome → lost
///     pnl = -size_usdc                   // 买错了方向
///
/// 返回本次 pass 中已结算的 paper_fills 数。
async fn reconcile_paper_fills_once(pool: &SqlitePool) -> sqlx::Result<usize> {
    // 1. 找出未结算且 market 已 resolved
    //    的 paper_fills。join markets 以获取 outcome。
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
        // 查这个 paper_fill 对应的实际 market
        // outcome。我们按行查,因为上面的
        // join 只返回了 paper_fills 字段;
        // outcome 在 markets.outcome 中。
        let outcome: Option<String> = sqlx::query_scalar(
            "SELECT outcome FROM markets WHERE id = ?",
        )
        .bind(&_market_id)
        .fetch_optional(pool)
        .await?
        .flatten();
        let Some(outcome) = outcome else { continue };
        let won = side.to_uppercase() == outcome.to_uppercase();
        // size_str 是 USDC 字符串,如 "12.5"。
        // 我们把它视为 PnL 计算的成本基础。
        //（对于 price=0.5 且 outcome=YES 的二元
        // prediction market,你能拿到的 "shares" 是
        // size / 0.5 = 2 * size。但我们保持简单:
        // PnL 按 win/loss 取 +/- size_usdc,
        // 与 bet 表 v0.5d sign_order 桩所用的
        // 保守核算方式一致。）
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
    // 错开一些,避免冷启动时的争抢
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

    /// v0.45a —— reconcile_paper_fills_once 在
    /// market 已 resolved 时 settle 未结算的
    /// paper_fills。我们用 in-memory SQLite pool
    /// 让测试自包含。
    #[tokio::test]
    async fn reconcile_paper_fills_settles_resolved_markets() {
        use sqlx::sqlite::SqlitePoolOptions;
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect(":memory:")
            .await
            .unwrap();
        // 最小 schema:markets + paper_fills
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

        // 插入 3 个 market —— 一个 resolved YES,
        // 一个 resolved NO,一个未 resolved。
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
        // v0.48a —— FALLBACK 权重从
        // sidecar/predict.py 镜像。
        // 若 Python 中这些常量变了,
        // 本测试也要相应修改。预期值
        // 手工从 sigmoid 算出:
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
        // 创建 markets + price_snapshots
        // 表(SQL 是对两者的 LEFT JOIN;
        // 若没有这些表,查询会报错)。
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
        // 空:0 个 resolved market → n_samples=0, brier=0.0
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
        // m1:resolved YES,无 snapshot
        sqlx::query("INSERT INTO markets VALUES ('m1', 'cat', 'q1', 1, 'YES', 1_700_000_000_000)")
            .execute(&pool)
            .await
            .unwrap();
        // m2:resolved YES,带 0.7 的 snapshot
        sqlx::query("INSERT INTO markets VALUES ('m2', 'cat', 'q2', 1, 'YES', 1_700_000_000_000)")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO price_snapshots (market_id, captured_at, best_bid, best_ask, mid_price, spread) VALUES ('m2', 1_000, 0.65, 0.75, 0.7, 0.1)")
            .execute(&pool)
            .await
            .unwrap();
        // 仅 m2 计入。在 price=0.7 且 outcome=YES=1.0 下,
        // FALLBACK 预测值小于 0.5(因为 price 高时 z < 0),
        // Brier 即为平方误差。
        let (n, b) = compute_live_brier(&pool, 50).await.unwrap();
        assert_eq!(n, 1);
        assert!(b > 0.0); // 有 Brier
        assert!(b < 1.0); // 二元 outcome 时在 [0, 1] 内
    }
}

// =================================================================
// ============== v0.48a —— 模型劣化检测器 =============
// =================================================================

/// v0.48a —— 预测 FALLBACK 模型权重。
/// Python 侧车（`predict.py`）中
/// `_FALLBACK_W0 = -0.5`、`_FALLBACK_W1 = 2.0`、
/// `_FALLBACK_W2 = 0.4`。Rust 侧在此镜像,
/// 让 degradation loop 不必绕回侧车即可算 Brier。
/// 若 Python 改了这些,这里的常量也要改
///（由 2 行测试覆盖）。
const FALLBACK_W0: f64 = -0.5;
const FALLBACK_W1: f64 = 2.0;
const FALLBACK_W2: f64 = 0.4;
const HORIZON_NORM_HOURS: f64 = 168.0;

/// v0.48a —— 用 FALLBACK 权重做单样本预测。
/// 镜像侧车 predict.py 的 `predict_logic`。
fn fallback_predict(price: f64, market_age_hours: f64) -> f64 {
    let z = FALLBACK_W0 + FALLBACK_W1 * (1.0 - price) + FALLBACK_W2 * (market_age_hours / HORIZON_NORM_HOURS);
    if z >= 0.0 { 1.0 / (1.0 + (-z).exp()) } else { z.exp() / (1.0 + z.exp()) }
}

/// v0.48a —— 在最近 N 个带 price snapshot 的
/// resolved market 上算实时 Brier。返回
/// `(n_samples, live_brier)`。当没有
/// 带 snapshot 的 resolved market 时 n_samples=0。
async fn compute_live_brier(pool: &SqlitePool, n: i64) -> sqlx::Result<(u64, f64)> {
    // v0.48a —— join markets 与其最新
    // price_snapshots 行。我们使用与 v0.47b
    // backtest IPC 相同的相关子查询模式。
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
            _ => continue, // 跳过未知值
        };
        // 跳过没有 snapshot 的 market —— 没有价格
        // 就没法算出有意义的预测。（与 v0.47b backtest
        // IPC 同策略,但这里不回退到 0.5:
        // FALLBACK 权重在 mid_price=0.5 时会一直
        // 预测 0.5,即 "无信号" 基线。
        // 计入这些会稀释漂移信号。）
        let Some(price) = mid_price else { continue };
        // 使用 FALLBACK 约定:在收盘前 1 天预测。
        // "实时" age 用 market 的 end_date - now
        // 近似,上限 168h(horizon norm 常量)。
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

/// v0.48a —— 从 `active.json` 读取 active 模型的
/// 训练期 Brier。v0.49b 重构为调用
/// `commands::active_model` 规范 helper,
/// 让磁盘读路径只在一处。active.json 缺失
/// 或无 `best.brier` 字段时返回 `None`。
async fn read_active_train_brier() -> Option<f64> {
    crate::commands::active_model::read_active_model_from_disk()
        .ok()
        .flatten()
        .and_then(|am| am.best_brier)
}

const DEFAULT_DEGRADATION_THRESHOLD: f64 = 0.05; // 实时 Brier + 0.05 = 告警
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
    // 错开一些,避免 8 个 loop 同时跑
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
// Phase 1.5 —— 市场异常扫描器
// ============================================================

/// 默认扫描间隔:5 分钟。
const DEFAULT_MARKET_ANOMALY_TICK_SEC: u64 = 300;

/// 运行市场异常扫描器 loop。
///
/// 每 5 分钟,扫描所有活跃的 football market,查找:
///   1. 价格尖峰(统计型,z-score > 2.0)
///   2. 均值回归信号(|z-score| > 2.0)
///
/// 每个异常写入 `market_anomalies` 表。
async fn run_market_anomaly_loop(
    pool: SqlitePool,
    _cfg: SchedulerConfig,
    shutdown: Arc<Notify>,
) {
    tokio::time::sleep(Duration::from_secs(30)).await;
    let mut ticker = tokio::time::interval(Duration::from_secs(
        DEFAULT_MARKET_ANOMALY_TICK_SEC,
    ));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    record_tick("market_anomaly");
    loop {
        tokio::select! {
            _ = ticker.tick() => {
                record_tick("market_anomaly");
                if let Err(e) = scan_market_anomalies_once(&pool).await {
                    tracing::warn!(error = %e, "market anomaly scan error");
                    use crate::infra::telemetry;
                    telemetry::emit(telemetry::Event::SchedulerError {
                        loop_name: "market_anomaly",
                        error: e.to_string(),
                    });
                }
            }
            _ = shutdown.notified() => {
                tracing::info!("market anomaly loop shutting down");
                break;
            }
        }
    }
}

/// 扫描所有活跃的 football market,寻找价格异常。
///
/// 对每个有 ≥ 3 条 price snapshot 的 market,计算
/// 最新价格的统计 z-score。若 |z| > 2.0,写入一条
/// `market_anomaly` 行,kind 为 'reversion' 或 'price_spike'。
async fn scan_market_anomalies_once(pool: &SqlitePool) -> sqlx::Result<()> {
    use crate::domain::mean_reversion::{DEFAULT_WINDOW, DEFAULT_Z_THRESHOLD, detect_reversion};
    use crate::domain::spike::{DEFAULT_STAT_WINDOW, detect_spikes_statistical};

    // 拉取所有活跃的 football market 及其 price snapshot。
    let market_ids: Vec<(String,)> = sqlx::query_as(
        "SELECT DISTINCT ps.market_id
         FROM price_snapshots ps
         JOIN markets m ON m.id = ps.market_id
         WHERE m.active = 1 AND m.resolved = 0
           AND m.category = 'football'",
    )
    .fetch_all(pool)
    .await?;

    let now = chrono::Utc::now().timestamp_millis();

    for (market_id,) in &market_ids {
        // 拉取该 market 的 price snapshot。
        let snapshots: Vec<(i64, f64)> = sqlx::query_as(
            "SELECT captured_at, mid_price
             FROM price_snapshots
             WHERE market_id = ?
             ORDER BY captured_at ASC",
        )
        .bind(market_id)
        .fetch_all(pool)
        .await?;

        if snapshots.len() < 3 {
            continue;
        }

        let prices: Vec<f64> = snapshots.iter().map(|(_, p)| *p).collect();

        // —— 均值回归检查
        let signal = detect_reversion(
            market_id,
            &prices,
            DEFAULT_WINDOW,
            DEFAULT_Z_THRESHOLD,
            now,
        );

        if signal.stats.is_overextended {
            let kind = if signal.stats.z_score > 0.0 {
                "price_spike"
            } else {
                "reversion"
            };
            let severity = if signal.confidence > 0.75 {
                "high"
            } else if signal.confidence > 0.5 {
                "medium"
            } else {
                "low"
            };

            let payload = serde_json::json!({
                "z_score": signal.stats.z_score,
                "mean": signal.stats.mean,
                "current": signal.stats.current,
                "std_dev": signal.stats.std_dev,
                "fade_signal": signal.fade_signal,
                "confidence": signal.confidence,
                "direction": signal.stats.direction,
            });

            sqlx::query(
                "INSERT INTO market_anomalies
                    (market_id, kind, severity, payload, detected_at, notified)
                 VALUES (?, ?, ?, ?, ?, 0)",
            )
            .bind(market_id)
            .bind(kind)
            .bind(severity)
            .bind(payload.to_string())
            .bind(now)
            .execute(pool)
            .await?;
        }

        // —— 统计尖峰检测(基于 z-score,自适应)
        use crate::domain::spike::PriceSnapshotRow;
        let snap_rows: Vec<PriceSnapshotRow> = snapshots
            .iter()
            .enumerate()
            .map(|(i, (at, price))| PriceSnapshotRow {
                id: i as i64,
                market_id: market_id.clone(),
                captured_at: *at,
                mid_price: *price,
            })
            .collect();

        let stat_alerts = detect_spikes_statistical(&snap_rows, DEFAULT_STAT_WINDOW, DEFAULT_Z_THRESHOLD);
        for alert in stat_alerts {
            // 仅在尚未存在时插入(同 market_id + detected_at)
            let existing: Option<(i64,)> = sqlx::query_as(
                "SELECT id FROM market_anomalies
                 WHERE market_id = ? AND detected_at = ? AND kind = 'price_spike'",
            )
            .bind(market_id)
            .bind(alert.detected_at)
            .fetch_optional(pool)
            .await?;

            if existing.is_some() {
                continue;
            }

            let severity = if alert.change_pct.abs() > 15.0 {
                "high"
            } else if alert.change_pct.abs() > 8.0 {
                "medium"
            } else {
                "low"
            };

            let payload = serde_json::json!({
                "old_price": alert.old_price,
                "new_price": alert.new_price,
                "change_pct": alert.change_pct,
            });

            sqlx::query(
                "INSERT INTO market_anomalies
                    (market_id, kind, severity, payload, detected_at, notified)
                 VALUES (?, 'price_spike', ?, ?, ?, 0)",
            )
            .bind(market_id)
            .bind(severity)
            .bind(payload.to_string())
            .bind(alert.detected_at)
            .execute(pool)
            .await?;
        }
    }

    Ok(())
}


// ============================================================
// v0.49c —— self-test cargo 测试
// ============================================================
//
// 这些测试演练 `record_tick` / `self_test`
// 配对。它们使用 `LOOP_LAST_TICK` 全局,
// 是个 OnceLock —— 所以先跑的测试"赢"得
// 初始化。后面的测试与第一个共享状态。
// 对我们来说没问题,因为我们只测
// helper 形状,而不测全局顺序。
//
// 重要:测试**不能**假设任意 loop 的
// `last_tick == 0`;在本进程中
// 别的测试可能已经 tick 过了。
// 下面测试只检查不变量
//（"healthy <=> age 在容差内"、
// "name 列表稳定"等）。

#[cfg(test)]
mod self_test_tests {
    use super::*;

    /// v0.49c —— loop 名字列表跨进程稳定。
    /// 若加第 9 个 loop,本测试会失败;请更新列表。
    #[test]
    fn self_test_returns_eight_loops() {
        let st = self_test();
        assert_eq!(st.loops.len(), 8, "got: {st:?}");
        // 按 name 字母序排序。
        let names: Vec<&str> = st.loops.iter().map(|l| l.name).collect();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted, "loops must be alphabetically sorted");
        // 所有预期 name 都在。
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

    /// v0.49c —— 最近 tick 过的 loop 是健康的。
    /// 我们强制 tick 一次再检查。
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

    /// v0.49c —— `all_healthy` 当且仅当每个
    /// loop 都健康时为 true。tick 每个 loop
    /// 之后,期望 all_healthy = true。
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

    /// v0.49c —— 未知 loop 名在测试里 panic
    ///（让笔误暴露）。生产环境下
    /// `record_tick` 静默。
    #[test]
    #[should_panic(expected = "unknown loop")]
    fn unknown_loop_name_panics_in_test() {
        record_tick("definitely_not_a_loop");
    }
}
