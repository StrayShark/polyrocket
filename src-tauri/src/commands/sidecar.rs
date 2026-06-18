//! L2 — Python sidecar process manager (M7).
//!
//! Spawns the optional `polyrocket-sidecar` Python process, sends
//! JSON-line requests, and tracks health. All protocol details are
//! in `domain::lab::sidecar`; this module handles the OS process.

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
use serde::{Deserialize, Serialize};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, State};

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    /// Path to the sidecar binary / python script. Defaults to
    /// `POLYROCKET_SIDECAR_CMD` env or "polyrocket-sidecar".
    pub command: Option<String>,
    pub args: Option<Vec<String>>,
}

/// Wrap the OS process + pipes in a small state struct.
///
/// v0.28a — fields are `Arc<Mutex<...>>` so the struct is
/// cheaply `Clone` (one Arc bump per field). This lets the
/// auto-promote worker in `train_job` clone the sidecar
/// state into a background `tokio::spawn` task without
/// moving the original out of Tauri's `State`.
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

    /// v0.11b — Send a `ping` to the running sidecar via stdin, read
    /// one line from stdout, return `Ok(latency_ms)` on success.
    ///
    /// This is a synchronous helper used by the health-probe scheduler
    /// loop. It MUST be called from a blocking context (e.g. via
    /// `tokio::task::spawn_blocking`) because it holds the stdin +
    /// stdout mutexes for the duration of the read.
    ///
    /// Returns:
    ///   - `Ok(latency_ms)` if we got a `pong` back within the timeout
    ///   - `Err(String)`     otherwise (process not running, write/read
    ///                        failed, timeout, parse error, etc.)
    ///
    /// Lock discipline: hold stdin only while writing, hold stdout
    /// only while reading. This way other code paths (e.g. user-initiated
    /// `sidecar_predict`) can interleave their own I/O without
    /// deadlocking.
    pub fn ping_blocking(&self, timeout_ms: u64) -> Result<u64, String> {
        use std::io::{BufRead, BufReader, Write};
        use std::time::{Duration, Instant};

        // Build the JSON-line request
        let id = format!("sweeper-{}", chrono::Utc::now().timestamp_millis());
        let payload = serde_json::json!({
            "id": id,
            "method": "ping",
            "params": {},
        });
        let line = serde_json::to_string(&payload).map_err(|e| format!("encode: {e}"))?;

        let started = Instant::now();
        let deadline = Duration::from_millis(timeout_ms);

        // Write + flush under stdin lock, then release.
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

        // Read one line under stdout lock, then release.
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

        // Check the deadline AFTER releasing the lock. (Approximate —
        // a hung read can block past the deadline, but in practice the
        // Python sidecar responds in <50ms and the pipe is line-
        // buffered, so read_line returns as soon as '\n' arrives.)
        if started.elapsed() > deadline {
            return Err(format!("timeout after {}ms", started.elapsed().as_millis()));
        }
        if response.trim().is_empty() {
            return Err("empty response".into());
        }

        // Best-effort parse: we just check that the response has
        // `ok: true`. We don't correlate the id because v0.11b is
        // the only writer; future versions with concurrent probes
        // will need a per-id oneshot channel.
        let parsed: serde_json::Value = serde_json::from_str(response.trim())
            .map_err(|e| format!("parse: {e}"))?;
        if parsed.get("ok").and_then(|v| v.as_bool()) != Some(true) {
            return Err(format!("not ok: {response}"));
        }
        Ok(started.elapsed().as_millis() as u64)
    }

    /// v0.12c — async-friendly wrapper around `ping_blocking`.
    ///
    /// The blocking helper holds a sync mutex; calling it directly
    /// from the async runtime would block the worker thread. This
    /// wrapper uses `tokio::task::spawn_blocking` to run the I/O
    /// off the runtime, with `tokio::time::timeout` for an enforced
    /// wall-clock deadline (the blocking helper's timeout is best-
    /// effort because the read can race the deadline).
    ///
    /// Returns the latency in ms on success, or an error string
    /// describing the failure.
    pub async fn ping_async(&self, timeout_ms: u64) -> Result<u64, String> {
        let this = Self {
            child: Arc::new(Mutex::new(None)),  // ping_async doesn't need the child
            stdin: Arc::new(Mutex::new(None)),
            stdout: Arc::new(Mutex::new(None)),
            status: Arc::new(Mutex::new(self.status.lock().map_err(|e| format!("status lock: {e}"))?.clone())),
        };
        // Move the real stdin/stdout into the spawned task by
        // swapping the contents. This is safe because we hold
        // no other references and we're on a single-threaded
        // async runtime per call.
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

    /// v0.13d — blocking version of `sidecar_predict`.
    ///
    /// Same lock discipline as `ping_blocking`: hold stdin only while
    /// writing, hold stdout only while reading. Falls back to empty
    /// Vec if stdin/stdout is unavailable (sidecar not running).
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

    /// v0.13d — async wrapper around `predict_blocking`.
    ///
    /// Same pattern as `ping_async`: `spawn_blocking` for the I/O,
    /// `tokio::time::timeout` for the wall-clock deadline. The
    /// returned `PredictResult` includes the model_version and
    /// brier_score that v0.12a / v0.13b added.
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

/// Start the sidecar. If already running, no-op.
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
    // v0.56 — propagate the proxy to the
    // sidecar. We set the standard env vars
    // (HTTP_PROXY / HTTPS_PROXY / ALL_PROXY) so
    // that any HTTP library the sidecar uses
    // (httpx, requests) automatically routes
    // through it. We only set these when the
    // Rust-side proxy is enabled; the env-var
    // path is opt-in to keep the default
    // (direct outbound) intact.
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

/// Stop the sidecar (SIGKILL equivalent).
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

/// Return current sidecar status.
#[tauri::command]
pub async fn sidecar_status(state: State<'_, SidecarState>) -> AppResult<SidecarStatus> {
    read_status(&state)
}

fn read_status(state: &SidecarState) -> AppResult<SidecarStatus> {
    let s = state.status.lock().unwrap().clone();
    Ok(s)
}

/// Send a `predict` request to the sidecar. Returns parsed predictions.
/// In v0.6b this is a best-effort: if no sidecar is running, returns
/// empty Vec (caller falls back to the heuristic domain::signal).
#[tauri::command]
pub async fn sidecar_predict(
    state: State<'_, SidecarState>,
    markets: Vec<(String, f64)>,
) -> AppResult<Vec<Prediction>> {
    if !state.is_running() {
        return Ok(Vec::new());
    }
    // Build the request and try to read a response.
    let id = format!("pred_{}", chrono::Utc::now().timestamp_millis());
    let line = build_predict_request(&id, &markets);

    // Send the line via stdin (synchronous std::io::Write; we hold the
    // lock briefly). If the sidecar has died, fall back to empty.
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
    // Read a single line from stdout. In v0.6b this is a one-shot
    // synchronous read; a real impl would use async I/O and a
    // correlation table (request id → oneshot).
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
        // Mismatched id; in v0.6c we'll fix with proper correlation.
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

/// v0.13d — async-friendly version of `sidecar_predict` that
/// returns the full `PredictResult` (with model_version +
/// brier_score). Uses `spawn_blocking` + `tokio::time::timeout`
/// like `ping_async`. Falls back to an empty `PredictResult` when
/// the sidecar is not running (matches the v0.6b semantics).
#[tauri::command]
pub async fn sidecar_predict_async(
    state: State<'_, SidecarState>,
    markets: Vec<(String, f64)>,
    timeout_ms: Option<u64>,
) -> AppResult<crate::domain::lab::sidecar::PredictResult> {
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
// ============== v0.17a — train_job IPC + progress events ==========
// =================================================================

/// Args for the `train_job` IPC. v0.17a — mirrors the Python
/// sidecar's optional params. All fields are optional; the
/// Python sidecar uses sensible defaults (n_trials=4, epochs=80).
#[derive(Debug, Clone, Deserialize)]
pub struct TrainJobArgs {
    /// Default 4 (max 4 in v0.17a; the grid is 4 hardcoded
    /// (lr, reg) combinations in `train.py`).
    pub n_trials: Option<u32>,
    /// Default 80. Per-trial training epochs.
    pub epochs: Option<u32>,
    /// Optional timeout in milliseconds for the IPC. Default
    /// 60s — the Python sweep is 4 × 80 epochs, usually 2-10s
    /// but can spike to 30s on a slow box.
    pub timeout_ms: Option<u64>,
}

/// v0.17a — kick off a training job on the Python sidecar.
///
/// Emits two events on the Tauri bus:
///   - `train_job:started`  — when the IPC is dispatched
///   - `train_job:finished` — when the sweep completes (or fails)
///
/// Returns the full `TrainResult` (job_id, status, best_brier,
/// best_params, trials, duration_ms, candidate_path, message).
///
/// Falls back to a "failed" result with no trials when the
/// sidecar is not running — matches the v0.6b predict fallback
/// (the L1 doesn't have to special-case "sidecar down").
#[tauri::command]
pub async fn train_job(
    state: State<'_, SidecarState>,
    app_state: State<'_, crate::infra::state::AppState>,
    app: AppHandle,
    args: TrainJobArgs,
) -> AppResult<TrainResult> {
    let job_id = format!("train-{}", uuid::Uuid::new_v4().to_string().split('-').next().unwrap_or("00000000"));
    let n_trials = args.n_trials.unwrap_or(4).clamp(1, 4);
    let epochs = args.epochs.unwrap_or(80).clamp(1, 1000);
    let timeout = args.timeout_ms.unwrap_or(60_000);
    let started_at = chrono::Utc::now().timestamp_millis();

    // v0.17a — emit started BEFORE the sidecar call so the L1
    // can immediately render the "Training…" pill.
    let _ = app.emit(
        "train_job:started",
        TrainStartedEvent {
            job_id: job_id.clone(),
            n_trials,
            epochs,
            started_at,
        },
    );
    // v0.42b — lifecycle event for the IPC accept. This
    // is the "user clicked Train" moment; it doesn't
    // mean the sidecar will succeed.
    use crate::infra::telemetry;
    telemetry::emit(telemetry::Event::TrainStarted {
        job_id: job_id.clone(),
        n_trials: n_trials as usize,
        epochs: epochs as usize,
    });

    if !state.is_running() {
        // Sidecar not running — emit a finished event with
        // status="failed" and a descriptive message, then
        // return the same shape so the L1 doesn't have to
        // handle a special "no sidecar" path.
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

    // Build the request line, write under stdin lock, read
    // one line under stdout lock. Same lock discipline as
    // `sidecar_predict` (v0.6b). The 4-trial sweep takes
    // 2-30s, well within the 60s timeout.
    let line = build_train_request(&job_id, Some(n_trials), Some(epochs));
    let started = std::time::Instant::now();
    let deadline = std::time::Duration::from_millis(timeout);

    let response_line = {
        // Write under stdin lock, release before reading.
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
        // Read one line under stdout lock.
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
        // v0.42b — emit lifecycle event. TrainFailed
        // captures the timeout distinctly from a
        // sidecar-decoded failure.
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
        // v0.42b — emit lifecycle event. Decode error
        // is treated as a failed train for telemetry.
        use crate::infra::telemetry;
        telemetry::emit(telemetry::Event::TrainFailed {
            job_id: job_id.clone(),
            error: e.to_string(),
        });
        AppError::Internal(format!("train_job decode: {e}"))
    })?;

    // v0.17a — emit finished with the parsed result.
    // v0.42b — emit TrainCompleted lifecycle event. We
    // pull the model_version from result.model_version
    // and best_brier straight off the parsed payload.
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

    // v0.28a — if auto-promote is enabled in AppState AND
    // the train succeeded, spawn a background worker that
    // calls `auto_promote_if_better` and emits the result
    // on `auto_promote:finished`. The train IPC returns
    // immediately; the worker runs in the background.
    //
    // The worker reads `state` (SidecarState) and `app` (AppHandle)
    // by cloning — both are cheap (SidecarState is just
    // Arc<Mutex<...>> internally; AppHandle is a clone of
    // a long-lived handle).
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

/// v0.18a — promote the current candidate to the active slot.
///
/// This is a fast, synchronous operation (~10ms file move).
/// No progress events. The IPC returns the full `PromoteResult`.
///
/// `args.job_id` is optional. If set, the Python sidecar
/// refuses to promote a candidate from a different job
/// (race-condition protection — protects against the case
/// where a second train finishes between the user's intent
/// to promote and the actual promote call).
///
/// Falls back to `promoted: false` with a descriptive message
/// when the sidecar is not running (matches the v0.17a train
/// fallback pattern).
#[tauri::command]
pub async fn promote_model(
    state: State<'_, SidecarState>,
    args: PromoteModelArgs,
) -> AppResult<PromoteResult> {
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

    // Same lock discipline as train_job: write under stdin
    // lock, read under stdout lock.
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
        // v0.42b — emit lifecycle event on successful
        // promote. We read the `reason` and `trial_index`
        // straight off the response. Skipped / failed
        // promotes don't emit (the OS notification path
        // already covers user-visible signal).
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

/// Args for the `promote_model` IPC. v0.18a — mirrors the
/// Python sidecar's optional `job_id` param.
/// v0.21a — added `trial_index` for bulk promote.
#[derive(Debug, Clone, Deserialize)]
pub struct PromoteModelArgs {
    /// If set, refuses to promote a candidate from a
    /// different job. Defaults to `None` (accept any
    /// current candidate).
    pub job_id: Option<String>,
    /// v0.21a — if set, promotes the n-th trial from
    /// `all_trials[]` instead of the best. 0-indexed.
    /// `None` (default) means "promote the best".
    pub trial_index: Option<usize>,
}

/// List the promote history. v0.19b — read-only audit.
/// No args; returns the last 20 promotions from active.json's
/// `promotion_history` array.
#[tauri::command]
pub async fn list_promote_history(
    state: State<'_, SidecarState>,
) -> AppResult<PromoteHistoryResult> {
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

    // Same lock discipline as train_job/promote_model.
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

/// Roll the active model back to a previous version. v0.20b.
#[tauri::command]
pub async fn rollback_model(
    state: State<'_, SidecarState>,
    args: RollbackModelArgs,
) -> AppResult<RollbackResult> {
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

/// Args for the `rollback_model` IPC. v0.20b — `model_version`
/// is the version to roll back to (looked up in the
/// `promotion_history` array).
#[derive(Debug, Clone, Deserialize)]
pub struct RollbackModelArgs {
    pub model_version: String,
}

/// Auto-promote the candidate only if it's meaningfully
/// better than the active model. v0.23b.
#[tauri::command]
pub async fn auto_promote_if_better(
    state: State<'_, SidecarState>,
    args: AutoPromoteIfBetterArgs,
) -> AppResult<AutoPromoteIfBetterResult> {
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

/// Args for the `auto_promote_if_better` IPC. v0.23b —
/// `brier_margin` is how much better the candidate must
/// be (lower Brier = better) for the auto-promote to
/// happen. Default 0.005. `trial_index` is which trial
/// to use (None = best).
#[derive(Debug, Clone, Deserialize)]
pub struct AutoPromoteIfBetterArgs {
    pub brier_margin: Option<f64>,
    pub trial_index: Option<usize>,
}

/// Bulk-promote every trial from the current candidate. v0.25b.
/// No args — the sidecar reads the candidate and promotes
/// every trial in `all_trials[]` in order. Returns a list
/// of per-trial results.
#[tauri::command]
pub async fn promote_all_trials(
    state: State<'_, SidecarState>,
) -> AppResult<PromoteAllTrialsResult> {
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
// ============== v0.43b — backtest_model IPC =======================
// =================================================================

/// v0.43b — args for the `backtest_model` IPC. The L1
/// pulls resolved markets from the markets DB, converts
/// each to a `BacktestSample`, and passes them in.
/// Returns a `BacktestResult` with Brier + calibration
/// + top winners/losers.
///
/// The sidecar is pure (no IO beyond reading the model
/// file), so the per-call cost is `O(samples)` — fast
/// for hundreds of samples, slow for millions. The
/// L1 should pre-filter to a reasonable time window.
#[derive(Debug, Clone, Deserialize)]
pub struct BacktestModelArgs {
    /// The model to backtest, e.g.
    /// "logistic-train-441c352b". Looked up in
    /// `archive.jsonl` first, then `active.json`.
    pub model_version: String,
    /// The list of (price, market_age_hours, outcome)
    /// samples to replay the model against. Each
    /// sample may also include a `label` for the
    /// top winners/losers display.
    pub samples: Vec<BacktestSample>,
}

/// v0.43b — replay a saved model against a list of
/// (price, market_age_hours, outcome) samples and
/// return Brier + calibration + per-sample
/// predictions. Closes the v0.17-v0.41 model
/// lifecycle gap: there's no way to ask
/// "how would this model have done on real
/// resolutions" without this.
///
/// The IPC's job is just protocol plumbing —
/// stdin/stdout lock + parse + return. The
/// prediction + Brier math lives in the Python
/// sidecar (v0.43a).
#[tauri::command]
pub async fn backtest_model(
    state: State<'_, SidecarState>,
    args: BacktestModelArgs,
) -> AppResult<BacktestResult> {
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
// v0.55 — explain_model IPC
// =================================================================

/// v0.55 — args for the `explain_model` IPC. The
/// L1 sends a model_version + optional sample;
/// the sidecar returns per-feature contributions
/// to the prediction. The L1 renders this as a
/// horizontal bar chart.
#[derive(Debug, Clone, Deserialize)]
pub struct ExplainModelArgs {
    /// The model to explain, e.g.
    /// "logistic-train-441c352b". Looked up in
    /// `archive.jsonl` first, then `active.json`.
    pub model_version: String,
    /// Optional sample: { price, market_age_hours }.
    /// When omitted, the sidecar uses a default
    /// sample (price=0.5, age=24h) so the user
    /// gets a "what would the model say for a
    /// typical market" view.
    #[serde(default)]
    pub sample: Option<ExplainSample>,
}

/// v0.55 — per-feature contribution for one sample.
/// For the 3-feature logistic model this is an
/// exact decomposition (not a SHAP approximation):
/// `contribution_i = w_i * x_i * p(1-p)`. The L1
/// renders the `features` array as a horizontal
/// bar chart (positive bars in green, negative
/// in red, length = `abs_contribution`).
#[tauri::command]
pub async fn explain_model(
    state: State<'_, SidecarState>,
    args: ExplainModelArgs,
) -> AppResult<ExplainResult> {
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
// v0.59 — shap_explain IPC (real SHAP via KernelExplainer)
// =================================================================

/// v0.59 — args for the `shap_explain` IPC. Same
/// shape as `ExplainModelArgs` (v0.55). The
/// sidecar returns per-feature SHAP values
/// that satisfy the efficiency axiom.
#[derive(Debug, Clone, Deserialize)]
pub struct ShapExplainArgs {
    /// The model to explain.
    pub model_version: String,
    /// Optional sample: { price, market_age_hours }.
    #[serde(default)]
    pub sample: Option<ExplainSample>,
}

/// v0.59 — compute true SHAP values for one
/// sample. The result satisfies the SHAP
/// efficiency axiom: `Σφ_i = f(x) - E[f(x)]`
/// (the deviation from the baseline
/// prediction).
#[tauri::command]
pub async fn shap_explain(
    state: State<'_, SidecarState>,
    args: ShapExplainArgs,
) -> AppResult<ShapResult> {
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

/// Send an arbitrary `SidecarRequest` and return the raw response.
/// Useful for `ping` and other lightweight methods.
#[tauri::command]
pub async fn sidecar_request(
    state: State<'_, SidecarState>,
    request: SidecarRequest,
) -> AppResult<SidecarResponse> {
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

/// Default list of well-known methods (for L1 auto-discovery).
pub fn known_methods() -> Vec<SidecarMethod> {
    vec![
        SidecarMethod::Ping,
        SidecarMethod::Predict,
        SidecarMethod::TrainJob,
        SidecarMethod::PromoteModel,
    ]
}

// =================================================================
// v0.33b — list_promote_history_archive (read dropped entries)
// =================================================================

/// v0.33b — args for the `list_promote_history_archive` IPC.
/// Mirrors the Python sidecar's archive.jsonl format. All
/// fields are optional; the L1 can paginate with `offset`
/// + `limit`, or filter by `from_ms` / `to_ms`.
#[derive(Debug, Clone, Deserialize)]
pub struct ListPromoteHistoryArchiveArgs {
    /// Optional lower bound on `promoted_at_ms`. Default
    /// 0 (no lower bound).
    pub from_ms: Option<i64>,
    /// Optional upper bound on `promoted_at_ms`. Default
    /// i64::MAX (no upper bound).
    pub to_ms: Option<i64>,
    /// Pagination offset. Default 0.
    pub offset: Option<usize>,
    /// Pagination limit. Default 100 (capped at 1000).
    pub limit: Option<usize>,
    /// v0.42e-3 — optional whitelist of job_ids. When
    /// supplied, the result only includes entries whose
    /// `job_id` is in this set. Used by the
    /// `ModelComparison` component to fetch weights for
    /// the 2-3 selected entries without pulling the
    /// whole archive. Empty array = no entries; missing
    /// = no filter (return all).
    #[serde(default)]
    pub job_ids: Option<Vec<String>>,
}

/// v0.33b — wire-format mirror of the Python sidecar's
/// archive.jsonl. Each entry is one line in the JSONL file
/// (one archived promotion). Fields mirror
/// `PromoteHistoryEntry` plus `archived_at_ms` (when the
/// entry was written to the archive).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromoteHistoryArchiveEntry {
    pub job_id: String,
    pub model_version: String,
    pub promoted_at_ms: i64,
    pub best_brier: Option<f64>,
    pub best_params: Option<serde_json::Value>,
    pub weights: Option<serde_json::Value>,
    pub trial_index: Option<usize>,
    /// v0.33b — when this entry was written to the archive
    /// file. May differ from `promoted_at_ms` if the sidecar
    /// was offline and the entry was written later.
    pub archived_at_ms: i64,
}

/// v0.33b — response of `list_promote_history_archive`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromoteHistoryArchiveResult {
    /// `true` if the archive file exists and was readable.
    pub ok: bool,
    /// All entries matching the filter (after pagination).
    pub entries: Vec<PromoteHistoryArchiveEntry>,
    /// Total entries in the file (before pagination).
    pub total: usize,
    /// Optional message (error or "no archive yet").
    pub message: Option<String>,
}

/// v0.33b — read the Python sidecar's `archive.jsonl` file
/// and return paginated entries. The file lives at
/// `~/.polyrocket/sidecar/models/archive.jsonl` (overridable
/// via `POLYROCKET_SIDECAR_MODEL_DIR`).
///
/// The file is JSONL: one JSON object per line. We parse
/// each line, filter by `from_ms`/`to_ms`, and return up to
/// `limit` entries starting at `offset`. Results are
/// sorted by `promoted_at_ms` descending (newest first).
///
/// The 20-entry cap on `promotion_history[]` is the primary
/// in-memory audit trail. The archive file is the durable
/// long-term trail. The L1 can show "View archive" on the
/// ModelLab page to see entries that fell off the cap.
#[tauri::command]
pub async fn list_promote_history_archive(
    args: ListPromoteHistoryArchiveArgs,
) -> AppResult<PromoteHistoryArchiveResult> {
    use std::io::{BufRead, BufReader};
    // v0.33b — derive the archive path from the same env
    // var the Python sidecar uses. If unset, default to
    // ~/.polyrocket/sidecar/models/archive.jsonl.
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
    // v0.42e-3 — build a HashSet for O(1) lookup if
    // the caller passed a job_ids whitelist. None =
    // no filter (return all matching time range).
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

    // Parse all lines, filter, sort newest-first, paginate.
    // For a long-term archive (thousands of entries) this
    // could be slow; for the expected use case (a few
    // hundred entries per year) it's fine.
    let mut all: Vec<PromoteHistoryArchiveEntry> = Vec::new();
    let reader = BufReader::new(file);
    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue, // skip malformed lines silently
        };
        if line.trim().is_empty() {
            continue;
        }
        let entry: PromoteHistoryArchiveEntry = match serde_json::from_str(&line) {
            Ok(e) => e,
            Err(_) => continue, // skip malformed lines silently
        };
        if entry.promoted_at_ms < from_ms || entry.promoted_at_ms > to_ms {
            continue;
        }
        // v0.42e-3 — apply the job_ids whitelist if set.
        // Empty whitelist returns no entries; missing =
        // no filter.
        if let Some(set) = &job_ids_filter {
            if !set.contains(&entry.job_id) {
                continue;
            }
        }
        all.push(entry);
    }
    // Newest first
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
// v0.28a — auto-promote config (in-memory, set via L1 IPC)
// =================================================================

/// v0.28a — args for `set_auto_promote_config`. Both fields
/// are optional: `None` means "leave unchanged" so the L1
/// can update only the field the user changed in the UI
/// (e.g. just the toggle, not the margin).
#[derive(Debug, Clone, Deserialize)]
pub struct SetAutoPromoteConfigArgs {
    pub enabled: Option<bool>,
    pub brier_margin: Option<f64>,
}

/// v0.28a — current auto-promote config. Returned by
/// `get_auto_promote_config` for the L1 to display
/// "what the Rust side currently has" (in case the L1
/// store was reset, e.g. by a hard refresh).
#[derive(Debug, Clone, Serialize)]
pub struct AutoPromoteConfigDto {
    pub enabled: bool,
    pub brier_margin: f64,
}

/// v0.28a — L1 pushes the user's auto-promote settings
/// into `AppState` so the Rust `train_job` handler can
/// decide whether to spawn the auto-promote worker.
///
/// Both fields are optional; only the supplied ones are
/// updated. Returns the new merged config so the L1
/// can confirm what Rust now has.
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
        // Clamp to a sane range: 0.0 (any improvement) to 0.1
        // (only promote if 10% better). Negative would be
        // "promote if not worse", which is silly.
        guard.brier_margin = m.clamp(0.0, 0.1);
    }
    Ok(AutoPromoteConfigDto {
        enabled: guard.enabled,
        brier_margin: guard.brier_margin,
    })
}

// =================================================================
// v0.42c — telemetry enabled toggle (L1 IPC)
// =================================================================

/// v0.42c — runtime override of telemetry enable/disable.
/// L1 calls this from the Settings toggle. The default
/// (false) is what `init_from_env` leaves it as unless
/// `POLYROCKET_TELEMETRY=1` was set in the env at
/// startup. After this call, the user is in full
/// control — the env var no longer matters for this
/// process.
#[tauri::command]
pub async fn set_telemetry_enabled(args: SetTelemetryEnabledArgs) -> AppResult<bool> {
    crate::infra::telemetry::set_enabled(args.enabled);
    Ok(args.enabled)
}

/// v0.42c — read the current telemetry state. The L1
/// calls this on Settings mount so the toggle
/// reflects "what the Rust side currently has"
/// (in case the env var set it at startup).
#[tauri::command]
pub async fn get_telemetry_enabled() -> AppResult<bool> {
    Ok(crate::infra::telemetry::is_enabled())
}

/// v0.42c — args for `set_telemetry_enabled`. We use a
/// struct (not a bare bool) for future-proofing: a
/// future `sink: Option<String>` could let the L1
/// pick stderr vs file vs no-op without an IPC
/// redesign.
#[derive(Debug, Clone, Deserialize)]
pub struct SetTelemetryEnabledArgs {
    pub enabled: bool,
}

/// v0.28a — read the current auto-promote config from
/// `AppState`. Returns defaults if the L1 has never
/// pushed any config.
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

/// v0.28a — event payload for the background auto-promote
/// worker. Emitted on the Tauri bus as `auto_promote:finished`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoPromoteFinishedEvent {
    /// The job_id from the train that triggered the auto-promote.
    pub job_id: String,
    /// Whether the auto-promote actually promoted.
    pub promoted: bool,
    /// Reason / status string from the sidecar.
    pub message: String,
    /// If promoted, the new model version.
    pub model_version: Option<String>,
    /// When the auto-promote finished (unix millis).
    pub finished_at: i64,
}

/// v0.28a — spawn a background task that calls
/// `auto_promote_if_better` on the sidecar, then emits
/// `auto_promote:finished`.
///
/// This is a private helper used by `train_job` after
/// the sidecar returns a successful train. It is NOT a
/// `#[tauri::command]` — it runs in a `tokio::spawn`'d
/// task so the train IPC returns immediately.
///
/// The worker:
/// 1. Calls `auto_promote_if_better(brier_margin)` via
///    the same stdin/stdout protocol as the user-facing
///    command
/// 2. Emits the result on `auto_promote:finished`
/// 3. Silently swallows errors (they are reflected in
///    `message`; we don't want a failed auto-promote
///    to crash the train IPC that already returned)
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

    // If the sidecar isn't running, the worker just
    // emits a "no-op" finished event so the L1 can
    // update its UI (e.g. "auto-promote skipped: sidecar down").
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
    // v0.42b — emit lifecycle event. The OS notification
    // path is unchanged (v0.39a only fires on
    // `promoted: true`); telemetry captures BOTH
    // outcomes for analysis. Future v0.42e may add a
    // skipped-notification toggle that piggybacks on
    // the AutoPromoteSkipped event.
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
    // v0.33b — list_promote_history_archive
    // ============================================================

    use std::io::Write;

    /// Helper: write a JSONL archive file with N entries.
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
    async fn archive_returns_empty_when_no_file() {
        // v0.33b — if archive.jsonl doesn't exist, return
        // ok=true with 0 entries and a friendly message
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
    async fn archive_reads_and_paginates_entries() {
        // v0.33b — write 25 entries, read with default
        // limit=100, verify all 25 returned, sorted newest-first
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
        // Newest first: entry[0] is the 25th written (i=24)
        // The job_id is `train-00000018` (24 in hex, 0-padded to 8)
        assert_eq!(r.entries[0].job_id, "train-00000018");
        // The last entry is the oldest (i=0)
        assert_eq!(r.entries[24].job_id, "train-00000000");
        // Each entry has all the required fields
        assert!(r.entries[0].archived_at_ms > 0);
        assert!(r.entries[0].weights.is_some());

        std::env::remove_var("POLYROCKET_SIDECAR_MODEL_DIR");
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[tokio::test]
    async fn archive_pagination_offset_and_limit() {
        // v0.33b — write 30 entries, read with offset=10
        // limit=5, verify 5 entries returned (indices 10..15
        // of the newest-first list, which is the 20th-16th
        // oldest entries)
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
    async fn archive_filters_by_time_range() {
        // v0.33b — write 10 entries at 1000ms intervals,
        // filter to 5 entries (i=3..7) by from_ms/to_ms
        let tmp = std::env::temp_dir().join(format!(
            "polyrocket_test_archive_time_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        write_test_archive(&tmp.join("archive.jsonl"), 10);
        std::env::set_var("POLYROCKET_SIDECAR_MODEL_DIR", &tmp);

        // Entry i=3 is at 1_700_000_003_000, i=7 is at 1_700_000_007_000
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
        // 5 entries match: i=3,4,5,6,7
        assert_eq!(r.total, 5);
        assert_eq!(r.entries.len(), 5);

        std::env::remove_var("POLYROCKET_SIDECAR_MODEL_DIR");
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[tokio::test]
    async fn archive_filters_by_job_ids() {
        // v0.42e-3 — whitelist filter on job_ids.
        // The ModelComparison modal uses this to
        // pull weights for the 2-3 selected
        // entries without fetching the whole
        // archive.
        let tmp = std::env::temp_dir().join(format!(
            "polyrocket_test_archive_jobids_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        write_test_archive(&tmp.join("archive.jsonl"), 10);
        std::env::set_var("POLYROCKET_SIDECAR_MODEL_DIR", &tmp);

        // Whitelist: train-00000002, train-00000005,
        // train-00000008 (the helper writes
        // job_id="train-{:08x}" so i=2 → 00000002).
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
        // total counts entries BEFORE pagination
        assert_eq!(r.total, 3);
        assert_eq!(r.entries.len(), 3);
        let ids: std::collections::HashSet<String> =
            r.entries.iter().map(|e| e.job_id.clone()).collect();
        assert!(ids.contains("train-00000002"));
        assert!(ids.contains("train-00000005"));
        assert!(ids.contains("train-00000008"));

        // Empty whitelist → 0 entries
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

        // Whitelist that matches nothing → 0 entries
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

        // Whitelist combined with from_ms — both
        // filters must apply
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
        // Only train-00000008 survives (i=5 is at
        // 1_700_000_005_000 which is below the
        // from_ms)
        assert_eq!(r4.total, 1);
        assert_eq!(r4.entries[0].job_id, "train-00000008");

        std::env::remove_var("POLYROCKET_SIDECAR_MODEL_DIR");
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
