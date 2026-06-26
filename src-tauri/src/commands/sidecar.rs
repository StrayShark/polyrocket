//! L2 —— Python 侧车（sidecar）进程管理器（M7）。
//!
//! 按需启动 `polyrocket-sidecar` Python 进程，发送 JSON-line 请求，
//! 并跟踪其健康状态。所有协议细节都在 `domain::lab::sidecar` 中；
//! 本模块负责与系统进程打交道。

use crate::AppError;
use crate::AppResult;
use crate::domain::lab::sidecar::{
    build_auto_promote_if_better_request, build_backtest_model_request, build_explain_model_request,
    build_predict_request, build_promote_all_trials_request, build_promote_request,
    build_rollback_request, build_shap_explain_request, build_train_request,
    parse_auto_promote_if_better_response, parse_backtest_model_response,
    parse_explain_model_response, parse_line, parse_list_promote_history_response,
    parse_promote_all_trials_response, parse_predict_response, parse_promote_response,
    parse_rollback_response, parse_shap_explain_response, parse_train_response,
    AutoPromoteIfBetterResult, BacktestResult, BacktestSample, ExplainResult, ExplainSample,
    Prediction, PromoteAllTrialsResult, PromoteHistoryResult, PromoteResult, RollbackResult,
    ShapResult, SidecarMethod, SidecarRequest, SidecarResponse, TrainResult, TrainTrial,
};
use crate::domain::lab::train_progress::{
    TrainFinishedEvent, TrainStartedEvent, TrainTrialDto,
};
use crate::infra::db;
use crate::platform::env::{is_sidecar_disabled, SIDECAR_DISABLED_MSG};
use serde::{Deserialize, Serialize};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, State};

/// v0.122a —— 侧车迁移版本的总开关。
///
/// 当 `POLYROCKET_DISABLE_SIDECAR=1` 时，每个侧车 IPC 入口
/// 在最开始调用本函数，并以规范的「已禁用」消息短路返回。
/// 调用方会看到携带 [`SIDECAR_DISABLED_MSG`] 的 `AppError`。
///
/// **进程控制**（`start_sidecar` / `stop_sidecar` / `sidecar_status`）
/// 故意不调用本函数 —— 即使处于禁用模式，用户也应能查看 / 重置
/// 侧车句柄。只有数据面 IPC 才受此开关约束。
fn check_sidecar_enabled() -> AppResult<()> {
    if is_sidecar_disabled() {
        Err(AppError::Internal(SIDECAR_DISABLED_MSG.to_string()))
    } else {
        Ok(())
    }
}

