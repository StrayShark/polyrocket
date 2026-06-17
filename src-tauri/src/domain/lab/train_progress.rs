//! L3 — Progress event payloads for the `train_job` IPC.
//!
//! v0.17a — `commands::sidecar::train_job` emits these events
//! via `AppHandle::emit` so the L1 ModelLab page can show
//! a training-in-progress indicator + the final result.
//!
//! Event names (all on the global Tauri event bus):
//!   - `train_job:started`   — IPC dispatched, training begins
//!   - `train_job:finished`  — training completed or failed
//!
//! Unlike `llm_analyze` (which had N parallel providers and
//! emitted per-provider events), `train_job` is a sequential
//! 4-trial sweep inside a single Python call. We don't get
//! per-trial events from the Python sidecar because the
//! stdio protocol is one-request-one-response. The L1 shows
//! a generic "Training…" pill during the run, then displays
//! the full result (per-trial stats + best Brier + params)
//! when the finished event arrives.
//!
//! The `job_id` is the UUID we pass to the Python sidecar;
//! it's used to correlate the started/finished pair.

use serde::{Deserialize, Serialize};

/// Payload for `train_job:started`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainStartedEvent {
    pub job_id: String,
    /// Total number of trials the Python sidecar will run
    /// (1-4). The L1 uses this to render a "N trials" hint
    /// in the progress pill.
    pub n_trials: u32,
    /// Per-trial training epochs. v0.17a default is 80.
    pub epochs: u32,
    /// Wall-clock start time in milliseconds.
    pub started_at: i64,
}

/// Payload for `train_job:finished`.
///
/// `status` is one of:
///   - `"completed"` — all trials finished, best model persisted
///   - `"failed"`    — sweep or persistence errored; see `message`
///
/// `best_brier` is `None` on failure. `trials` is empty on
/// failure (we couldn't record any trial stats before the
/// error).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainFinishedEvent {
    pub job_id: String,
    pub status: String,
    /// Lower is better. `None` on failure.
    pub best_brier: Option<f64>,
    /// Best trial's weights as `{"w0", "w1", "w2"}`. `None` on failure.
    pub best_params: Option<serde_json::Value>,
    /// Per-trial stats (empty on failure).
    pub trials: Vec<TrainTrialDto>,
    pub duration_ms: i64,
    /// Absolute path of the candidate JSON the sidecar wrote.
    /// `None` on failure.
    pub candidate_path: Option<String>,
    /// Human-readable error message. `None` on success.
    pub message: Option<String>,
    pub finished_at: i64,
}

/// One trial's stats in the finished event. Mirrors
/// `domain::lab::sidecar::TrainTrial` but lives here
/// to keep the progress module self-contained.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainTrialDto {
    pub lr: f64,
    pub reg: f64,
    pub brier: f64,
    pub weights: serde_json::Value,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn started_event_round_trip() {
        let e = TrainStartedEvent {
            job_id: "train-441c352b".into(),
            n_trials: 4,
            epochs: 80,
            started_at: 1_000,
        };
        let s = serde_json::to_string(&e).unwrap();
        let d: TrainStartedEvent = serde_json::from_str(&s).unwrap();
        assert_eq!(d.job_id, "train-441c352b");
        assert_eq!(d.n_trials, 4);
    }

    #[test]
    fn finished_event_completed() {
        let e = TrainFinishedEvent {
            job_id: "train-441c352b".into(),
            status: "completed".into(),
            best_brier: Some(0.184),
            best_params: Some(serde_json::json!({"w0": 0.1, "w1": 0.2, "w2": 0.3})),
            trials: vec![TrainTrialDto {
                lr: 0.05,
                reg: 0.01,
                brier: 0.184,
                weights: serde_json::json!({"w0": 0.1, "w1": 0.2, "w2": 0.3}),
            }],
            duration_ms: 4200,
            candidate_path: Some("/home/x/.polyrocket/sidecar/models/candidate.json".into()),
            message: None,
            finished_at: 5_200,
        };
        let s = serde_json::to_string(&e).unwrap();
        let d: TrainFinishedEvent = serde_json::from_str(&s).unwrap();
        assert_eq!(d.status, "completed");
        assert!(d.best_brier.is_some());
        assert!(d.best_brier.unwrap() < 0.2);
    }

    #[test]
    fn finished_event_failed() {
        let e = TrainFinishedEvent {
            job_id: "train-deadbeef".into(),
            status: "failed".into(),
            best_brier: None,
            best_params: None,
            trials: vec![],
            duration_ms: 500,
            candidate_path: None,
            message: Some("OSError: disk full".into()),
            finished_at: 1_000,
        };
        let s = serde_json::to_string(&e).unwrap();
        let d: TrainFinishedEvent = serde_json::from_str(&s).unwrap();
        assert_eq!(d.status, "failed");
        assert!(d.best_brier.is_none());
        assert!(d.trials.is_empty());
        assert!(d.message.unwrap().contains("disk full"));
    }
}
