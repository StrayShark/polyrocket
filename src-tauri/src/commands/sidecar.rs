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