/// v0.122b —— 从 `active.json` 加载权重，产出推理层所需的
/// 三元组：(weights, model_version, brier_score)。当文件缺失
/// 或格式错误时回退到 fallback 权重（与 Python `active.py` 行为一致）。
///
/// 副作用：每次调用都会读磁盘。Python `active.py` 原本有 1 秒
/// mtime 缓存；Rust 调度器的 `degradation_check` 循环每 5 分钟
/// 调用一次，因此 I/O 开销可忽略。v0.122d（auto_promote）若
/// 性能剖析显示此处为热点，会加入进程内 mtime 缓存。
fn load_active_or_fallback() -> (
    crate::domain::lab::inference::InferenceWeights,
    String,
    Option<f64>,
) {
    use crate::commands::active_model::{active_model_path, read_active_model_from_disk};
    let path = active_model_path();
    let active = read_active_model_from_disk().ok().flatten();
    match active {
        Some(m) => {
            let weights = if let Some(w) = &m.weights {
                if w.len() >= 3 {
                    crate::domain::lab::inference::InferenceWeights {
                        w0: w[0],
                        w1: w[1],
                        w2: w[2],
                        horizon_norm_hours: 168.0,
                    }
                } else {
                    crate::domain::lab::inference::InferenceWeights::fallback()
                }
            } else {
                crate::domain::lab::inference::InferenceWeights::fallback()
            };
            (weights, m.model_version, m.best_brier)
        }
        None => {
            // 还没有 active.json（首次使用）。打日志让用户
            // 知道正在使用 fallback。
            tracing::info!(
                "v0.122b inference: no active.json at {}; using inline fallback weights",
                path.display()
            );
            (
                crate::domain::lab::inference::InferenceWeights::fallback(),
                "logistic-0.1.0".to_string(),
                None,
            )
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct SidecarStatus {
    pub running: bool,
    pub pid: Option<u32>,
    pub command: String,
    pub last_error: Option<String>,
}

impl Default for SidecarStatus {
    fn default() -> Self {
        Self {
            running: false,
            pid: None,
            command: std::env::var("POLYROCKET_SIDECAR_CMD")
                .unwrap_or_else(|_| "polyrocket-sidecar".into()),
            last_error: None,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct StartSidecarArgs {
    /// 侧车二进制 / python 脚本路径。默认
    /// 为 `POLYROCKET_SIDECAR_CMD` 环境变量或 "polyrocket-sidecar"。
    pub command: Option<String>,
    pub args: Option<Vec<String>>,
}

/// 将系统进程 + 管道封装为一个小型状态结构体。
///
/// v0.28a —— 字段均为 `Arc<Mutex<...>>`，便于以极低成本克隆
/// （每个字段一次 Arc 引用计数递增）。这样 `train_job` 中的
/// auto-promote worker 就能把侧车状态克隆到一个后台
/// `tokio::spawn` 任务中，而无需把原对象从 Tauri 的 `State` 中搬出。
#[derive(Clone)]
pub struct SidecarState {
    pub child: Arc<Mutex<Option<Child>>>,
    pub stdin: Arc<Mutex<Option<ChildStdin>>>,
    pub stdout: Arc<Mutex<Option<ChildStdout>>>,
    pub status: Arc<Mutex<SidecarStatus>>,
}

impl Default for SidecarState {
    fn default() -> Self {
        Self {
            child: Arc::new(Mutex::new(None)),
            stdin: Arc::new(Mutex::new(None)),
            stdout: Arc::new(Mutex::new(None)),
            status: Arc::new(Mutex::new(SidecarStatus::default())),
        }
    }
}

impl SidecarState {
    pub fn new() -> Self { Self::default() }

    pub fn is_running(&self) -> bool {
        self.status.lock().ok().map(|s| s.running).unwrap_or(false)
    }

    pub fn set_status(&self, status: SidecarStatus) {
        if let Ok(mut s) = self.status.lock() {
            *s = status;
        }
    }

    /// v0.11b —— 通过 stdin 向运行中的侧车发送 `ping`，
    /// 从 stdout 读取一行，命中后返回 `Ok(latency_ms)`。
    ///
    /// 这是一个同步辅助函数，供健康探测调度循环使用。
    /// 必须在阻塞上下文中调用（例如通过 `tokio::task::spawn_blocking`），
    /// 因为它在读取期间会持有 stdin + stdout 互斥锁。
    ///
    /// 返回值：
    ///   - `Ok(latency_ms)` —— 在超时时间内收到 `pong`
    ///   - `Err(String)` —— 其他情况（进程未运行、写 / 读失败、
    ///                       超时、解析错误等）
    ///
    /// 加锁约定：仅在写入时持有 stdin，仅在读取时持有 stdout。
    /// 这样其他代码路径（例如用户触发的 `sidecar_predict`）
    /// 就能在不发生死锁的前提下穿插自己的 I/O。
    pub fn ping_blocking(&self, timeout_ms: u64) -> Result<u64, String> {
        use std::io::{BufRead, BufReader, Write};
        use std::time::{Duration, Instant};

        // 构造 JSON-line 请求
        let id = format!("sweeper-{}", chrono::Utc::now().timestamp_millis());
        let payload = serde_json::json!({
            "id": id,
            "method": "ping",
            "params": {},
        });
        let line = serde_json::to_string(&payload).map_err(|e| format!("encode: {e}"))?;

        let started = Instant::now();
        let deadline = Duration::from_millis(timeout_ms);

        // 在 stdin 锁内写并 flush，写完即释放。
        {
            let mut stdin_guard = self.stdin.lock().map_err(|e| format!("stdin lock: {e}"))?;
            let stdin = stdin_guard.as_mut().ok_or_else(|| "stdin not available".to_string())?;
            if let Err(e) = writeln!(stdin, "{line}") {
                return Err(format!("write: {e}"));
            }
            if let Err(e) = stdin.flush() {
                return Err(format!("flush: {e}"));
            }
        }

        // 在 stdout 锁内读一行，读完即释放。
        let response = {
            let mut stdout_guard = self.stdout.lock().map_err(|e| format!("stdout lock: {e}"))?;
            let stdout = stdout_guard.as_mut().ok_or_else(|| "stdout not available".to_string())?;
            let mut reader = BufReader::new(stdout);
            let mut buf = String::new();
            if let Err(e) = reader.read_line(&mut buf) {
                return Err(format!("read: {e}"));
            }
            buf
        };

        // 释放锁之后再检查是否超时（这是近似做法 —— 一次卡住的读取
        // 可能跨过超时点。但实际上 Python 侧车 < 50ms 就响应，
        // 而且管道是行缓冲的，'\n' 一到 read_line 就返回）。
        if started.elapsed() > deadline {
            return Err(format!("timeout after {}ms", started.elapsed().as_millis()));
        }
        if response.trim().is_empty() {
            return Err("empty response".into());
        }

        // 尽力解析：仅检查响应是否含 `ok: true`。
        // 不做 id 关联，因为 v0.11b 阶段只有这一个写入者；
        // 后续版本若有多探测并发，需要按 id 走 oneshot 通道。
        let parsed: serde_json::Value = serde_json::from_str(response.trim())
            .map_err(|e| format!("parse: {e}"))?;
        if parsed.get("ok").and_then(|v| v.as_bool()) != Some(true) {
            return Err(format!("not ok: {response}"));
        }
        Ok(started.elapsed().as_millis() as u64)
    }

    /// v0.12c —— `ping_blocking` 的异步友好版本。
    ///
    /// 阻塞版本内部使用同步互斥锁，若直接在异步运行时调用会
    /// 阻塞工作线程。本封装使用 `tokio::task::spawn_blocking`
    /// 把 I/O 移到运行时之外，再用 `tokio::time::timeout` 强制
    /// 设定一个墙上时钟截止时间（阻塞版本的超时只是尽力而为，
    /// 因为读操作可能与截止时间赛跑）。
    ///
    /// 成功时返回毫秒级的延迟，否则返回描述失败原因的错误字符串。
    pub async fn ping_async(&self, timeout_ms: u64) -> Result<u64, String> {
        let this = Self {
            child: Arc::new(Mutex::new(None)),  // ping_async 用不到 child
            stdin: Arc::new(Mutex::new(None)),
            stdout: Arc::new(Mutex::new(None)),
            status: Arc::new(Mutex::new(self.status.lock().map_err(|e| format!("status lock: {e}"))?.clone())),
        };
        // 通过 swap 把真正的 stdin/stdout 移入派生任务。
        // 这是安全的，因为此时我们没有持有其他引用，且每次调用
        // 都运行在同一个单线程异步运行时上。
        *this.stdin.lock().map_err(|e| format!("stdin lock: {e}"))? =
            self.stdin.lock().map_err(|e| format!("stdin lock: {e}"))?.take();
        *this.stdout.lock().map_err(|e| format!("stdout lock: {e}"))? =
            self.stdout.lock().map_err(|e| format!("stdout lock: {e}"))?.take();

        match tokio::time::timeout(
            std::time::Duration::from_millis(timeout_ms),
            tokio::task::spawn_blocking(move || this.ping_blocking(timeout_ms)),
        )
        .await
        {
            Ok(Ok(Ok(latency))) => Ok(latency),
            Ok(Ok(Err(e))) => Err(e),
            Ok(Err(e)) => Err(format!("spawn_blocking: {e}")),
            Err(_) => Err(format!("async timeout after {timeout_ms}ms")),
        }
    }

    /// v0.13d —— `sidecar_predict` 的阻塞版本。
    ///
    /// 加锁约定与 `ping_blocking` 一致：仅在写入时持有 stdin，
    /// 仅在读取时持有 stdout。stdin/stdout 不可用时
    /// （侧车未运行）回退为空 Vec。
    pub fn predict_blocking(
        &self,
        markets: &[(String, f64)],
        timeout_ms: u64,
    ) -> Result<crate::domain::lab::sidecar::PredictResult, String> {
        use std::io::{BufRead, BufReader, Write};
        use std::time::{Duration, Instant};

        if !self.is_running() {
            return Err("sidecar not running".to_string());
        }

        let id = format!("pred_{}", chrono::Utc::now().timestamp_millis());
        let line = build_predict_request(&id, markets);
        let started = Instant::now();
        let deadline = Duration::from_millis(timeout_ms);

        {
            let mut stdin_guard = self.stdin.lock().map_err(|e| format!("stdin lock: {e}"))?;
            let stdin = stdin_guard.as_mut().ok_or_else(|| "stdin not available".to_string())?;
            if let Err(e) = writeln!(stdin, "{line}") {
                return Err(format!("write: {e}"));
            }
            if let Err(e) = stdin.flush() {
                return Err(format!("flush: {e}"));
            }
        }

        let response_line = {
            let mut stdout_guard = self.stdout.lock().map_err(|e| format!("stdout lock: {e}"))?;
            let stdout = stdout_guard.as_mut().ok_or_else(|| "stdout not available".to_string())?;
            let mut reader = BufReader::new(stdout);
            let mut buf = String::new();
            if let Err(e) = reader.read_line(&mut buf) {
                return Err(format!("read: {e}"));
            }
            buf
        };

        if started.elapsed() > deadline {
            return Err(format!("timeout after {}ms", started.elapsed().as_millis()));
        }
        if response_line.trim().is_empty() {
            return Err("empty response".into());
        }

        let parsed = parse_line(&response_line).map_err(|e| format!("parse: {e}"))?;
        let response = match parsed {
            crate::domain::lab::sidecar::ParseResult::Response(r) => r,
            crate::domain::lab::sidecar::ParseResult::Request(_) => {
                return Err("got a request when expecting a response".into());
            }
        };
        if response.id != id {
            return Err(format!(
                "sidecar id mismatch: sent={id}, got={}",
                response.id
            ));
        }
        parse_predict_response(&response).map_err(|e| format!("decode: {e}"))
    }

    /// v0.13d —— `predict_blocking` 的 async 包装。
///
/// 与 `ping_async` 采用同一模式：`spawn_blocking` 处理 I/O，
/// `tokio::time::timeout` 兜底墙钟截止时间。返回的 `PredictResult`
/// 包含 v0.12a / v0.13b 新增的 model_version 与 brier_score。
pub async fn predict_async(
        &self,
        markets: Vec<(String, f64)>,
        timeout_ms: u64,
    ) -> Result<crate::domain::lab::sidecar::PredictResult, String> {
        let this = Self {
            child: Arc::new(Mutex::new(None)),
            stdin: Arc::new(Mutex::new(None)),
            stdout: Arc::new(Mutex::new(None)),
            status: Arc::new(Mutex::new(self.status.lock().map_err(|e| format!("status lock: {e}"))?.clone())),
        };
        *this.stdin.lock().map_err(|e| format!("stdin lock: {e}"))? =
            self.stdin.lock().map_err(|e| format!("stdin lock: {e}"))?.take();
        *this.stdout.lock().map_err(|e| format!("stdout lock: {e}"))? =
            self.stdout.lock().map_err(|e| format!("stdout lock: {e}"))?.take();

        match tokio::time::timeout(
            std::time::Duration::from_millis(timeout_ms),
            tokio::task::spawn_blocking(move || this.predict_blocking(&markets, timeout_ms)),
        )
        .await
        {
            Ok(Ok(Ok(p))) => Ok(p),
            Ok(Ok(Err(e))) => Err(e),
            Ok(Err(e)) => Err(format!("spawn_blocking: {e}")),
            Err(_) => Err(format!("async timeout after {timeout_ms}ms")),
        }
    }
}

/// 启动侧车。若已在运行则什么都不做。
#[tauri::command]
pub async fn start_sidecar(
    state: State<'_, SidecarState>,
    args: StartSidecarArgs,
) -> AppResult<SidecarStatus> {
    if state.is_running() {
        return read_status(&state);
    }
    let cmd = args.command.clone()
        .or_else(|| std::env::var("POLYROCKET_SIDECAR_CMD").ok())
        .unwrap_or_else(|| "polyrocket-sidecar".to_string());
    let cmd_args = args.args.clone().unwrap_or_default();

    let mut command = Command::new(&cmd);
    command.args(&cmd_args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // v0.56 —— 把代理透传给侧车。设置标准环境变量
    // （HTTP_PROXY / HTTPS_PROXY / ALL_PROXY），
    // 这样侧车用到的任何 HTTP 库（httpx、requests）
    // 都会自动通过它走。仅在 Rust 端启用代理时
    // 才设置这些变量；环境变量路径是 opt-in 的，
    // 以保持默认（直连出站）行为不变。
    if let Ok(proxy) = std::env::var("POLYROCKET_PROXY") {
        let p = proxy.trim();
        if !p.is_empty() {
            command.env("HTTP_PROXY", p);
            command.env("HTTPS_PROXY", p);
            command.env("ALL_PROXY", p);
        }
    }
    let mut child = command.spawn().map_err(|e| {
        AppError::Internal(format!("sidecar spawn '{cmd}': {e}"))
    })?;
    let pid = child.id();
    let stdin = child.stdin.take().ok_or_else(|| {
        AppError::Internal("sidecar stdin unavailable".into())
    })?;
    let stdout = child.stdout.take().ok_or_else(|| {
        AppError::Internal("sidecar stdout unavailable".into())
    })?;
    *state.child.lock().unwrap() = Some(child);
    *state.stdin.lock().unwrap() = Some(stdin);
    *state.stdout.lock().unwrap() = Some(stdout);
    let status = SidecarStatus { running: true, pid: Some(pid), command: cmd, last_error: None };
    state.set_status(status.clone());
    Ok(status)
}

/// 停止侧车（相当于 SIGKILL）。
#[tauri::command]
pub async fn stop_sidecar(state: State<'_, SidecarState>) -> AppResult<SidecarStatus> {
    let mut guard = state.child.lock().unwrap();
    if let Some(mut child) = guard.take() {
        let _ = child.kill();
        let _ = child.wait();
    }
    *state.stdin.lock().unwrap() = None;
    *state.stdout.lock().unwrap() = None;
    let status = SidecarStatus { running: false, ..Default::default() };
    state.set_status(status.clone());
    Ok(status)
}

/// 返回当前侧车状态。
#[tauri::command]
pub async fn sidecar_status(state: State<'_, SidecarState>) -> AppResult<SidecarStatus> {
    read_status(&state)
}

fn read_status(state: &SidecarState) -> AppResult<SidecarStatus> {
    let s = state.status.lock().unwrap().clone();
    Ok(s)
}

/// 向侧车发送 `predict` 请求，返回解析后的预测结果。
/// 在 v0.6b 中这是尽力而为：若侧车未运行，返回空 Vec
/// （调用方回退到 domain::signal 的启发式逻辑）。
#[tauri::command]
pub async fn sidecar_predict(
    state: State<'_, SidecarState>,
    markets: Vec<(String, f64)>,
) -> AppResult<Vec<Prediction>> {
    // v0.122b —— 当 POLYROCKET_DISABLE_SIDECAR=1 时走新的 Rust 路径。
    // 未设置时（默认）仍然使用 Python 侧车。
    // v0.122g 会翻转默认行为。（这个开关是回滚迁移的安全阀。）
    if !is_sidecar_disabled() {
        return sidecar_predict_legacy(state, markets).await;
    }
    // 新 Rust 路径 —— 调用 inference::predict_from_markets
    // 并把结果转换为传输用的 `Vec<Prediction>` 形态。
    let (weights, model_version, brier_score) = load_active_or_fallback();
    let inputs: Vec<_> = markets
        .iter()
        .map(|(id, price)| crate::domain::lab::inference::MarketInput {
            market_id: id.as_str(),
            price: *price,
            market_age_hours: 0.0, // 传输格式不包含 age;默认 0
        })
        .collect();
    let r = crate::domain::lab::inference::predict_from_markets(
        &weights,
        &model_version,
        brier_score,
        &inputs,
    );
    Ok(r.predictions
        .into_iter()
        .map(|p| Prediction {
            market_id: p.market_id,
            prob: p.prob,
            confidence: p.confidence,
            rationale: Some(p.rationale),
        })
        .collect())
}

/// v0.122b —— 原 `sidecar_predict` 函数体，现在作为私有辅助函数。
/// 完整保留是为了在 `POLYROCKET_DISABLE_SIDECAR=1` 时
/// 仍然可走旧的 Python 路径。
async fn sidecar_predict_legacy(
    state: State<'_, SidecarState>,
    markets: Vec<(String, f64)>,
) -> AppResult<Vec<Prediction>> {
    check_sidecar_enabled()?;
    if !state.is_running() {
        return Ok(Vec::new());
    }
    // 构造请求并尝试读取响应。
    let id = format!("pred_{}", chrono::Utc::now().timestamp_millis());
    let line = build_predict_request(&id, &markets);

    // 通过 stdin 发送（同步 std::io::Write；短暂持有锁）。
    // 若侧车已挂，则回退为空。
    {
        let mut stdin_guard = state.stdin.lock().unwrap();
        let Some(stdin) = stdin_guard.as_mut() else {
            return Ok(Vec::new());
        };
        use std::io::Write;
        if let Err(e) = writeln!(stdin, "{line}") {
            return Err(AppError::Internal(format!("sidecar write: {e}")));
        }
        if let Err(e) = stdin.flush() {
            return Err(AppError::Internal(format!("sidecar flush: {e}")));
        }
    }
    // 从 stdout 读取单行。v0.6b 阶段这是一次性同步读取；
    // 真正的实现应该用异步 I/O 加关联表（请求 id → oneshot）。
    let mut response_line = String::new();
    {
        let mut stdout_guard = state.stdout.lock().unwrap();
        let Some(stdout) = stdout_guard.as_mut() else {
            return Ok(Vec::new());
        };
        use std::io::{BufRead, BufReader};
        let mut reader = BufReader::new(stdout);
        if let Err(e) = reader.read_line(&mut response_line) {
            return Err(AppError::Internal(format!("sidecar read: {e}")));
        }
    }
    if response_line.trim().is_empty() {
        return Ok(Vec::new());
    }
    let parsed = parse_line(&response_line).map_err(|e| {
        AppError::Internal(format!("sidecar parse: {e}"))
    })?;
    let response = match parsed {
        crate::domain::lab::sidecar::ParseResult::Response(r) => r,
        crate::domain::lab::sidecar::ParseResult::Request(_) => {
            return Err(AppError::Internal("got a request when expecting a response".into()));
        }
    };
    if response.id != id {
        // id 不匹配；v0.6c 将通过正确的关联机制修复。
        return Err(AppError::Internal(format!(
            "sidecar id mismatch: sent={id}, got={}",
            response.id
        )));
    }
    let preds = parse_predict_response(&response).map_err(|e| {
        AppError::Internal(format!("sidecar decode: {e}"))
    })?;
    Ok(preds.predictions)
}

/// v0.13d —— `sidecar_predict` 的异步友好版本，
/// 返回完整的 `PredictResult`（含 model_version + brier_score）。
/// 与 `ping_async` 同样使用 `spawn_blocking` + `tokio::time::timeout`。
/// 当侧车未运行时回退到一个空的 `PredictResult`（与 v0.6b 语义一致）。
#[tauri::command]
pub async fn sidecar_predict_async(
    state: State<'_, SidecarState>,
    markets: Vec<(String, f64)>,
    timeout_ms: Option<u64>,
) -> AppResult<crate::domain::lab::sidecar::PredictResult> {
    check_sidecar_enabled()?;
    use crate::domain::lab::sidecar::PredictResult as PR;
    if !state.is_running() {
        return Ok(PR {
            predictions: Vec::new(),
            model_version: None,
            brier_score: None,
        });
    }
    let timeout = timeout_ms.unwrap_or(5_000);
    match state.predict_async(markets, timeout).await {
        Ok(p) => Ok(p),
        Err(e) => Err(AppError::Internal(format!("sidecar predict_async: {e}"))),
    }
}

// =================================================================
// ============== v0.17a —— train_job IPC 与进度事件 ==============
// =================================================================

/// `train_job` IPC 的入参。v0.17a —— 镜像 Python 侧车的可选参数。
/// 所有字段均可选；Python 侧车使用合理默认值（n_trials=4, epochs=80）。
#[derive(Debug, Clone, Deserialize)]
pub struct TrainJobArgs {
    /// 默认 4（v0.17a 中最大 4；`train.py` 中的网格为
    /// 4 组硬编码的 (lr, reg) 组合）。
    pub n_trials: Option<u32>,
    /// 默认 80。每轮训练的 epoch 数。
    pub epochs: Option<u32>,
    /// IPC 可选超时（毫秒）。默认 60 秒 —— Python 扫描为
    /// 4 × 80 epoch，通常 2-10 秒，慢机器可能飙升到 30 秒。
    pub timeout_ms: Option<u64>,
}

/// v0.17a —— 在 Python 侧车上启动一个训练任务。
///
/// 在 Tauri 总线上发射两个事件：
///   - `train_job:started`  —— IPC 被分发时
///   - `train_job:finished` —— 扫描完成（或失败）时
///
/// 返回完整的 `TrainResult`（job_id、status、best_brier、
/// best_params、trials、duration_ms、candidate_path、message）。
///
/// 当侧车未运行时回退为一个无 trial 的「failed」结果
/// —— 与 v0.6b 的 predict fallback 一致
/// （L1 不必为「侧车宕掉」单独写分支）。
#[tauri::command]
pub async fn train_job(
    state: State<'_, SidecarState>,
    app_state: State<'_, crate::infra::state::AppState>,
    app: AppHandle,
    args: TrainJobArgs,
) -> AppResult<TrainResult> {
    check_sidecar_enabled()?;
    let job_id = format!("train-{}", uuid::Uuid::new_v4().to_string().split('-').next().unwrap_or("00000000"));
    let n_trials = args.n_trials.unwrap_or(4).clamp(1, 4);
    let epochs = args.epochs.unwrap_or(80).clamp(1, 1000);
    let timeout = args.timeout_ms.unwrap_or(60_000);
    let started_at = chrono::Utc::now().timestamp_millis();

    // v0.17a —— 在调用侧车之前就先 emit started 事件，
    // 这样 L1 可以立即渲染出「Training…」提示。
    let _ = app.emit(
        "train_job:started",
        TrainStartedEvent {
            job_id: job_id.clone(),
            n_trials,
            epochs,
            started_at,
        },
    );
    // v0.42b —— IPC 接收瞬间的生命周期事件。
    // 这代表「用户点击了 Train」，并不保证侧车最终会成功。
    use crate::infra::telemetry;
    telemetry::emit(telemetry::Event::TrainStarted {
        job_id: job_id.clone(),
        n_trials: n_trials as usize,
        epochs: epochs as usize,
    });

    if !state.is_running() {
        // 侧车未运行 —— 发射一个 status="failed" 的 finished
        // 事件并附带描述信息，然后返回同样形态的结果，
        // L1 不必为「侧车不可用」单独写分支。
        telemetry::emit(telemetry::Event::TrainFailed {
            job_id: job_id.clone(),
            error: "sidecar not running".into(),
        });
        let _ = app.emit(
            "train_job:finished",
            TrainFinishedEvent {
                job_id: job_id.clone(),
                status: "failed".into(),
                best_brier: None,
                best_params: None,
                trials: vec![],
                duration_ms: 0,
                candidate_path: None,
                message: Some("sidecar not running".into()),
                finished_at: chrono::Utc::now().timestamp_millis(),
            },
        );
        return Ok(TrainResult {
            job_id,
            status: "failed".into(),
            best_brier: None,
            best_params: None,
            trials: vec![],
            duration_ms: 0,
            candidate_path: None,
            message: Some("sidecar not running".into()),
        });
    }

    // 构造请求行，在 stdin 锁内写入，在 stdout 锁内读取一行。
    // 加锁约定与 `sidecar_predict`（v0.6b）一致。
    // 4-trial 扫描耗时 2-30 秒，远小于 60 秒超时。
    let line = build_train_request(&job_id, Some(n_trials), Some(epochs));
    let started = std::time::Instant::now();
    let deadline = std::time::Duration::from_millis(timeout);

    let response_line = {
        // 在 stdin 锁内写入，读之前释放锁。
        {
            let mut stdin_guard = state.stdin.lock().map_err(|e| format!("stdin lock: {e}"))
                .map_err(AppError::Internal)?;
            let stdin = stdin_guard.as_mut()
                .ok_or_else(|| AppError::Internal("stdin not available".into()))?;
            use std::io::Write;
            if let Err(e) = writeln!(stdin, "{line}") {
                return Err(AppError::Internal(format!("train_job write: {e}")));
            }
            if let Err(e) = stdin.flush() {
                return Err(AppError::Internal(format!("train_job flush: {e}")));
            }
        }
        // 在 stdout 锁内读取一行。
        let mut stdout_guard = state.stdout.lock().map_err(|e| format!("stdout lock: {e}"))
            .map_err(AppError::Internal)?;
        let stdout = stdout_guard.as_mut()
            .ok_or_else(|| AppError::Internal("stdout not available".into()))?;
        use std::io::{BufRead, BufReader};
        let mut reader = BufReader::new(stdout);
        let mut buf = String::new();
        if let Err(e) = reader.read_line(&mut buf) {
            return Err(AppError::Internal(format!("train_job read: {e}")));
        }
        buf
    };

    let elapsed = started.elapsed();
    if elapsed > deadline {
        let msg = format!("train_job timeout after {}ms", elapsed.as_millis());
        // v0.42b —— 发射生命周期事件。TrainFailed 把超时
        // 与侧车解码得到的失败区别开来。
        use crate::infra::telemetry;
        telemetry::emit(telemetry::Event::TrainFailed {
            job_id: job_id.clone(),
            error: msg.clone(),
        });
        let _ = app.emit(
            "train_job:finished",
            TrainFinishedEvent {
                job_id: job_id.clone(),
                status: "failed".into(),
                best_brier: None,
                best_params: None,
                trials: vec![],
                duration_ms: elapsed.as_millis() as i64,
                candidate_path: None,
                message: Some(msg.clone()),
                finished_at: chrono::Utc::now().timestamp_millis(),
            },
        );
        return Err(AppError::Internal(msg));
    }

    let parsed = parse_line(&response_line).map_err(|e| {
        AppError::Internal(format!("train_job parse: {e}"))
    })?;
    let response = match parsed {
        crate::domain::lab::sidecar::ParseResult::Response(r) => r,
        _ => {
            return Err(AppError::Internal("train_job: not a response".into()));
        }
    };
    if response.id != job_id {
        return Err(AppError::Internal(format!(
            "train_job id mismatch: sent={job_id}, got={}",
            response.id
        )));
    }
    let result = parse_train_response(&response).map_err(|e| {
        // v0.42b —— 发射生命周期事件。遥测把解码错误
        // 也归类为训练失败。
        use crate::infra::telemetry;
        telemetry::emit(telemetry::Event::TrainFailed {
            job_id: job_id.clone(),
            error: e.to_string(),
        });
        AppError::Internal(format!("train_job decode: {e}"))
    })?;

    // v0.17a —— 用解析结果发射 finished 事件。
    // v0.42b —— 发射 TrainCompleted 生命周期事件。
    // model_version 从 result.model_version 读取，
    // best_brier 直接来自解析后的 payload。
    use crate::infra::telemetry as _t;
    let train_completed = matches!(result.status.as_str(), "succeeded" | "ok");
    if train_completed {
        let model_version = result
            .best_params
            .as_ref()
            .and_then(|p| p.get("model_version").and_then(|v| v.as_str()))
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("logistic-{}", result.job_id));
        _t::emit(_t::Event::TrainCompleted {
            job_id: result.job_id.clone(),
            model_version,
            best_brier: result.best_brier,
            duration_ms: result.duration_ms as u64,
        });
    } else {
        _t::emit(_t::Event::TrainFailed {
            job_id: result.job_id.clone(),
            error: result.message.clone().unwrap_or_else(|| "train failed".into()),
        });
    }
    let _ = app.emit(
        "train_job:finished",
        TrainFinishedEvent {
            job_id: result.job_id.clone(),
            status: result.status.clone(),
            best_brier: result.best_brier,
            best_params: result.best_params.clone(),
            trials: result.trials.iter().map(|t| TrainTrialDto {
                lr: t.lr,
                reg: t.reg,
                brier: t.brier,
                weights: t.weights.clone(),
            }).collect(),
            duration_ms: result.duration_ms,
            candidate_path: result.candidate_path.clone(),
            message: result.message.clone(),
            finished_at: chrono::Utc::now().timestamp_millis(),
        },
    );

    // v0.28a —— 若 AppState 中启用了 auto-promote，且
    // 本次训练成功，则派生一个后台 worker 调用
    // `auto_promote_if_better`，并把结果 emit 到
    // `auto_promote:finished`。train IPC 立即返回，
    // worker 在后台运行。
    //
    // worker 通过克隆读取 `state`（SidecarState）和 `app`（AppHandle）
    // —— 两者都很轻量（SidecarState 内部仅是 Arc<Mutex<...>>；
    // AppHandle 是某个长期句柄的克隆）。
    if result.status == "succeeded" || result.status == "ok" {
        let auto_promote_enabled = {
            let guard = app_state
                .auto_promote
                .lock()
                .map_err(|e| AppError::Internal(format!("auto_promote lock: {e}")))?;
            guard.enabled
        };
        if auto_promote_enabled {
            let brier_margin = {
                let guard = app_state
                    .auto_promote
                    .lock()
                    .map_err(|e| AppError::Internal(format!("auto_promote lock: {e}")))?;
                guard.brier_margin
            };
            let sidecar = state.inner().clone();
            let app_clone = app.clone();
            let train_job_id = result.job_id.clone();
            tokio::spawn(async move {
                run_auto_promote_worker(
                    sidecar,
                    app_clone,
                    brier_margin,
                    train_job_id,
                )
                .await;
            });
        }
    }

    Ok(result)
}

/// v0.18a —— 把当前候选提升到 active 槽位。
///
/// 这是一个快速、同步的操作（约 10 毫秒的文件移动）。
/// 不发射进度事件。IPC 返回完整的 `PromoteResult`。
///
/// `args.job_id` 可选。若设置，Python 侧车会拒绝提升来自
/// 其他 job 的候选（用于竞态保护 —— 防止「用户打算提升
/// → 第二次训练完成 → 实际提升」之间发生目标漂移）。
///
/// 当侧车未运行时回退为 `promoted: false` 并附带描述信息
/// （与 v0.17a 的 train fallback 模式一致）。
#[tauri::command]
pub async fn promote_model(
    state: State<'_, SidecarState>,
    args: PromoteModelArgs,
) -> AppResult<PromoteResult> {
    check_sidecar_enabled()?;
    let job_id = format!("promote-{}", uuid::Uuid::new_v4().to_string().split('-').next().unwrap_or("00000000"));
    let line = build_promote_request(&job_id, args.job_id.as_deref(), args.trial_index);

    if !state.is_running() {
        return Ok(PromoteResult {
            promoted: false,
            status: "failed".into(),
            previous_path: None,
            active_path: None,
            promoted_at_ms: None,
            model_version: String::new(),
            message: Some("sidecar not running".into()),
            trial_index: None,
        });
    }

    // 与 train_job 相同的加锁约定：stdin 锁内写入，
    // stdout 锁内读取。
    let response_line = {
        {
            let mut stdin_guard = state.stdin.lock()
                .map_err(|e| format!("stdin lock: {e}"))
                .map_err(AppError::Internal)?;
            let stdin = stdin_guard.as_mut()
                .ok_or_else(|| AppError::Internal("stdin not available".into()))?;
            use std::io::Write;
            if let Err(e) = writeln!(stdin, "{line}") {
                return Err(AppError::Internal(format!("promote write: {e}")));
            }
            if let Err(e) = stdin.flush() {
                return Err(AppError::Internal(format!("promote flush: {e}")));
            }
        }
        let mut stdout_guard = state.stdout.lock()
            .map_err(|e| format!("stdout lock: {e}"))
            .map_err(AppError::Internal)?;
        let stdout = stdout_guard.as_mut()
            .ok_or_else(|| AppError::Internal("stdout not available".into()))?;
        use std::io::{BufRead, BufReader};
        let mut reader = BufReader::new(stdout);
        let mut buf = String::new();
        if let Err(e) = reader.read_line(&mut buf) {
            return Err(AppError::Internal(format!("promote read: {e}")));
        }
        buf
    };

    let parsed = parse_line(&response_line).map_err(|e| {
        AppError::Internal(format!("promote parse: {e}"))
    })?;
    let response = match parsed {
        crate::domain::lab::sidecar::ParseResult::Response(r) => r,
        _ => return Err(AppError::Internal("promote: not a response".into())),
    };
    if response.id != job_id {
        return Err(AppError::Internal(format!(
            "promote id mismatch: sent={job_id}, got={}",
            response.id
        )));
    }
    parse_promote_response(&response).map_err(|e| {
        AppError::Internal(format!("promote decode: {e}"))
    })
    .map(|r| {
        // v0.42b —— 提升成功时发射生命周期事件。
        // `reason` 和 `trial_index` 直接从响应读取。
        // 跳过 / 失败的提升不发射（系统通知路径已能
        // 覆盖用户可见的提示）。
        if r.promoted {
            use crate::infra::telemetry;
            telemetry::emit(telemetry::Event::PromoteCompleted {
                job_id: args.job_id.clone().unwrap_or_else(|| "<latest>".into()),
                model_version: r.model_version.clone(),
                trial_index: r.trial_index,
                reason: r.message.clone().unwrap_or_else(|| "Promoted".into()),
            });
        }
        r
    })
}

/// `promote_model` IPC 的入参。v0.18a —— 镜像 Python 侧车的
/// 可选 `job_id` 参数。v0.21a —— 新增 `trial_index` 以支持批量提升。
#[derive(Debug, Clone, Deserialize)]
pub struct PromoteModelArgs {
    /// 若设置，则拒绝提升来自其他 job 的候选。
    /// 默认 `None`（接受任意当前候选）。
    pub job_id: Option<String>,
    /// v0.21a —— 若设置，则提升 `all_trials[]` 中的第 n 个 trial
    /// 而不是 best。0 索引。`None`（默认）意为「提升最佳」。
    pub trial_index: Option<usize>,
}

/// 列出提升历史。v0.19b —— 只读审计。
/// 无入参；返回 active.json 中 `promotion_history` 数组的最近 20 条。
#[tauri::command]
pub async fn list_promote_history(
    state: State<'_, SidecarState>,
) -> AppResult<PromoteHistoryResult> {
    check_sidecar_enabled()?;
    let job_id = format!(
        "list-history-{}",
        uuid::Uuid::new_v4()
            .to_string()
            .split('-')
            .next()
            .unwrap_or("00000000")
    );
    let line = crate::domain::lab::sidecar::build_list_promote_history_request(&job_id);

    if !state.is_running() {
        return Ok(PromoteHistoryResult {
            ok: false,
            entries: Vec::new(),
            count: 0,
            message: Some("sidecar not running".into()),
        });
    }

    // 与 train_job / promote_model 相同的加锁约定。
    let response_line = {
        {
            let mut stdin_guard = state.stdin.lock()
                .map_err(|e| format!("stdin lock: {e}"))
                .map_err(AppError::Internal)?;
            let stdin = stdin_guard.as_mut()
                .ok_or_else(|| AppError::Internal("stdin not available".into()))?;
            use std::io::Write;
            if let Err(e) = writeln!(stdin, "{line}") {
                return Err(AppError::Internal(format!("list_promote_history write: {e}")));
            }
            if let Err(e) = stdin.flush() {
                return Err(AppError::Internal(format!("list_promote_history flush: {e}")));
            }
        }
        let mut stdout_guard = state.stdout.lock()
            .map_err(|e| format!("stdout lock: {e}"))
            .map_err(AppError::Internal)?;
        let stdout = stdout_guard.as_mut()
            .ok_or_else(|| AppError::Internal("stdout not available".into()))?;
        use std::io::{BufRead, BufReader};
        let mut reader = BufReader::new(stdout);
        let mut buf = String::new();
        if let Err(e) = reader.read_line(&mut buf) {
            return Err(AppError::Internal(format!("list_promote_history read: {e}")));
        }
        buf
    };

    let parsed = parse_line(&response_line).map_err(|e| {
        AppError::Internal(format!("list_promote_history parse: {e}"))
    })?;
    let response = match parsed {
        crate::domain::lab::sidecar::ParseResult::Response(r) => r,
        _ => return Err(AppError::Internal("list_promote_history: not a response".into())),
    };
    if response.id != job_id {
        return Err(AppError::Internal(format!(
            "list_promote_history id mismatch: sent={job_id}, got={}",
            response.id
        )));
    }
    parse_list_promote_history_response(&response).map_err(|e| {
        AppError::Internal(format!("list_promote_history decode: {e}"))
    })
}

/// 将 active 模型回滚到之前的版本。v0.20b。
#[tauri::command]
pub async fn rollback_model(
    state: State<'_, SidecarState>,
    args: RollbackModelArgs,
) -> AppResult<RollbackResult> {
    check_sidecar_enabled()?;
    let job_id = format!(
        "rollback-{}",
        uuid::Uuid::new_v4()
            .to_string()
            .split('-')
            .next()
            .unwrap_or("00000000")
    );
    let line = build_rollback_request(&job_id, &args.model_version);

    if !state.is_running() {
        return Ok(RollbackResult {
            rolled_back: false,
            status: "failed".into(),
            previous_path: None,
            active_path: None,
            rolled_back_at_ms: None,
            model_version: String::new(),
            message: Some("sidecar not running".into()),
        });
    }

    let response_line = {
        {
            let mut stdin_guard = state.stdin.lock()
                .map_err(|e| format!("stdin lock: {e}"))
                .map_err(AppError::Internal)?;
            let stdin = stdin_guard.as_mut()
                .ok_or_else(|| AppError::Internal("stdin not available".into()))?;
            use std::io::Write;
            if let Err(e) = writeln!(stdin, "{line}") {
                return Err(AppError::Internal(format!("rollback write: {e}")));
            }
            if let Err(e) = stdin.flush() {
                return Err(AppError::Internal(format!("rollback flush: {e}")));
            }
        }
        let mut stdout_guard = state.stdout.lock()
            .map_err(|e| format!("stdout lock: {e}"))
            .map_err(AppError::Internal)?;
        let stdout = stdout_guard.as_mut()
            .ok_or_else(|| AppError::Internal("stdout not available".into()))?;
        use std::io::{BufRead, BufReader};
        let mut reader = BufReader::new(stdout);
        let mut buf = String::new();
        if let Err(e) = reader.read_line(&mut buf) {
            return Err(AppError::Internal(format!("rollback read: {e}")));
        }
        buf
    };

    let parsed = parse_line(&response_line).map_err(|e| {
        AppError::Internal(format!("rollback parse: {e}"))
    })?;
    let response = match parsed {
        crate::domain::lab::sidecar::ParseResult::Response(r) => r,
        _ => return Err(AppError::Internal("rollback: not a response".into())),
    };
    if response.id != job_id {
        return Err(AppError::Internal(format!(
            "rollback id mismatch: sent={job_id}, got={}",
            response.id
        )));
    }
    parse_rollback_response(&response).map_err(|e| {
        AppError::Internal(format!("rollback decode: {e}"))
    })
}

/// `rollback_model` IPC 的入参。v0.20b —— `model_version` 是
/// 要回滚到的目标版本（在 `promotion_history` 数组中查找）。
#[derive(Debug, Clone, Deserialize)]
pub struct RollbackModelArgs {
    pub model_version: String,
}

/// 仅当候选明显优于 active 模型时才自动提升。v0.23b。
#[tauri::command]
pub async fn auto_promote_if_better(
    state: State<'_, SidecarState>,
    args: AutoPromoteIfBetterArgs,
) -> AppResult<AutoPromoteIfBetterResult> {
    check_sidecar_enabled()?;
    let job_id = format!(
        "auto-promote-{}",
        uuid::Uuid::new_v4()
            .to_string()
            .split('-')
            .next()
            .unwrap_or("00000000")
    );
    let line = build_auto_promote_if_better_request(
        &job_id,
        args.brier_margin,
        args.trial_index,
    );

    if !state.is_running() {
        return Ok(AutoPromoteIfBetterResult {
            promoted: false,
            skipped: true,
            reason: "sidecar not running".into(),
            candidate_brier: None,
            active_brier: None,
            margin: args.brier_margin.unwrap_or(0.005),
            model_version: None,
            promoted_at_ms: None,
            message: Some("sidecar not running".into()),
        });
    }

    let response_line = {
        {
            let mut stdin_guard = state.stdin.lock()
                .map_err(|e| format!("stdin lock: {e}"))
                .map_err(AppError::Internal)?;
            let stdin = stdin_guard.as_mut()
                .ok_or_else(|| AppError::Internal("stdin not available".into()))?;
            use std::io::Write;
            if let Err(e) = writeln!(stdin, "{line}") {
                return Err(AppError::Internal(format!("auto_promote write: {e}")));
            }
            if let Err(e) = stdin.flush() {
                return Err(AppError::Internal(format!("auto_promote flush: {e}")));
            }
        }
        let mut stdout_guard = state.stdout.lock()
            .map_err(|e| format!("stdout lock: {e}"))
            .map_err(AppError::Internal)?;
        let stdout = stdout_guard.as_mut()
            .ok_or_else(|| AppError::Internal("stdout not available".into()))?;
        use std::io::{BufRead, BufReader};
        let mut reader = BufReader::new(stdout);
        let mut buf = String::new();
        if let Err(e) = reader.read_line(&mut buf) {
            return Err(AppError::Internal(format!("auto_promote read: {e}")));
        }
        buf
    };

    let parsed = parse_line(&response_line).map_err(|e| {
        AppError::Internal(format!("auto_promote parse: {e}"))
    })?;
    let response = match parsed {
        crate::domain::lab::sidecar::ParseResult::Response(r) => r,
        _ => return Err(AppError::Internal("auto_promote: not a response".into())),
    };
    if response.id != job_id {
        return Err(AppError::Internal(format!(
            "auto_promote id mismatch: sent={job_id}, got={}",
            response.id
        )));
    }
    parse_auto_promote_if_better_response(&response).map_err(|e| {
        AppError::Internal(format!("auto_promote decode: {e}"))
    })
}

/// `auto_promote_if_better` IPC 的入参。v0.23b —— `brier_margin`
/// 是候选必须比当前 active 优秀多少（Brier 越低越好）才会触发
/// 自动提升。默认 0.005。`trial_index` 选择使用哪一个 trial
/// （None = 最佳）。
#[derive(Debug, Clone, Deserialize)]
pub struct AutoPromoteIfBetterArgs {
    pub brier_margin: Option<f64>,
    pub trial_index: Option<usize>,
}

/// 批量提升当前候选中的全部 trial。v0.25b。
/// 无入参 —— 侧车读取候选并按顺序提升 `all_trials[]` 中的
/// 每个 trial。返回每个 trial 的结果列表。
#[tauri::command]
pub async fn promote_all_trials(
    state: State<'_, SidecarState>,
) -> AppResult<PromoteAllTrialsResult> {
    check_sidecar_enabled()?;
    let job_id = format!(
        "promote-all-{}",
        uuid::Uuid::new_v4()
            .to_string()
            .split('-')
            .next()
            .unwrap_or("00000000")
    );
    let line = build_promote_all_trials_request(&job_id);

    if !state.is_running() {
        return Ok(PromoteAllTrialsResult {
            ok: false,
            results: Vec::new(),
            count: 0,
            message: Some("sidecar not running".into()),
        });
    }

    let response_line = {
        {
            let mut stdin_guard = state.stdin.lock()
                .map_err(|e| format!("stdin lock: {e}"))
                .map_err(AppError::Internal)?;
            let stdin = stdin_guard.as_mut()
                .ok_or_else(|| AppError::Internal("stdin not available".into()))?;
            use std::io::Write;
            if let Err(e) = writeln!(stdin, "{line}") {
                return Err(AppError::Internal(format!("promote_all write: {e}")));
            }
            if let Err(e) = stdin.flush() {
                return Err(AppError::Internal(format!("promote_all flush: {e}")));
            }
        }
        let mut stdout_guard = state.stdout.lock()
            .map_err(|e| format!("stdout lock: {e}"))
            .map_err(AppError::Internal)?;
        let stdout = stdout_guard.as_mut()
            .ok_or_else(|| AppError::Internal("stdout not available".into()))?;
        use std::io::{BufRead, BufReader};
        let mut reader = BufReader::new(stdout);
        let mut buf = String::new();
        if let Err(e) = reader.read_line(&mut buf) {
            return Err(AppError::Internal(format!("promote_all read: {e}")));
        }
        buf
    };

    let parsed = parse_line(&response_line).map_err(|e| {
        AppError::Internal(format!("promote_all parse: {e}"))
    })?;
    let response = match parsed {
        crate::domain::lab::sidecar::ParseResult::Response(r) => r,
        _ => return Err(AppError::Internal("promote_all: not a response".into())),
    };
    if response.id != job_id {
        return Err(AppError::Internal(format!(
            "promote_all id mismatch: sent={job_id}, got={}",
            response.id
        )));
    }
    parse_promote_all_trials_response(&response).map_err(|e| {
        AppError::Internal(format!("promote_all decode: {e}"))
    })
}

// =================================================================
// ============== v0.43b —— backtest_model IPC ===================
// =================================================================

/// v0.43b —— `backtest_model` IPC 的入参。L1 从 markets
/// 表中拉取已结算的市场，把每条转换为 `BacktestSample` 后传入。
/// 返回 `BacktestResult`，包含 Brier + 校准（calibration）
/// + 表现最好 / 最差的若干样本。
///
/// 侧车逻辑很纯粹（除读模型文件外无 IO），因此单次调用
/// 的复杂度为 `O(samples)` —— 数百样本很快，数百万则很慢。
/// L1 应预先按合理时间窗口过滤。
#[derive(Debug, Clone, Deserialize)]
pub struct BacktestModelArgs {
    /// 要回测的模型，例如
    /// "logistic-train-441c352b"。先在 `archive.jsonl`
    /// 中查找，再到 `active.json`。
    pub model_version: String,
    /// 用于回放模型的 (price, market_age_hours, outcome)
    /// 样本列表。每个样本还可以包含一个 `label`，
    /// 用于 best/worst 列表展示。
    pub samples: Vec<BacktestSample>,
}

/// v0.43b —— 把保存的模型在 (price, market_age_hours, outcome)
/// 样本列表上回放，返回 Brier + 校准 + 每个样本的预测。
/// 补齐了 v0.17–v0.41 模型生命周期的缺口：没有这个 IPC，
/// 就无法回答「这个模型在真实结算上的表现如何」。
///
/// IPC 本身只做协议传输 —— stdin/stdout 加锁、解析、返回。
/// 预测 + Brier 计算都在 Python 侧车中（v0.43a）。
#[tauri::command]
pub async fn backtest_model(
    state: State<'_, SidecarState>,
    args: BacktestModelArgs,
) -> AppResult<BacktestResult> {
    check_sidecar_enabled()?;
    let job_id = format!(
        "backtest-{}",
        uuid::Uuid::new_v4()
            .to_string()
            .split('-')
            .next()
            .unwrap_or("00000000")
    );
    let line = build_backtest_model_request(&job_id, &args.model_version, &args.samples);

    if !state.is_running() {
        return Ok(BacktestResult {
            ok: false,
            model_version: args.model_version,
            sample_count: 0,
            brier_mean: None,
            brier_breakdown: Vec::new(),
            calibration: Vec::new(),
            top_winners: Vec::new(),
            top_losers: Vec::new(),
            message: Some("sidecar not running".into()),
        });
    }

    let response_line = {
        {
            let mut stdin_guard = state.stdin.lock()
                .map_err(|e| format!("stdin lock: {e}"))
                .map_err(AppError::Internal)?;
            let stdin = stdin_guard.as_mut()
                .ok_or_else(|| AppError::Internal("stdin not available".into()))?;
            use std::io::Write;
            if let Err(e) = writeln!(stdin, "{line}") {
                return Err(AppError::Internal(format!("backtest write: {e}")));
            }
            if let Err(e) = stdin.flush() {
                return Err(AppError::Internal(format!("backtest flush: {e}")));
            }
        }
        let mut stdout_guard = state.stdout.lock()
            .map_err(|e| format!("stdin lock: {e}"))
            .map_err(AppError::Internal)?;
        let stdout = stdout_guard.as_mut()
            .ok_or_else(|| AppError::Internal("stdout not available".into()))?;
        use std::io::{BufRead, BufReader};
        let mut reader = BufReader::new(stdout);
        let mut buf = String::new();
        if let Err(e) = reader.read_line(&mut buf) {
            return Err(AppError::Internal(format!("backtest read: {e}")));
        }
        buf
    };

    let parsed = parse_line(&response_line).map_err(|e| {
        AppError::Internal(format!("backtest parse: {e}"))
    })?;
    let response = match parsed {
        crate::domain::lab::sidecar::ParseResult::Response(r) => r,
        _ => return Err(AppError::Internal("backtest: not a response".into())),
    };
    if response.id != job_id {
        return Err(AppError::Internal(format!(
            "backtest id mismatch: sent={job_id}, got={}",
            response.id
        )));
    }
    parse_backtest_model_response(&response).map_err(|e| {
        AppError::Internal(format!("backtest decode: {e}"))
    })
}

// =================================================================
// v0.55 —— explain_model IPC
// =================================================================

/// v0.55 —— `explain_model` IPC 的入参。
/// L1 发送 model_version + 可选 sample；
/// 侧车返回每个特征对预测的贡献。L1
/// 把它渲染为水平条形图。
#[derive(Debug, Clone, Deserialize)]
pub struct ExplainModelArgs {
    /// 要解释的模型，例如
    /// "logistic-train-441c352b"。先在 `archive.jsonl`
    /// 中查找，再到 `active.json`。
    pub model_version: String,
    /// 可选 sample：{ price, market_age_hours }。
    /// 缺省时侧车使用默认 sample（price=0.5, age=24h），
    /// 让用户看到「模型对典型市场会输出什么」。
    #[serde(default)]
    pub sample: Option<ExplainSample>,
}

/// v0.55 —— 单个样本中每个特征的贡献。
/// 对 3 特征 logistic 模型而言这是精确分解
/// （非 SHAP 近似）：
/// `contribution_i = w_i * x_i * p(1-p)`。
/// L1 把 `features` 数组渲染为水平条形图
/// （正值绿色，负值红色，长度为 `abs_contribution`）。
#[tauri::command]
pub async fn explain_model(
    state: State<'_, SidecarState>,
    args: ExplainModelArgs,
) -> AppResult<ExplainResult> {
    check_sidecar_enabled()?;
    let job_id = format!(
        "explain-{}",
        uuid::Uuid::new_v4()
            .to_string()
            .split('-')
            .next()
            .unwrap_or("00000000")
    );
    let line = build_explain_model_request(
        &job_id,
        &args.model_version,
        args.sample.as_ref(),
    );

    if !state.is_running() {
        return Ok(ExplainResult {
            ok: false,
            model_version: args.model_version,
            features: Vec::new(),
            prediction: None,
            sample: None,
            message: "sidecar not running".into(),
        });
    }

    let response_line = {
        {
            let mut stdin_guard = state.stdin.lock()
                .map_err(|e| format!("stdin lock: {e}"))
                .map_err(AppError::Internal)?;
            let stdin = stdin_guard.as_mut()
                .ok_or_else(|| AppError::Internal("stdin not available".into()))?;
            use std::io::Write;
            if let Err(e) = writeln!(stdin, "{line}") {
                return Err(AppError::Internal(format!("explain write: {e}")));
            }
            if let Err(e) = stdin.flush() {
                return Err(AppError::Internal(format!("explain flush: {e}")));
            }
        }
        let mut stdout_guard = state.stdout.lock()
            .map_err(|e| format!("stdin lock: {e}"))
            .map_err(AppError::Internal)?;
        let stdout = stdout_guard.as_mut()
            .ok_or_else(|| AppError::Internal("stdout not available".into()))?;
        use std::io::{BufRead, BufReader};
        let mut reader = BufReader::new(stdout);
        let mut buf = String::new();
        if let Err(e) = reader.read_line(&mut buf) {
            return Err(AppError::Internal(format!("explain read: {e}")));
        }
        buf
    };

    let parsed = parse_line(&response_line).map_err(|e| {
        AppError::Internal(format!("explain parse: {e}"))
    })?;
    let response = match parsed {
        crate::domain::lab::sidecar::ParseResult::Response(r) => r,
        _ => return Err(AppError::Internal("explain: not a response".into())),
    };
    if response.id != job_id {
        return Err(AppError::Internal(format!(
            "explain id mismatch: sent={job_id}, got={}",
            response.id
        )));
    }
    parse_explain_model_response(&response).map_err(|e| {
        AppError::Internal(format!("explain decode: {e}"))
    })
}

// =================================================================
// v0.59 —— shap_explain IPC（通过 KernelExplainer 计算真实 SHAP）
// =================================================================

/// v0.59 —— `shap_explain` IPC 的入参。结构与
/// `ExplainModelArgs`（v0.55）相同。侧车返回满足
/// efficiency axiom 的逐特征 SHAP 值。
#[derive(Debug, Clone, Deserialize)]
pub struct ShapExplainArgs {
    /// 要解释的模型。
    pub model_version: String,
    /// 可选 sample：{ price, market_age_hours }。
    #[serde(default)]
    pub sample: Option<ExplainSample>,
}

/// v0.59 —— 为单个样本计算真实 SHAP 值。
/// 结果满足 SHAP efficiency axiom：
/// `Σφ_i = f(x) - E[f(x)]`（即相对基线预测的偏离）。
#[tauri::command]
pub async fn shap_explain(
    state: State<'_, SidecarState>,
    args: ShapExplainArgs,
) -> AppResult<ShapResult> {
    check_sidecar_enabled()?;
    let job_id = format!(
        "shap-{}",
        uuid::Uuid::new_v4()
            .to_string()
            .split('-')
            .next()
            .unwrap_or("00000000")
    );
    let line = build_shap_explain_request(
        &job_id,
        &args.model_version,
        args.sample.as_ref(),
    );

    if !state.is_running() {
        return Ok(ShapResult {
            ok: false,
            model_version: args.model_version,
            method: "kernel_shap".to_string(),
            features: Vec::new(),
            baseline_prediction: None,
            target_prediction: None,
            efficiency_diff: None,
            sample: None,
            message: "sidecar not running".into(),
        });
    }

    let response_line = {
        {
            let mut stdin_guard = state.stdin.lock()
                .map_err(|e| format!("stdin lock: {e}"))
                .map_err(AppError::Internal)?;
            let stdin = stdin_guard.as_mut()
                .ok_or_else(|| AppError::Internal("stdin not available".into()))?;
            use std::io::Write;
            if let Err(e) = writeln!(stdin, "{line}") {
                return Err(AppError::Internal(format!("shap write: {e}")));
            }
            if let Err(e) = stdin.flush() {
                return Err(AppError::Internal(format!("shap flush: {e}")));
            }
        }
        let mut stdout_guard = state.stdout.lock()
            .map_err(|e| format!("stdin lock: {e}"))
            .map_err(AppError::Internal)?;
        let stdout = stdout_guard.as_mut()
            .ok_or_else(|| AppError::Internal("stdout not available".into()))?;
        use std::io::{BufRead, BufReader};
        let mut reader = BufReader::new(stdout);
        let mut buf = String::new();
        if let Err(e) = reader.read_line(&mut buf) {
            return Err(AppError::Internal(format!("shap read: {e}")));
        }
        buf
    };

    let parsed = parse_line(&response_line).map_err(|e| {
        AppError::Internal(format!("shap parse: {e}"))
    })?;
    let response = match parsed {
        crate::domain::lab::sidecar::ParseResult::Response(r) => r,
        _ => return Err(AppError::Internal("shap: not a response".into())),
    };
    if response.id != job_id {
        return Err(AppError::Internal(format!(
            "shap id mismatch: sent={job_id}, got={}",
            response.id
        )));
    }
    parse_shap_explain_response(&response).map_err(|e| {
        AppError::Internal(format!("shap decode: {e}"))
    })
}

/// 发送任意 `SidecarRequest` 并返回原始响应。
/// 适用于 `ping` 及其他轻量方法。
#[tauri::command]
pub async fn sidecar_request(
    state: State<'_, SidecarState>,
    request: SidecarRequest,
) -> AppResult<SidecarResponse> {
    check_sidecar_enabled()?;
    if !state.is_running() {
        return Err(AppError::Internal("sidecar not running".into()));
    }
    let line = serde_json::to_string(&request).map_err(|e| {
        AppError::Internal(format!("encode: {e}"))
    })?;
    {
        let mut stdin_guard = state.stdin.lock().unwrap();
        let Some(stdin) = stdin_guard.as_mut() else {
            return Err(AppError::Internal("stdin not available".into()));
        };
        use std::io::Write;
        if let Err(e) = writeln!(stdin, "{line}") {
            return Err(AppError::Internal(format!("sidecar write: {e}")));
        }
        if let Err(e) = stdin.flush() {
            return Err(AppError::Internal(format!("sidecar flush: {e}")));
        }
    }
    let mut response_line = String::new();
    {
        let mut stdout_guard = state.stdout.lock().unwrap();
        let Some(stdout) = stdout_guard.as_mut() else {
            return Err(AppError::Internal("stdout not available".into()));
        };
        use std::io::{BufRead, BufReader};
        let mut reader = BufReader::new(stdout);
        if let Err(e) = reader.read_line(&mut response_line) {
            return Err(AppError::Internal(format!("sidecar read: {e}")));
        }
    }
    let parsed = parse_line(&response_line).map_err(|e| {
        AppError::Internal(format!("sidecar parse: {e}"))
    })?;
    match parsed {
        crate::domain::lab::sidecar::ParseResult::Response(r) => Ok(r),
        _ => Err(AppError::Internal("not a response".into())),
    }
}

/// 默认的已知方法列表（供 L1 自动发现使用）。
pub fn known_methods() -> Vec<SidecarMethod> {
    vec![
        SidecarMethod::Ping,
        SidecarMethod::Predict,
        SidecarMethod::TrainJob,
        SidecarMethod::PromoteModel,
    ]
}

// =================================================================
// v0.33b —— list_promote_history_archive（读取被裁掉的条目）
// =================================================================

/// v0.33b —— `list_promote_history_archive` IPC 的入参。
/// 镜像 Python 侧车的 archive.jsonl 格式。所有字段均可选；
/// L1 可通过 `offset` + `limit` 分页，或按
/// `from_ms` / `to_ms` 过滤。
#[derive(Debug, Clone, Deserialize)]
pub struct ListPromoteHistoryArchiveArgs {
    /// `promoted_at_ms` 的可选下界。默认 0（无下界）。
    pub from_ms: Option<i64>,
    /// `promoted_at_ms` 的可选上界。默认 i64::MAX（无上界）。
    pub to_ms: Option<i64>,
    /// 分页偏移。默认 0。
    pub offset: Option<usize>,
    /// 分页大小。默认 100（上限 1000）。
    pub limit: Option<usize>,
    /// v0.42e-3 —— 可选的 job_ids 白名单。提供时，
    /// 结果仅包含 `job_id` 属于该集合的条目。
    /// `ModelComparison` 组件用它在不取整个 archive 的
    /// 前提下获取 2-3 个选中条目的权重。空数组 = 无条目；
    /// 缺省 = 不过滤（返回全部）。
    #[serde(default)]
    pub job_ids: Option<Vec<String>>,
}

/// v0.33b —— Python 侧车 archive.jsonl 的传输格式镜像。
/// 每条对应 JSONL 文件的一行（即一次被归档的提升）。
/// 字段镜像 `PromoteHistoryEntry`，并新增 `archived_at_ms`
/// （条目写入 archive 的时间）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromoteHistoryArchiveEntry {
    pub job_id: String,
    pub model_version: String,
    pub promoted_at_ms: i64,
    pub best_brier: Option<f64>,
    pub best_params: Option<serde_json::Value>,
    pub weights: Option<serde_json::Value>,
    pub trial_index: Option<usize>,
    /// v0.33b —— 该条目写入 archive 文件的时间。
    /// 若侧车当时离线、稍后才写入，可能与 `promoted_at_ms` 不同。
    pub archived_at_ms: i64,
}

/// v0.33b —— `list_promote_history_archive` 的响应。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromoteHistoryArchiveResult {
    /// `true` 表示 archive 文件存在并可读。
    pub ok: bool,
    /// 过滤后（已分页）的所有条目。
    pub entries: Vec<PromoteHistoryArchiveEntry>,
    /// 文件中的总条目数（分页之前）。
    pub total: usize,
    /// 可选消息（错误或「暂无 archive」）。
    pub message: Option<String>,
}

/// v0.33b —— 读取 Python 侧车的 `archive.jsonl` 文件
/// 并返回分页条目。文件位于
/// `~/.polyrocket/sidecar/models/archive.jsonl`
/// （可通过 `POLYROCKET_SIDECAR_MODEL_DIR` 覆盖）。
///
/// 文件为 JSONL：每行一个 JSON 对象。我们逐行解析，
/// 按 `from_ms`/`to_ms` 过滤，返回从 `offset` 开始
/// 最多 `limit` 条。结果按 `promoted_at_ms` 倒序
/// （最新的在最前面）。
///
/// `promotion_history[]` 的 20 条上限是主要的内存审计轨迹。
/// archive 文件是持久的长期轨迹。L1 在 ModelLab 页面上
/// 提供「查看 archive」入口，可以看到被挤出 20 条上限的那些条目。
#[tauri::command]
pub async fn list_promote_history_archive(
    args: ListPromoteHistoryArchiveArgs,
) -> AppResult<PromoteHistoryArchiveResult> {
    check_sidecar_enabled()?;
    use std::io::{BufRead, BufReader};
    // v0.33b —— 用 Python 侧车所用的同一个环境变量
    // 推导 archive 路径。未设置时默认
    // ~/.polyrocket/sidecar/models/archive.jsonl。
    let model_dir = std::env::var("POLYROCKET_SIDECAR_MODEL_DIR").unwrap_or_else(|_| {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        format!("{home}/.polyrocket/sidecar/models")
    });
    let archive_path = std::path::PathBuf::from(model_dir).join("archive.jsonl");

    if !archive_path.exists() {
        return Ok(PromoteHistoryArchiveResult {
            ok: true,
            entries: Vec::new(),
            total: 0,
            message: Some("no archive yet; archive is created on first overflow".into()),
        });
    }

    let from_ms = args.from_ms.unwrap_or(0);
    let to_ms = args.to_ms.unwrap_or(i64::MAX);
    let offset = args.offset.unwrap_or(0);
    let limit = args.limit.unwrap_or(100).min(1000);
    // v0.42e-3 —— 若调用方传入了 job_ids 白名单，
    // 构建一个 HashSet 以实现 O(1) 查询。
    // None = 不过滤（返回时间范围内的全部）。
    let job_ids_filter: Option<std::collections::HashSet<String>> =
        args.job_ids.as_ref().map(|v| v.iter().cloned().collect());

    let file = match std::fs::File::open(&archive_path) {
        Ok(f) => f,
        Err(e) => {
            return Ok(PromoteHistoryArchiveResult {
                ok: false,
                entries: Vec::new(),
                total: 0,
                message: Some(format!("failed to open archive: {e}")),
            });
        }
    };

    // 解析所有行、过滤、按时间倒序排序、再分页。
    // 对一个长期 archive（数千条）这可能偏慢；
    // 对预期场景（每年几百条）已经足够。
    let mut all: Vec<PromoteHistoryArchiveEntry> = Vec::new();
    let reader = BufReader::new(file);
    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue, // 静默跳过格式错误的行
        };
        if line.trim().is_empty() {
            continue;
        }
        let entry: PromoteHistoryArchiveEntry = match serde_json::from_str(&line) {
            Ok(e) => e,
            Err(_) => continue, // 静默跳过格式错误的行
        };
        if entry.promoted_at_ms < from_ms || entry.promoted_at_ms > to_ms {
            continue;
        }
        // v0.42e-3 —— 若设置了 job_ids 白名单则应用。
        // 空白名单返回 0 条；缺省 = 不过滤。
        if let Some(set) = &job_ids_filter {
            if !set.contains(&entry.job_id) {
                continue;
            }
        }
        all.push(entry);
    }
    // 最新的排在前面
    all.sort_by(|a, b| b.promoted_at_ms.cmp(&a.promoted_at_ms));
    let total = all.len();
    let entries: Vec<PromoteHistoryArchiveEntry> = all
        .into_iter()
        .skip(offset)
        .take(limit)
        .collect();

    Ok(PromoteHistoryArchiveResult {
        ok: true,
        entries,
        total,
        message: None,
    })
}

// =================================================================
// v0.28a —— auto-promote 配置（内存中，由 L1 IPC 设置）
// =================================================================

/// v0.28a —— `set_auto_promote_config` 的入参。
/// 两个字段都是可选的：`None` 意为「保持不变」，
/// 这样 L1 只能更新用户在 UI 中改动的那一项
/// （例如只改开关、不动 margin）。
#[derive(Debug, Clone, Deserialize, specta::Type)]
pub struct SetAutoPromoteConfigArgs {
    pub enabled: Option<bool>,
    pub brier_margin: Option<f64>,
}

/// v0.28a —— 当前的 auto-promote 配置。
/// 由 `get_auto_promote_config` 返回给 L1，用于展示
/// 「Rust 端现在持有的是什么」（例如硬刷新导致
/// L1 store 被重置时）。
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct AutoPromoteConfigDto {
    pub enabled: bool,
    pub brier_margin: f64,
}

/// v0.28a —— L1 把用户的 auto-promote 设置推入
/// `AppState`，以便 Rust 端的 `train_job` handler 决定
/// 是否派生 auto-promote worker。
///
/// 两个字段均可选；只更新提供的字段。返回合并后的新
/// 配置，让 L1 可以确认 Rust 当前持有的状态。
#[tauri::command]
pub async fn set_auto_promote_config(
    state: State<'_, crate::infra::state::AppState>,
    args: SetAutoPromoteConfigArgs,
) -> AppResult<AutoPromoteConfigDto> {
    let mut guard = state
        .auto_promote
        .lock()
        .map_err(|e| AppError::Internal(format!("auto_promote lock: {e}")))?;
    if let Some(e) = args.enabled {
        guard.enabled = e;
    }
    if let Some(m) = args.brier_margin {
        // 把 margin 钳制在合理区间：0.0（任意提升即可）到
        // 0.1（必须好 10% 才提升）。负值相当于
        // 「只要不比当前更差就提升」，没什么意义。
        guard.brier_margin = m.clamp(0.0, 0.1);
    }
    Ok(AutoPromoteConfigDto {
        enabled: guard.enabled,
        brier_margin: guard.brier_margin,
    })
}

// =================================================================
// v0.42c —— telemetry 启用开关（L1 IPC）
// =================================================================

/// v0.42c —— 运行时覆盖 telemetry 的开 / 关状态。
/// L1 在 Settings 开关中调用本接口。默认（false）是 `init_from_env`
/// 留下的值，除非启动时设置了 `POLYROCKET_TELEMETRY=1`。
/// 本调用之后，用户完全控制 —— 环境变量对本进程不再起作用。
#[tauri::command]
pub async fn set_telemetry_enabled(args: SetTelemetryEnabledArgs) -> AppResult<bool> {
    crate::infra::telemetry::set_enabled(args.enabled);
    Ok(args.enabled)
}

/// v0.42c —— 读取当前 telemetry 状态。L1 在 Settings
/// 页面挂载时调用本接口，使开关能反映「Rust 侧当前持有
/// 的状态」（应对启动时由环境变量设定的情形）。
#[tauri::command]
pub async fn get_telemetry_enabled() -> AppResult<bool> {
    Ok(crate::infra::telemetry::is_enabled())
}

/// v0.42c —— `set_telemetry_enabled` 的入参。
/// 使用结构体而非裸 bool 是为了将来扩展：
/// 比如新增 `sink: Option<String>` 让 L1 在
/// stderr / 文件 / no-op 之间切换时无需重新设计 IPC。
#[derive(Debug, Clone, Deserialize)]
pub struct SetTelemetryEnabledArgs {
    pub enabled: bool,
}

/// v0.28a —— 从 `AppState` 读取当前 auto-promote 配置。
/// 若 L1 从未推过任何配置则返回默认值。
#[tauri::command]
pub async fn get_auto_promote_config(
    state: State<'_, crate::infra::state::AppState>,
) -> AppResult<AutoPromoteConfigDto> {
    let guard = state
        .auto_promote
        .lock()
        .map_err(|e| AppError::Internal(format!("auto_promote lock: {e}")))?;
    Ok(AutoPromoteConfigDto {
        enabled: guard.enabled,
        brier_margin: guard.brier_margin,
    })
}

/// v0.28a —— 后台 auto-promote worker 的事件 payload。
/// 在 Tauri 总线上以 `auto_promote:finished` 发射。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoPromoteFinishedEvent {
    /// 触发本次 auto-promote 的 train job_id。
    pub job_id: String,
    /// auto-promote 是否真的完成了 promote。
    pub promoted: bool,
    /// 来自侧车的 reason / status 字符串。
    pub message: String,
    /// 若完成 promote,新模型版本。
    pub model_version: Option<String>,
    /// auto-promote 完成的时间（unix 毫秒）。
    pub finished_at: i64,
}

/// v0.28a —— 在后台任务中调用侧车的 `auto_promote_if_better`,
/// 然后发射 `auto_promote:finished`。
///
/// 这是一个私有辅助函数，由 `train_job` 在侧车返回
/// 一次成功 train 之后调用。它不是 `#[tauri::command]` ——
///
/// 运行在 `tokio::spawn` 派生的任务中，因此 train IPC 会
/// 立即返回。
///
/// 工作流程：
/// 1. 通过与面向用户的命令相同的 stdin/stdout 协议调用
///    `auto_promote_if_better(brier_margin)`
/// 2. 在 `auto_promote:finished` 上发射结果
/// 3. 静默吞掉错误（错误信息会反映在 `message` 中；
///    我们不希望一次失败的 auto-promote 把已经返回的
///    train IPC 也搞崩）
async fn run_auto_promote_worker(
    sidecar: SidecarState,
    app: AppHandle,
    brier_margin: f64,
    job_id: String,
) {
    let inner_job_id = format!(
        "auto-promote-{}",
        uuid::Uuid::new_v4()
            .to_string()
            .split('-')
            .next()
            .unwrap_or("00000000")
    );
    let line = build_auto_promote_if_better_request(&inner_job_id, Some(brier_margin), None);

    // 如果侧车未运行，worker 就直接发射一个
    // "no-op" 的 finished 事件，便于 L1 更新
    // 自身的 UI（例如「auto-promote skipped: sidecar down」）。
    if !sidecar.is_running() {
        let _ = app.emit(
            "auto_promote:finished",
            AutoPromoteFinishedEvent {
                job_id,
                promoted: false,
                message: "sidecar not running".into(),
                model_version: None,
                finished_at: chrono::Utc::now().timestamp_millis(),
            },
        );
        return;
    }

    let response_line = {
        let write_result: Result<String, String> = (|| -> Result<String, String> {
            let mut stdin_guard = sidecar
                .stdin
                .lock()
                .map_err(|e| format!("stdin lock: {e}"))?;
            let stdin = stdin_guard
                .as_mut()
                .ok_or_else(|| "stdin not available".to_string())?;
            use std::io::Write;
            writeln!(stdin, "{line}").map_err(|e| format!("auto_promote write: {e}"))?;
            stdin.flush().map_err(|e| format!("auto_promote flush: {e}"))?;
            drop(stdin_guard);

            let mut stdout_guard = sidecar
                .stdout
                .lock()
                .map_err(|e| format!("stdout lock: {e}"))?;
            let stdout = stdout_guard
                .as_mut()
                .ok_or_else(|| "stdout not available".to_string())?;
            use std::io::{BufRead, BufReader};
            let mut reader = BufReader::new(stdout);
            let mut response_line = String::new();
            reader
                .read_line(&mut response_line)
                .map_err(|e| format!("auto_promote read: {e}"))?;
            Ok(response_line)
        })();
        match write_result {
            Ok(l) => l,
            Err(e) => {
                let _ = app.emit(
                    "auto_promote:finished",
                    AutoPromoteFinishedEvent {
                        job_id,
                        promoted: false,
                        message: format!("auto_promote worker error: {e}"),
                        model_version: None,
                        finished_at: chrono::Utc::now().timestamp_millis(),
                    },
                );
                return;
            }
        }
    };

    let parsed = match parse_line(&response_line) {
        Ok(p) => p,
        Err(e) => {
            let _ = app.emit(
                "auto_promote:finished",
                AutoPromoteFinishedEvent {
                    job_id,
                    promoted: false,
                    message: format!("auto_promote parse: {e}"),
                    model_version: None,
                    finished_at: chrono::Utc::now().timestamp_millis(),
                },
            );
            return;
        }
    };
    let response = match parsed {
        crate::domain::lab::sidecar::ParseResult::Response(r) => r,
        _ => {
            let _ = app.emit(
                "auto_promote:finished",
                AutoPromoteFinishedEvent {
                    job_id,
                    promoted: false,
                    message: "auto_promote: not a response".into(),
                    model_version: None,
                    finished_at: chrono::Utc::now().timestamp_millis(),
                },
            );
            return;
        }
    };
    if response.id != inner_job_id {
        let _ = app.emit(
            "auto_promote:finished",
            AutoPromoteFinishedEvent {
                job_id,
                promoted: false,
                message: format!(
                    "auto_promote id mismatch: sent={inner_job_id}, got={}",
                    response.id
                ),
                model_version: None,
                finished_at: chrono::Utc::now().timestamp_millis(),
            },
        );
        return;
    }
    let result: AutoPromoteIfBetterResult = match parse_auto_promote_if_better_response(&response) {
        Ok(r) => r,
        Err(e) => {
            let _ = app.emit(
                "auto_promote:finished",
                AutoPromoteFinishedEvent {
                    job_id,
                    promoted: false,
                    message: format!("auto_promote decode: {e}"),
                    model_version: None,
                    finished_at: chrono::Utc::now().timestamp_millis(),
                },
            );
            return;
        }
    };

    let _ = app.emit(
        "auto_promote:finished",
        AutoPromoteFinishedEvent {
            job_id: job_id.clone(),
            promoted: result.promoted,
            message: result.message.clone().unwrap_or_default(),
            model_version: if result.promoted {
                result.model_version.clone()
            } else {
                None
            },
            finished_at: chrono::Utc::now().timestamp_millis(),
        },
    );
    // v0.42b —— 发射生命周期事件。系统通知路径
    // 不变（v0.39a 仅在 `promoted: true` 时触发）；
    // telemetry 同时捕获两种结果用于分析。
    // 后续 v0.42e 可能会加入一个「跳过时也通知」
    // 的开关，复用 AutoPromoteSkipped 事件。
    use crate::infra::telemetry;
    if result.promoted {
        telemetry::emit(telemetry::Event::AutoPromoteFired {
            job_id,
            model_version: result.model_version.unwrap_or_default(),
            message: result.message.unwrap_or_default(),
        });
    } else {
        telemetry::emit(telemetry::Event::AutoPromoteSkipped {
            job_id,
            message: result.message.unwrap_or_default(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sidecar_status_default_has_no_pid() {
        let s = SidecarStatus::default();
        assert!(!s.running);
        assert!(s.pid.is_none());
    }

    #[test]
    fn known_methods_all_unique() {
        let m = known_methods();
        let mut v: Vec<_> = m.iter().map(|x| x.as_str()).collect();
        v.sort();
        v.dedup();
        assert_eq!(v.len(), m.len());
    }

    // ============================================================
    // v0.33b —— list_promote_history_archive
    // ============================================================

    use std::io::Write;

    /// 辅助函数：写入包含 N 条记录的 JSONL archive 文件。
    fn write_test_archive(path: &std::path::Path, n: usize) {
        let mut f = std::fs::File::create(path).unwrap();
        for i in 0..n {
            let entry = serde_json::json!({
                "job_id": format!("train-{:08x}", i),
                "model_version": format!("logistic-train-{:08x}", i),
                "promoted_at_ms": 1_700_000_000_000_i64 + (i as i64) * 1000,
                "best_brier": 0.20 - (i as f64) * 0.001,
                "best_params": {"lr": 0.01, "reg": 0.001},
                "weights": {"w0": -0.5, "w1": 2.0, "w2": 0.4},
                "trial_index": if i % 2 == 0 { serde_json::Value::Null } else { serde_json::json!(i / 2) },
                "archived_at_ms": 1_700_000_000_000_i64 + (i as i64) * 1000,
            });
            writeln!(f, "{}", entry.to_string()).unwrap();
        }
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn archive_returns_empty_when_no_file() {
        // v0.33b —— 如果 archive.jsonl 不存在,返回
        // ok=true 且 0 条记录,并附一条友好提示
        let tmp = std::env::temp_dir().join(format!(
            "polyrocket_test_archive_none_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        std::env::set_var("POLYROCKET_SIDECAR_MODEL_DIR", &tmp);

        let r = list_promote_history_archive(ListPromoteHistoryArchiveArgs {
            from_ms: None,
            to_ms: None,
            offset: None,
            limit: None,
            job_ids: None,
        })
        .await
        .unwrap();
        assert!(r.ok);
        assert_eq!(r.entries.len(), 0);
        assert_eq!(r.total, 0);
        assert!(r.message.is_some());
        assert!(r.message.as_ref().unwrap().contains("no archive"));

        std::env::remove_var("POLYROCKET_SIDECAR_MODEL_DIR");
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn archive_reads_and_paginates_entries() {
        // v0.33b —— 写入 25 条，用默认 limit=100 读取，
        // 验证 25 条全部返回、按时间倒序
        let tmp = std::env::temp_dir().join(format!(
            "polyrocket_test_archive_25_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        write_test_archive(&tmp.join("archive.jsonl"), 25);
        std::env::set_var("POLYROCKET_SIDECAR_MODEL_DIR", &tmp);

        let r = list_promote_history_archive(ListPromoteHistoryArchiveArgs {
            from_ms: None,
            to_ms: None,
            offset: None,
            limit: None,
            job_ids: None,
        })
        .await
        .unwrap();
        assert!(r.ok);
        assert_eq!(r.total, 25);
        assert_eq!(r.entries.len(), 25);
        // 最新的排在前面：entry[0] 是第 25 条写入的（i=24）
        // job_id 为 `train-00000018`（24 的十六进制，0 补齐到 8 位）
        assert_eq!(r.entries[0].job_id, "train-00000018");
        // 最后一条是最早的（i=0）
        assert_eq!(r.entries[24].job_id, "train-00000000");
        // 每条都包含所有必需字段
        assert!(r.entries[0].archived_at_ms > 0);
        assert!(r.entries[0].weights.is_some());

        std::env::remove_var("POLYROCKET_SIDECAR_MODEL_DIR");
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn archive_pagination_offset_and_limit() {
        // v0.33b —— 写入 30 条记录，用 offset=10 limit=5 读取，
        // 验证返回 5 条（newest-first 列表的索引 10..15，
        // 也就是按时间从老到新的第 16 到 20 条）
        let tmp = std::env::temp_dir().join(format!(
            "polyrocket_test_archive_pag_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        write_test_archive(&tmp.join("archive.jsonl"), 30);
        std::env::set_var("POLYROCKET_SIDECAR_MODEL_DIR", &tmp);

        let r = list_promote_history_archive(ListPromoteHistoryArchiveArgs {
            from_ms: None,
            to_ms: None,
            offset: Some(10),
            limit: Some(5),
            job_ids: None,
        })
        .await
        .unwrap();
        assert!(r.ok);
        assert_eq!(r.total, 30);
        assert_eq!(r.entries.len(), 5);

        std::env::remove_var("POLYROCKET_SIDECAR_MODEL_DIR");
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn archive_filters_by_time_range() {
        // v0.33b —— 以 1000ms 间隔写入 10 条，
        // 用 from_ms / to_ms 过滤为 5 条（i=3..7）
        let tmp = std::env::temp_dir().join(format!(
            "polyrocket_test_archive_time_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        write_test_archive(&tmp.join("archive.jsonl"), 10);
        std::env::set_var("POLYROCKET_SIDECAR_MODEL_DIR", &tmp);

        // i=3 对应 1_700_000_003_000，i=7 对应 1_700_000_007_000
        let r = list_promote_history_archive(ListPromoteHistoryArchiveArgs {
            from_ms: Some(1_700_000_003_000),
            to_ms: Some(1_700_000_007_000),
            offset: None,
            limit: None,
            job_ids: None,
        })
        .await
        .unwrap();
        assert!(r.ok);
        // 命中 5 条：i=3,4,5,6,7
        assert_eq!(r.total, 5);
        assert_eq!(r.entries.len(), 5);

        std::env::remove_var("POLYROCKET_SIDECAR_MODEL_DIR");
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn archive_filters_by_job_ids() {
        // v0.42e-3 —— 对 job_ids 应用白名单过滤。
        // ModelComparison 模态框用它来
        // 在不取整个 archive 的前提下
        // 取出 2-3 个被选条目的权重。
        let tmp = std::env::temp_dir().join(format!(
            "polyrocket_test_archive_jobids_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        write_test_archive(&tmp.join("archive.jsonl"), 10);
        std::env::set_var("POLYROCKET_SIDECAR_MODEL_DIR", &tmp);

        // 白名单：train-00000002、train-00000005、
        // train-00000008（辅助函数写入的 job_id 形如
        // "train-{:08x}"，因此 i=2 → 00000002）。
        let r = list_promote_history_archive(ListPromoteHistoryArchiveArgs {
            from_ms: None,
            to_ms: None,
            offset: None,
            limit: None,
            job_ids: Some(vec![
                "train-00000002".into(),
                "train-00000005".into(),
                "train-00000008".into(),
            ]),
        })
        .await
        .unwrap();
        assert!(r.ok);
        // total 在分页前统计
        assert_eq!(r.total, 3);
        assert_eq!(r.entries.len(), 3);
        let ids: std::collections::HashSet<String> =
            r.entries.iter().map(|e| e.job_id.clone()).collect();
        assert!(ids.contains("train-00000002"));
        assert!(ids.contains("train-00000005"));
        assert!(ids.contains("train-00000008"));

        // 空白名单 → 0 条
        let r2 = list_promote_history_archive(ListPromoteHistoryArchiveArgs {
            from_ms: None,
            to_ms: None,
            offset: None,
            limit: None,
            job_ids: Some(vec![]),
        })
        .await
        .unwrap();
        assert_eq!(r2.total, 0);
        assert_eq!(r2.entries.len(), 0);

        // 白名单无任何匹配 → 0 条
        let r3 = list_promote_history_archive(ListPromoteHistoryArchiveArgs {
            from_ms: None,
            to_ms: None,
            offset: None,
            limit: None,
            job_ids: Some(vec!["train-00000999".into()]),
        })
        .await
        .unwrap();
        assert_eq!(r3.total, 0);
        assert_eq!(r3.entries.len(), 0);

        // 白名单与 from_ms 同时使用 —— 两个过滤
        // 必须都生效
        let r4 = list_promote_history_archive(ListPromoteHistoryArchiveArgs {
            from_ms: Some(1_700_000_006_000),
            to_ms: None,
            offset: None,
            limit: None,
            job_ids: Some(vec![
                "train-00000002".into(),
                "train-00000005".into(),
                "train-00000008".into(),
            ]),
        })
        .await
        .unwrap();
        // 仅 train-00000008 留下（i=5 的时间戳是
        // 1_700_000_005_000，低于 from_ms）
        assert_eq!(r4.total, 1);
        assert_eq!(r4.entries[0].job_id, "train-00000008");

        std::env::remove_var("POLYROCKET_SIDECAR_MODEL_DIR");
        let _ = std::fs::remove_dir_all(&tmp);
    }
// ----- v0.122a：POLYROCKET_DISABLE_SIDECAR 短路 -----

    #[test]
    #[serial_test::serial]
    fn check_sidecar_enabled_passes_when_flag_unset() {
        // SAFETY：与其他会修改环境变量的测试串行执行。
        unsafe { std::env::remove_var("POLYROCKET_DISABLE_SIDECAR") };
        assert!(check_sidecar_enabled().is_ok(),
            "默认状态（开关未设置）必须放行侧车 IPC");
    }

    #[test]
    #[serial_test::serial]
    fn check_sidecar_enabled_blocks_when_flag_set() {
        // SAFETY：与其他会修改环境变量的测试串行执行。
        unsafe { std::env::set_var("POLYROCKET_DISABLE_SIDECAR", "1") };
        let err = check_sidecar_enabled()
            .expect_err("开关设置时必须短路返回");
        let msg = format!("{err:?}");
        assert!(
            msg.contains("sidecar disabled") && msg.contains("v0.122"),
            "错误信息必须包含规范的禁用提示，实际: {msg}"
        );
        unsafe { std::env::remove_var("POLYROCKET_DISABLE_SIDECAR") };
    }
}