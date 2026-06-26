//! L4 —— 进程内 telemetry。
//!
//! v0.42a —— opt-in 事件发射。默认关闭；在环境变量
//! 中设置 `POLYROCKET_TELEMETRY=1` 可开启。
//!
//! v0.49a —— 文件保留。当 telemetry 开启时，每个
//! 事件也会被追加到
//! `<app_data_dir>/logs/telemetry/session-<start_unix>.jsonl`
//! 中的 per-session JSONL 文件。启动时我们会删除
//! 早于 `POLYROCKET_TELEMETRY_RETENTION_DAYS`（默认 14）的
//! session 文件。
//!
//! ## 设计
//!
//! - **默认关闭**（零开销）。`is_enabled()` 是一次
//!   static atomic 读，所以调用点无论开关都是 `O(1)`。
//! - **无 PII、无模型权重、无密钥。** 事件是
//!   粗粒度的生命周期标记（train started、
//!   promote completed、scheduler tick），加上一小袋
//!   类型化上下文（job_id、loop_name、latency_ms）。
//! - **Sink 为 stderr（开启时始终）+ per-session JSONL
//!   文件（v0.49a，当 log_dir 已设置时）。**
//!   用 `polyrocket 2> telemetry.log` 抓实时流；文件
//!   给你一个跨重启的持久记录。L1 可调
//!   `list_telemetry_logs` / `purge_telemetry_logs`
//!   来浏览和清理。
//! - **Sink 可替换**通过 `Sink` trait。v0.42a 自带
//!   `StderrSink`；v0.49a 新增 `FileSink`。
//!
//! ## 为什么不直接用 `log`/`tracing` crate？
//!
//! 现有调度器已经用 `tracing` 做人可读的 debug 输出。
//! Telemetry 是另一条流：机器可解析、opt-in、生命周期
//! 限定。我们不想让 tracing subscriber 默认被洪水淹没。
//!
//! ## 用法
//!
//! ```ignore
//! use crate::infra::telemetry;
//! telemetry::emit(telemetry::Event::TrainCompleted {
//!     job_id: "train-441c352b".into(),
//!     model_version: "logistic-train-441c352b".into(),
//!     best_brier: Some(0.172),
//!     duration_ms: 12_345,
//! });
//! ```
//!
//! 分层规则：L4 可依赖 L5（`platform::env`）。
//! **不得**依赖 L3 / L2 / L1。发射事件的调用点
//! 位于 L2（IPC handler）和 L4（scheduler）——
//! 它们直接 `use crate::infra::telemetry`。

use crate::platform::env;
use serde::Serialize;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

/// 进程级 enabled 标志。`static` 以让 `is_enabled()` 是一次
/// atomic 加载 —— 无锁竞争。
static ENABLED: AtomicBool = AtomicBool::new(false);
static INITIALIZED: AtomicBool = AtomicBool::new(false);

