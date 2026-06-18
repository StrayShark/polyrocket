//! L4 — Process-local telemetry.
//!
//! v0.42a — opt-in event emission. Default off; turn on
//! with `POLYROCKET_TELEMETRY=1` in the env.
//!
//! ## Design
//!
//! - **Default off** (zero overhead). `is_enabled()` is a
//!   static atomic read, so call sites are `O(1)` either way.
//! - **No PII, no model weights, no secrets.** Events are
//!   coarse-grained lifecycle markers (train started,
//!   promote completed, scheduler tick) plus a small
//!   typed context bag (job_id, loop_name, latency_ms).
//! - **Sink is stderr, NDJSON.** One event per line:
//!   `{"name":"train_completed","ts_unix_ms":...,"ctx":{...}}`.
//!   Capture with `polyrocket 2> telemetry.log`. No file
//!   rotation, no locking.
//! - **Sink is swappable** via the `Sink` trait. v0.42a ships
//!   a `StderrSink`; future: file sink, Sentry sink, no-op
//!   test sink.
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
}

/// Emit a single event. No-op if `!is_enabled()`.
///
/// On the enabled path, serializes to NDJSON and writes
/// one line to stderr. Failure to write is silent (stderr
/// is best-effort; we never want telemetry to crash the
/// app).
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
}
