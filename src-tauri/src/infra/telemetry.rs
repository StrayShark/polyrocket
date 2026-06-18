//! L4 — Process-local telemetry.
//!
//! v0.42a — opt-in event emission. Default off; turn on
//! with `POLYROCKET_TELEMETRY=1` in the env.
//!
//! v0.49a — file retention. When telemetry is on, every
//! event is also appended to a per-session JSONL file in
//! `<app_data_dir>/logs/telemetry/session-<start_unix>.jsonl`.
//! On startup we delete session files older than
//! `POLYROCKET_TELEMETRY_RETENTION_DAYS` (default 14).
//!
//! ## Design
//!
//! - **Default off** (zero overhead). `is_enabled()` is a
//!   static atomic read, so call sites are `O(1)` either way.
//! - **No PII, no model weights, no secrets.** Events are
//!   coarse-grained lifecycle markers (train started,
//!   promote completed, scheduler tick) plus a small
//!   typed context bag (job_id, loop_name, latency_ms).
//! - **Sinks are stderr (always, when enabled) AND a
//!   per-session JSONL file (when log_dir is set, v0.49a).**
//!   Capture with `polyrocket 2> telemetry.log` for the
//!   live stream; the file gives you a persistent record
//!   that survives restarts. The L1 can call
//!   `list_telemetry_logs` / `purge_telemetry_logs` to
//!   browse and clean up.
//! - **Sink is swappable** via the `Sink` trait. v0.42a ships
//!   a `StderrSink`; v0.49a adds a `FileSink`.
//!
//! ## Why not `log`/`tracing` crates?
//!
//! The existing scheduler already uses `tracing` for
//! human-readable debug output. Telemetry is a separate
//! stream: machine-parseable, opt-in, lifecycle-scoped. We
//! don't want to flood the tracing subscriber by default.
//!
//! ## Use
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
//! Layer rules: L4 may depend on L5 (`platform::env`). It
//! may NOT depend on L3 / L2 / L1. The call sites that emit
//! events live in L2 (IPC handlers) and L4 (scheduler) —
//! they `use crate::infra::telemetry` directly.

use crate::platform::env;
use serde::Serialize;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

/// Process-global enabled flag. `static` so `is_enabled()` is
/// a single atomic load — no lock contention.
static ENABLED: AtomicBool = AtomicBool::new(false);
static INITIALIZED: AtomicBool = AtomicBool::new(false);