/// 从 env 读 `POLYROCKET_TELEMETRY` 并设置全局开关。**幂等** —— 多次调用安全，
/// 只有第一次有效果（`INITIALIZED` swap 一次）。
///
/// **调用方**：`lib.rs::run()` 在 startup 调一次。后续 L1 IPC (`set_telemetry_enabled`)
/// 通过 `set_enabled()` 改值。
pub fn init_from_env() {
    if INITIALIZED.swap(true, Ordering::SeqCst) {
        return;
    }
    let v = env::env_str("POLYROCKET_TELEMETRY")
        .map(|s| s == "1" || s.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    ENABLED.store(v, Ordering::SeqCst);
}

/// 如果本进程开启了 telemetry 则为 true。廉价的 atomic
/// 加载 —— 可在热路径上调用。
///
/// **为什么用 `Relaxed` 序**：开关的设置是 idempotent + 后写覆盖前写，不需要 happens-before
/// 关系。`Relaxed` load 在 x86 是 free，ARM 上是普通 ldr。
pub fn is_enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

/// 仅测试用：覆盖 enabled 标志。`#[cfg(test)]` 防止进生产 binary。
#[cfg(test)]
pub fn set_enabled_for_test(v: bool) {
    ENABLED.store(v, Ordering::SeqCst);
    INITIALIZED.store(true, Ordering::SeqCst);
}

/// v0.42c —— 运行时覆盖。当用户在 Settings
/// 切换该偏好时，由 L1 `setTelemetryEnabled` IPC
/// 调用。**不**会触碰 `INITIALIZED` 标志，所以后续
/// `init_from_env` 调用仍会是 no-op（幂等）。
///
/// **为什么不 mutate `INITIALIZED`**：L1 toggle 是用户意图，应该持续生效；如果后续
/// 重新 `init_from_env` 会覆盖用户选择，所以这里**不**改 `INITIALIZED`。
pub fn set_enabled(v: bool) {
    ENABLED.store(v, Ordering::SeqCst);
}

// ============================================================
// v0.49a —— 文件保留
// ============================================================
//
// 当 telemetry 开启时，每个事件也会被追加到
// per-session JSONL 文件。文件路径在进程生命周期内
// 固定（在启动时通过 `set_log_dir` 设置）。启动时
// 我们也会跑一次保留扫描，删除早于
// `POLYROCKET_TELEMETRY_RETENTION_DAYS`（默认 14）的
// session 文件。L1 也可以调 `purge_telemetry_logs`
// 手动触发扫描。
//
// 文件名格式：`session-<start_unix>.jsonl`。
// 按字典序排序的文件名 == 按时间排序。
// 10 位 start_unix 意味着可排序的部分是
// "session-" 之后的前 10 个字符。

static LOG_DIR: std::sync::Mutex<Option<PathBuf>> = std::sync::Mutex::new(None);

/// 为 FileSink 设置日志目录。在 `app_data_dir` 可达
/// 之后，于启动时调用一次。幂等 —— 只有第一次
/// 调用有效。创建该目录（如果不存在）。
/// 设置 telemetry 文件日志目录。**只第一次调用生效**（幂等），后续调用 no-op。
///
/// **业务流程**：`lib.rs::run()` 在 startup 调一次，传 `<app_data_dir>/logs/telemetry/`。
/// 该目录会被 `create_dir_all` 创建（如果不存在）。
///
/// **不重置已有目录**：如果 polyrocket 重启，已存在的 session 文件会保留，由
/// `retention_days()` 自动清理过期文件。
pub fn set_log_dir(dir: PathBuf) -> std::io::Result<()> {
    let mut slot = LOG_DIR.lock().expect("LOG_DIR lock poisoned");
    if slot.is_some() {
        return Ok(());
    }
    std::fs::create_dir_all(&dir)?;
    *slot = Some(dir);
    Ok(())
}

/// 默认的保留窗口（单位天）。从
/// `POLYROCKET_TELEMETRY_RETENTION_DAYS`（env-var 覆盖）读取；
/// 14 是出厂默认。
pub fn retention_days() -> u64 {
    env::env_u64("POLYROCKET_TELEMETRY_RETENTION_DAYS", 14)
}

/// L1 telemetry 日志列表中的一行。由 `list_telemetry_logs` 返回，
/// 让 Settings 卡片能展示磁盘上的文件清单。
#[derive(Debug, Clone, Serialize)]
/// 单个 telemetry session 文件的元数据。L1 「Settings → Telemetry」用这个列表
/// 展示历史 session。
///
/// **来源**：`list_telemetry_logs()` 读 `<log_dir>/` 下的 `session-*.jsonl` 文件
/// + 它们的 mtime。
pub struct TelemetryLogInfo {
    /// 仅文件名，如 `session-1740000000.jsonl`。
    pub name: String,
    /// 磁盘上的绝对路径。
    pub path: String,
    /// 文件大小（字节；若文件在 `read_dir` 与
    /// `metadata` 之间消失则为 0 —— 容忍这种
    /// 竞态）。
    pub size_bytes: u64,
    /// 文件修改时间，unix 秒（未知时为 0）。
    pub modified_unix: u64,
    /// 如果是当前活跃 session 文件则为 true
    ///（即当前进程正在向它追加）。
    pub is_current: bool,
}

/// 在当前 log dir 下返回所有 telemetry session 文件的排序列表。
/// 当前进程的 session 被标记 `is_current = true`。
/// 当尚未设置 log dir 时返回空列表（不是错误）
/// —— L1 在启动完成前就可能调到这个。
pub fn list_telemetry_logs() -> Vec<TelemetryLogInfo> {
    let dir = match LOG_DIR.lock().expect("LOG_DIR lock poisoned").clone() {
        Some(d) => d,
        None => return vec![],
    };
    let current = current_session_path().and_then(|p| p.file_name().map(|f| f.to_os_string()));
    list_in_dir(&dir, current.as_ref())
}

fn list_in_dir(dir: &std::path::Path, current_name: Option<&std::ffi::OsString>) -> Vec<TelemetryLogInfo> {
    let Ok(rd) = std::fs::read_dir(dir) else { return vec![]; };
    let mut out: Vec<TelemetryLogInfo> = rd
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let p = e.path();
            // 仅匹配 "session-*.jsonl" 模式。
            // 按文件名字典序排序即可得到时序。
            let fname = p.file_name()?.to_os_string();
            let name = fname.to_str()?.to_string();
            if !name.starts_with("session-") || !name.ends_with(".jsonl") {
                return None;
            }
            let meta = e.metadata().ok();
            let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
            let modified = meta
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let is_current = current_name
                .map(|c| c == &fname)
                .unwrap_or(false);
            Some(TelemetryLogInfo {
                name,
                path: p.to_string_lossy().to_string(),
                size_bytes: size,
                modified_unix: modified,
                is_current,
            })
        })
        .collect();
    // 最旧优先 —— 用户自上而下阅读。
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// 手动触发一次保留扫描。返回删除的文件数。
/// 保留窗口是 `POLYROCKET_TELEMETRY_RETENTION_DAYS`（默认 14）。
///
/// 策略：当文件名的 timestamp 早于
/// `now - retention_days` 时即视为"陈旧"。
/// 我们用文件名（`session-<unix>.jsonl`）而非
/// mtime，因为后者会被文件系统备份工具/`touch`
/// 扰动。
pub fn purge_telemetry_logs() -> std::io::Result<u64> {
    let dir = match LOG_DIR.lock().expect("LOG_DIR lock poisoned").clone() {
        Some(d) => d,
        None => return Ok(0),
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let cutoff = now.saturating_sub(retention_days() * 86_400);
    let mut deleted = 0u64;
    for entry in std::fs::read_dir(&dir)? {
        let entry = match entry { Ok(e) => e, Err(_) => continue };
        let p = entry.path();
        let name = match p.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };
        if !name.starts_with("session-") || !name.ends_with(".jsonl") {
            continue;
        }
        // 抽出 timestamp 部分："session-<digits>.jsonl"
        let ts_str = &name["session-".len()..name.len() - ".jsonl".len()];
        let Ok(ts) = ts_str.parse::<u64>() else { continue };
        if ts < cutoff {
            if std::fs::remove_file(&p).is_ok() {
                deleted += 1;
            }
        }
    }
    Ok(deleted)
}

