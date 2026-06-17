//! L2 — Python sidecar process manager (M7).
//!
//! Spawns the optional `polyrocket-sidecar` Python process, sends
//! JSON-line requests, and tracks health. All protocol details are
//! in `domain::lab::sidecar`; this module handles the OS process.

use crate::AppError;
use crate::AppResult;
use crate::domain::lab::sidecar::{
    build_predict_request, parse_line, parse_predict_response, Prediction, SidecarMethod,
    SidecarRequest, SidecarResponse,
};
use crate::infra::db;
use serde::{Deserialize, Serialize};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::Mutex;
use tauri::State;

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
    Ok(preds)
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