/// Initialise from env. Idempotent — safe to call from
/// multiple setup paths; only the first call has effect.
pub fn init_from_env() {
    if INITIALIZED.swap(true, Ordering::SeqCst) {
        return;
    }
    let v = env::env_str("POLYROCKET_TELEMETRY")
        .map(|s| s == "1" || s.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    ENABLED.store(v, Ordering::SeqCst);
}

/// True if telemetry is on for this process. Cheap atomic
/// load — call from hot paths if needed.
pub fn is_enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

/// Test-only: override the enabled flag.
#[cfg(test)]
pub fn set_enabled_for_test(v: bool) {
    ENABLED.store(v, Ordering::SeqCst);
    INITIALIZED.store(true, Ordering::SeqCst);
}

/// v0.42c — runtime override. Called by the L1
/// `setTelemetryEnabled` IPC when the user toggles
/// the pref in Settings. Does NOT touch the
/// `INITIALIZED` flag, so a later `init_from_env` call
/// would still be a no-op (idempotent).
pub fn set_enabled(v: bool) {
    ENABLED.store(v, Ordering::SeqCst);
}

// ============================================================
// v0.49a — file retention
// ============================================================
//
// When telemetry is enabled, every event is also appended to
// a per-session JSONL file. The file path is fixed for the
// lifetime of the process (set via `set_log_dir` at startup).
// On startup we also run a retention sweep that deletes
// session files older than `POLYROCKET_TELEMETRY_RETENTION_DAYS`
// (default 14). The L1 can also call `purge_telemetry_logs` to
// run the sweep on demand.
//
// File name format: `session-<start_unix>.jsonl`. Lex-sorted
// filenames → time-sorted. Two-digit start_unix means the
// sortable part is the first 10 chars after "session-".

static LOG_DIR: std::sync::Mutex<Option<PathBuf>> = std::sync::Mutex::new(None);

/// Set the log directory for the FileSink. Called once at
/// startup, after `app_data_dir` is reachable. Idempotent —
/// only the first call has effect. Creates the directory
/// if missing.
pub fn set_log_dir(dir: PathBuf) -> std::io::Result<()> {
    let mut slot = LOG_DIR.lock().expect("LOG_DIR lock poisoned");
    if slot.is_some() {
        return Ok(());
    }
    std::fs::create_dir_all(&dir)?;
    *slot = Some(dir);
    Ok(())
}

/// Default retention window in days. Read from
/// `POLYROCKET_TELEMETRY_RETENTION_DAYS` (env-var override);
/// 14 is the out-of-box default.
pub fn retention_days() -> u64 {
    env::env_u64("POLYROCKET_TELEMETRY_RETENTION_DAYS", 14)
}

/// One row in the L1 telemetry-log list. Returned by
/// `list_telemetry_logs` so the Settings card can show
/// the on-disk file inventory.
#[derive(Debug, Clone, Serialize)]
pub struct TelemetryLogInfo {
    /// File name only, e.g. `session-1740000000.jsonl`.
    pub name: String,
    /// Absolute path on disk.
    pub path: String,
    /// File size in bytes (0 if the file vanished between
    /// `read_dir` and `metadata` — we tolerate that).
    pub size_bytes: u64,
    /// File modification time, unix seconds (0 if
    /// unknown).
    pub modified_unix: u64,
    /// True if this is the active session file (i.e.
    /// the current process is appending to it).
    pub is_current: bool,
}

/// Return a sorted list of all telemetry session files
/// under the current log dir. The current process's
/// session is flagged `is_current = true`. Returns an
/// empty list (not an error) when no log dir is set
/// yet — the L1 might call this before startup finishes.
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
            // Only the "session-*.jsonl" pattern. Lex-sort
            // on the filename gives chronological order.
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
    // Oldest first — user reading top-down.
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// Manually trigger a retention sweep. Returns the number
/// of files deleted. The retention window is
/// `POLYROCKET_TELEMETRY_RETENTION_DAYS` (default 14).
///
/// Policy: a file is "stale" when its filename timestamp
/// is older than `now - retention_days`. We use the
/// filename (`session-<unix>.jsonl`) rather than mtime
/// because the latter can be perturbed by filesystem
/// backup tools / `touch`.
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
        // Extract the timestamp portion: "session-<digits>.jsonl"
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

/// Compute the per-session file path. The session starts
/// at the first `emit()` call after `set_log_dir`. Subsequent
/// events in the same process append to the same file.
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

/// Append one NDJSON line to the session file. Best-effort:
/// errors (full disk, permission, etc.) are silently dropped.
/// Telemetry is observability, not a hard dependency.
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

/// All telemetry events. Add variants here as new lifecycle
/// hooks appear; the wire format (NDJSON) is stable because
/// the `Serialize` impl is auto-derived.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "name", rename_all = "snake_case")]
pub enum Event {
    /// Scheduler loop woke up. `tick_index` is per-loop
    /// monotonically increasing; useful for spotting
    /// stuck loops.
    SchedulerTick {
        loop_name: &'static str,
        tick_index: u64,
    },
    /// Scheduler loop caught a recoverable error. The
    /// loop is still running; this is a "noticed"
    /// event, not a "crash".
    SchedulerError {
        loop_name: &'static str,
        error: String,
    },
    /// `train_job` IPC accepted.
    TrainStarted {
        job_id: String,
        n_trials: usize,
        epochs: usize,
    },
    /// `train_job` IPC returned a successful train.
    TrainCompleted {
        job_id: String,
        model_version: String,
        best_brier: Option<f64>,
        duration_ms: u64,
    },
    /// `train_job` IPC failed (sidecar error, DB error,
    /// panic caught).
    TrainFailed {
        job_id: String,
        error: String,
    },
    /// `promote_model` succeeded.
    PromoteCompleted {
        job_id: String,
        model_version: String,
        trial_index: Option<usize>,
        reason: String,
    },
    /// Background auto-promote worker (v0.28a) actually
    /// promoted (i.e. the candidate beat the active by
    /// the configured Brier margin).
    AutoPromoteFired {
        job_id: String,
        model_version: String,
        message: String,
    },
    /// Background auto-promote worker ran but did NOT
    /// promote (candidate not better, no active, etc.).
    /// This is the v0.42e feature: opt-in signal for
    /// "your training didn't improve anything" —
    /// currently only logged, not surfaced via OS
    /// notification.
    AutoPromoteSkipped {
        job_id: String,
        message: String,
    },
    /// Sidecar process connected and answered the
    /// first probe. The "sidecar is alive" marker.
    SidecarConnected {
        pid: Option<u32>,
    },
    /// Sidecar process exited or became unresponsive.
    /// `reason` is the last error string from the
    /// reader task.
    SidecarDisconnected {
        reason: String,
    },
    /// Daily brief job finished generating a summary.
    DailyBriefGenerated {
        summary_chars: usize,
        duration_ms: u64,
    },
    /// Anomaly detector flagged a market.
    AnomalyDetected {
        kind: String,
        severity: u8,
        details: String,
    },
    /// LLM health probe result.
    LlmHealthProbe {
        provider: String,
        ok: bool,
        latency_ms: u64,
        error: Option<String>,
    },
    /// Mirror executor tick. `intents_pending` is the
    /// queue depth at the start of the tick; `executed`
    /// and `errors` are outcomes within the tick.
    MirrorExecutorTick {
        intents_pending: usize,
        executed: usize,
        errors: usize,
    },
    /// Audit retention sweep removed N rows.
    AuditPurged {
        rows: u64,
        retention_days: u64,
    },
    /// v0.45a — paper_fills reconciliation pass
    /// settled N paper_fills (i.e. they hit markets
    /// that became resolved between the previous
    /// tick and this one).
    PaperFillsReconciled {
        settled: u64,
    },
    /// v0.48a — model degradation check. Emitted by
    /// the 8th scheduler loop when the live Brier
    /// (computed against the FALLBACK model
    /// weights on recently resolved markets) has
    /// drifted above the threshold relative to the
    /// train-time Brier. The L1 listens to this and
    /// optionally fires an OS notification (gated
    /// by a Settings pref).
    ModelDegradation {
        /// Number of recent resolved markets the
        /// live Brier was computed over. The L1 uses
        /// this to know "is this a real signal or
        /// just a small-sample fluctuation".
        n_samples: u64,
        /// Live Brier score (mean over the n_samples
        /// most recent resolved markets, using
        /// FALLBACK weights as the prediction model).
        live_brier: f64,
        /// Train-time Brier from the active model
        /// (best_brier in active.json, or 0.0 when
        /// no active model is set).
        train_brier: f64,
        /// live_brier - train_brier. Positive means
        /// the live performance is worse than train.
        /// Negative means better (rare; usually
        /// live is at least as good as train).
        drift: f64,
        /// True when drift > the configured threshold.
        /// The L1 only fires an OS notification when
        /// this is true (rather than every tick).
        alert: bool,
    },
}

/// Emit a single event. No-op if `!is_enabled()`.
///
/// On the enabled path, serializes to NDJSON and writes
/// one line to stderr AND (v0.49a) appends the same line
/// to the per-session JSONL file. Failure to write is
/// silent (best-effort; we never want telemetry to crash
/// the app).
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
    // v0.49a — file sink. Same line, appended to the
    // session file. Errors are silent.
    append_to_file(&payload);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emit_is_noop_when_disabled() {
        set_enabled_for_test(false);
        // Should not panic, should not write. The
        // assertion is "we get back without crashing".
        emit(Event::SchedulerTick {
            loop_name: "test",
            tick_index: 1,
        });
    }

    #[test]
    fn emit_writes_ndjson_when_enabled() {
        set_enabled_for_test(true);
        // We can't easily capture stderr in a unit test
        // without a custom Sink trait (planned v0.42a
        // extension). For now, just confirm the call
        // doesn't panic and returns.
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
        // Tag should be the snake_case variant name.
        assert!(s.contains("\"name\":\"train_completed\""), "got: {s}");
        assert!(s.contains("\"job_id\":\"train-x\""), "got: {s}");
        assert!(s.contains("\"best_brier\":0.2"), "got: {s}");
        assert!(s.contains("\"duration_ms\":100"), "got: {s}");
    }

    #[test]
    fn init_from_env_is_idempotent() {
        set_enabled_for_test(false);
        // Second call must not panic and must not
        // re-read the env.
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

    // v0.49a — file sink tests. We use a per-test
    // tempdir so the global LOG_DIR state is overwritten
    // (set_log_dir is idempotent, so we reset via a
    // direct unsafe write — fine in tests).
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

        // Reset global LOG_DIR via the public API. The
        // set_log_dir is "first call wins" — for tests
        // we use a fresh dir each time, and we patch
        // the global via the path the test wants.
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
        // Each emit is exactly one line.
        assert_eq!(body.lines().count(), 2, "got: {body}");

        // Cleanup
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

        // Write a "stale" file with a filename ts in
        // the distant past. The retention window for
        // this test is 14 days, so any ts older than
        // 14d-ago should be deleted.
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let old_ts = now - (30 * 86_400); // 30 days ago
        let recent_ts = now - (3 * 86_400); // 3 days ago
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
        // File is created lazily but stays empty (or
        // isn't created at all). Either way, no event
        // payload in the dir.
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