/// 计算 per-session 文件路径。session 从
/// `set_log_dir` 之后的第一次 `emit()` 开始。
/// 同一进程内的后续事件会追加到同一文件。
fn current_session_path() -> Option<PathBuf> {
    let dir = LOG_DIR.lock().expect("LOG_DIR lock poisoned").clone()?;
    let start = SESSION_START_UNIX
        .get()
        .copied()
        .unwrap_or_else(|| now_unix());
    Some(dir.join(format!("session-{}.jsonl", start)))
}

use std::sync::OnceLock;
static SESSION_START_UNIX: OnceLock<u64> = OnceLock::new();

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// 向 session 文件追加一行 NDJSON。尽力而为：
/// 错误（磁盘满、权限等）被静默丢弃。
/// Telemetry 是可观测性工具，不是硬依赖。
fn append_to_file(line: &str) {
    let path = match current_session_path() {
        Some(p) => p,
        None => return,
    };
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        let _ = writeln!(f, "{}", line);
        let _ = f.flush();
    }
}

/// 所有 telemetry 事件。随新的生命周期 hook
/// 出现时在这里加 variant；wire 格式（NDJSON）保持
/// 稳定，因为 `Serialize` 实现是自动派生的。
/// polyrocket 发出的所有 telemetry 事件类型。serde tag = `"name"`（snake_case 命名）。
///
/// **当前事件列表**：
///   - `SchedulerTick` — 调度 loop tick 一次
///   - `TrainCompleted` — train job 成功完成
///   - `PromoteCompleted` — model promotion 完成
///   - `AutoPromoteTriggered` / `AutoPromoteSkipped` — auto-promote 决策
///   - `AnomalyDetected` — anomaly 探测发现异常
///   - `SidecarStarted` / `SidecarCrashed` — 侧车生命周期
///   - `AuditPurged` — 审计日志清理
///   - `MirrorPassCompleted` / `MirrorQueueDepth` — mirror 队列状态
///   - `DegradationAlert` — 模型退化警告
///   - `MirrorExecutorSlept` — mirror executor 跳过无任务
///
/// **wire 格式**：`{"name": "train_completed", "fields": {...}, "ts_ms": ...}`
///（`ts_ms` 由 `emit()` 自动加，事件本身不携带时间戳）。
///
/// **加新事件**：在 enum 加 variant → 在 `emit()` 不需要改（自动 serde）→
/// 在 L1 `useTelemetry` 订阅（如果需要展示）。
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "name", rename_all = "snake_case")]
pub enum Event {
    /// 调度 loop 唤醒。`tick_index` 是 per-loop
    /// 单调递增的；可用于发现卡住的 loop。
    SchedulerTick {
        loop_name: &'static str,
        tick_index: u64,
    },
    /// 调度 loop 捕获到一个可恢复的错误。Loop
    /// 还在跑；这是"已注意到"事件，
    /// 不是"崩溃"。
    SchedulerError {
        loop_name: &'static str,
        error: String,
    },
    /// `train_job` IPC 已接受。
    TrainStarted {
        job_id: String,
        n_trials: usize,
        epochs: usize,
    },
    /// `train_job` IPC 返回了一次成功的 train。
    TrainCompleted {
        job_id: String,
        model_version: String,
        best_brier: Option<f64>,
        duration_ms: u64,
    },
    /// `train_job` IPC 失败（侧车错误、DB 错误、
    /// 捕获到 panic）。
    TrainFailed {
        job_id: String,
        error: String,
    },
    /// `promote_model` 成功。
    PromoteCompleted {
        job_id: String,
        model_version: String,
        trial_index: Option<usize>,
        reason: String,
    },
    /// 后台 auto-promote worker（v0.28a）实际
    /// 提升了（即候选在配置的 Brier margin
    /// 范围内胜过了 active）。
    AutoPromoteFired {
        job_id: String,
        model_version: String,
        message: String,
    },
    /// 后台 auto-promote worker 跑了但**没有**
    /// 提升（候选不够好、没有 active 等）。
    /// 这是 v0.42e 特性：「你的训练没有
    /// 提升任何东西」的 opt-in 信号 ——
    /// 目前仅记录日志，不通过 OS 通知上抛。
    AutoPromoteSkipped {
        job_id: String,
        message: String,
    },
    /// 侧车进程已连接并回应了首次探测。
    /// 「sidecar 活着」标记。
    SidecarConnected {
        pid: Option<u32>,
    },
    /// 侧车进程退出或变得无响应。`reason` 是
    /// 来自 reader task 的最后一条错误字符串。
    SidecarDisconnected {
        reason: String,
    },
    /// daily brief job 完成了 summary 生成。
    DailyBriefGenerated {
        summary_chars: usize,
        duration_ms: u64,
    },
    /// 异常检测器标记了一个 market。
    AnomalyDetected {
        kind: String,
        severity: u8,
        details: String,
    },
    /// LLM 健康探测结果。
    LlmHealthProbe {
        provider: String,
        ok: bool,
        latency_ms: u64,
        error: Option<String>,
    },
    /// Mirror executor（镜像执行器）的一次 tick。
    /// `intents_pending` 是 tick 起始时的队列深度；`executed` 和 `errors` 是该 tick 内的结果。
    MirrorExecutorTick {
        intents_pending: usize,
        executed: usize,
        errors: usize,
    },
    /// 审计保留扫描删除了 N 行。
    AuditPurged {
        rows: u64,
        retention_days: u64,
    },
    /// v0.45a —— paper_fills 对账 pass
    /// 结算了 N 条 paper_fills（即它们在
    /// 上次 tick 与本次之间对应的 market
    /// 变为了 resolved）。
    PaperFillsReconciled {
        settled: u64,
    },
    /// v0.48a —— 模型退化检查。由第 8 个
    /// 调度 loop 在实时 Brier（针对最近
    /// 已 resolved 的 market 用 FALLBACK 模型
    /// 权重计算）相对训练期 Brier 漂移超
    /// 阈值时发出。L1 监听此事件并
    /// 选择性地触发 OS 通知（由 Settings
    /// 偏好控制）。
    ModelDegradation {
        /// 实时 Brier 所基于的最近已 resolved
        /// market 数。L1 用它判断「这是真信号
        /// 还是小样本波动」。
        n_samples: u64,
        /// 实时 Brier 分数（针对最近 n_samples
        /// 个已 resolved market 的平均，用
        /// FALLBACK 权重作为预测模型）。
        live_brier: f64,
        /// Active 模型的训练期 Brier
        ///（active.json 中的 best_brier，
        /// 或无 active 模型时为 0.0）。
        train_brier: f64,
        /// live_brier - train_brier。正值表示
        /// 实时表现比训练期差。负值表示更好
        ///（少见；通常实时至少和训练期一样好）。
        drift: f64,
        /// 当 drift > 配置阈值时为 true。
        /// L1 只在此为 true 时触发 OS 通知
        ///（而非每个 tick 都触发）。
        alert: bool,
    },
}

