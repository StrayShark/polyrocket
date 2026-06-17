//! L2 — Python sidecar process manager (M7).
//!
//! Spawns the optional `polyrocket-sidecar` Python process, sends
//! JSON-line requests, and tracks health. All protocol details are
//! in `domain::lab::sidecar`; this module handles the OS process.

use crate::AppError;
use crate::AppResult;
use crate::domain::lab::sidecar::{
    build_predict_request, build_promote_request, build_train_request, parse_line,
    parse_predict_response, parse_promote_response, parse_train_response, Prediction,
    PromoteResult, SidecarMethod, SidecarRequest, SidecarResponse, TrainResult, TrainTrial,
};
use crate::domain::lab::train_progress::{
    TrainFinishedEvent, TrainStartedEvent, TrainTrialDto,
};
use crate::infra::db;
use serde::{Deserialize, Serialize};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::Mutex;
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
pub struct SidecarState {
    pub child: Mutex<Option<Child>>,
    pub stdin: Mutex<Option<ChildStdin>>,
    pub stdout: Mutex<Option<ChildStdout>>,
    pub status: Mutex<SidecarStatus>,
}

impl Default for SidecarState {
    fn default() -> Self {
        Self {
            child: Mutex::new(None),
            stdin: Mutex::new(None),
            stdout: Mutex::new(None),
            status: Mutex::new(SidecarStatus::default()),
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
            child: Mutex::new(None),  // ping_async doesn't need the child
            stdin: Mutex::new(None),
            stdout: Mutex::new(None),
            status: Mutex::new(self.status.lock().map_err(|e| format!("status lock: {e}"))?.clone()),
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
            child: Mutex::new(None),
            stdin: Mutex::new(None),
            stdout: Mutex::new(None),
            status: Mutex::new(self.status.lock().map_err(|e| format!("status lock: {e}"))?.clone()),
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

    if !state.is_running() {
        // Sidecar not running — emit a finished event with
        // status="failed" and a descriptive message, then
        // return the same shape so the L1 doesn't have to
        // handle a special "no sidecar" path.
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
        AppError::Internal(format!("train_job decode: {e}"))
    })?;

    // v0.17a — emit finished with the parsed result.
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
    let line = build_promote_request(&job_id, args.job_id.as_deref());

    if !state.is_running() {
        return Ok(PromoteResult {
            promoted: false,
            status: "failed".into(),
            previous_path: None,
            active_path: None,
            promoted_at_ms: None,
            model_version: String::new(),
            message: Some("sidecar not running".into()),
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
}

/// Args for the `promote_model` IPC. v0.18a — mirrors the
/// Python sidecar's optional `job_id` param.
#[derive(Debug, Clone, Deserialize)]
pub struct PromoteModelArgs {
    /// If set, refuses to promote a candidate from a
    /// different job. Defaults to `None` (accept any
    /// current candidate).
    pub job_id: Option<String>,
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
}