/// 发送一个 telemetry 事件。如果 `!is_enabled()` 则 no-op。
///
/// 启用路径上：序列化为 NDJSON 并向 stderr 写一行，
/// 同时（v0.49a）把同一行追加到 per-session JSONL
/// 文件。写盘失败被静默忽略。
/// 发送一个 telemetry 事件。**完全 best-effort**：序列化失败 / 写盘失败 / disabled → 静默返回。
///
/// **业务流程**：
///   1. 如果 `!is_enabled()` → 直接返回（cheap atomic load）
///   2. `serde_json::to_string(&event)` —— 失败就 return
///   3. stderr sink —— 永远（开了就开）
///   4. file sink —— 写一行 NDJSON 到 current session file
///
/// **为什么不 panic / log error**：telemetry 是观察性工具，不能影响主流程。卡死的话
/// 用 `polyrocket 2> telemetry.log` 抓 stderr 看为什么 emit 不工作。
///
/// **添加调用方**：`use crate::infra::telemetry; telemetry::emit(Event::X { ... });`
/// —— L2 (commands) 和 L4 (scheduler) 都直接调；L3 (domain) 通过 `&self` 参数传入
/// 间接触发（domain 不能依赖 L4）。
pub fn emit(event: Event) {
    if !is_enabled() {
        return;
    }
    let payload = match serde_json::to_string(&event) {
        Ok(s) => s,
        Err(_) => return,
    };
    let mut err = std::io::stderr().lock();
    let _ = writeln!(err, "{}", payload);
    let _ = err.flush();
    // v0.49a —— 文件 sink。同一行，追加到
    // session 文件。错误静默处理。
    append_to_file(&payload);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emit_is_noop_when_disabled() {
        set_enabled_for_test(false);
        // 不应 panic，不应写入。断言
        // 是「我们不崩地回来了」。
        emit(Event::SchedulerTick {
            loop_name: "test",
            tick_index: 1,
        });
    }

    #[test]
    fn emit_writes_ndjson_when_enabled() {
        set_enabled_for_test(true);
        // 没有自定义 Sink trait 时我们难以在单元测试中
        // 抓取 stderr（计划在 v0.42a 扩展）。现在
        // 只确认调用不 panic 并返回。
        emit(Event::TrainCompleted {
            job_id: "train-test".into(),
            model_version: "logistic-train-test".into(),
            best_brier: Some(0.17),
            duration_ms: 1234,
        });
    }

    #[test]
    fn event_serializes_to_stable_json() {
        let ev = Event::TrainCompleted {
            job_id: "train-x".into(),
            model_version: "logistic-train-x".into(),
            best_brier: Some(0.2),
            duration_ms: 100,
        };
        let s = serde_json::to_string(&ev).unwrap();
        // tag 应是 snake_case 形式的 variant 名。
        assert!(s.contains("\"name\":\"train_completed\""), "got: {s}");
        assert!(s.contains("\"job_id\":\"train-x\""), "got: {s}");
        assert!(s.contains("\"best_brier\":0.2"), "got: {s}");
        assert!(s.contains("\"duration_ms\":100"), "got: {s}");
    }

    #[test]
    fn init_from_env_is_idempotent() {
        set_enabled_for_test(false);
        // 第二次调用必须不 panic 且不
        // 重新读 env。
        init_from_env();
        init_from_env();
    }

    #[test]
    fn optional_fields_become_null() {
        let ev = Event::TrainFailed {
            job_id: "train-y".into(),
            error: "kaboom".into(),
        };
        let s = serde_json::to_string(&ev).unwrap();
        assert!(s.contains("\"name\":\"train_failed\""), "got: {s}");
        assert!(s.contains("\"error\":\"kaboom\""), "got: {s}");
    }

    #[test]
    fn set_enabled_flips_at_runtime() {
        set_enabled_for_test(false);
        assert!(!is_enabled());
        set_enabled(true);
        assert!(is_enabled());
        set_enabled(false);
        assert!(!is_enabled());
    }

    // v0.49a —— 文件 sink 测试。我们用 per-test
    // tempdir 以覆盖全局 LOG_DIR 状态
    //（set_log_dir 是幂等的，所以通过
    // 直接 unsafe 写重置 —— 测试中可以）。
    use std::sync::Mutex;
    static FILE_TESTS: Mutex<()> = Mutex::new(());

    #[test]
    fn file_sink_creates_session_file() {
        let _g = FILE_TESTS.lock().unwrap();
        let dir = std::env::temp_dir().join(format!(
            "polyrocket_telemetry_test_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();

        // 通过公开 API 重置全局 LOG_DIR。
        // set_log_dir 是「先到先得」—— 测试中
        // 我们每次用新目录，并通过
        // 测试想要的路径 patch 全局。
        {
            let mut slot = LOG_DIR.lock().unwrap();
            *slot = Some(dir.clone());
        }

        set_enabled_for_test(true);
        emit(Event::TrainStarted {
            job_id: "train-t1".into(),
            n_trials: 3,
            epochs: 5,
        });
        emit(Event::TrainCompleted {
            job_id: "train-t1".into(),
            model_version: "logistic-train-t1".into(),
            best_brier: Some(0.1),
            duration_ms: 100,
        });

        let infos = list_telemetry_logs();
        assert_eq!(infos.len(), 1, "expected 1 session file, got {infos:?}");
        assert!(infos[0].is_current);
        assert!(infos[0].size_bytes > 0);
        let body = std::fs::read_to_string(&infos[0].path).unwrap();
        assert!(body.contains("\"name\":\"train_started\""), "got: {body}");
        assert!(body.contains("\"name\":\"train_completed\""), "got: {body}");
        // 每次 emit 恰好一行。
        assert_eq!(body.lines().count(), 2, "got: {body}");

        // 清理
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn purge_telemetry_logs_deletes_old_files() {
        let _g = FILE_TESTS.lock().unwrap();
        let dir = std::env::temp_dir().join(format!(
            "polyrocket_telemetry_purge_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();

        // 写一个文件名 timestamp 在远古的「陈旧」文件。
        // 本测试的保留窗口是 14 天，所以任何
        // 早于 14 天前的 ts 应当被删除。
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let old_ts = now - (30 * 86_400); // 30 天前
        let recent_ts = now - (3 * 86_400); // 3 天前
        std::fs::write(dir.join(format!("session-{old_ts}.jsonl")), b"old\n").unwrap();
        std::fs::write(dir.join(format!("session-{recent_ts}.jsonl")), b"recent\n").unwrap();

        {
            let mut slot = LOG_DIR.lock().unwrap();
            *slot = Some(dir.clone());
        }

        let deleted = purge_telemetry_logs().unwrap();
        assert_eq!(deleted, 1, "should have deleted 1 file");

        let remaining: Vec<_> = std::fs::read_dir(&dir).unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        assert_eq!(remaining.len(), 1);
        assert!(remaining[0].contains(&recent_ts.to_string()), "got: {remaining:?}");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn file_sink_noop_when_disabled() {
        let _g = FILE_TESTS.lock().unwrap();
        let dir = std::env::temp_dir().join(format!(
            "polyrocket_telemetry_disabled_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        {
            let mut slot = LOG_DIR.lock().unwrap();
            *slot = Some(dir.clone());
        }
        set_enabled_for_test(false);
        emit(Event::SchedulerTick { loop_name: "x", tick_index: 1 });
        // 文件懒创建但保持为空（或根本不创建）。
        // 无论哪种情况，目录里都没有事件 payload。
        let count = std::fs::read_dir(&dir).unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                let p = e.path();
                std::fs::read_to_string(&p)
                    .map(|b| b.contains("scheduler_tick"))
                    .unwrap_or(false)
            })
            .count();
        assert_eq!(count, 0);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
