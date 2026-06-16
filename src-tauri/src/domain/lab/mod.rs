//! L3 — Lab (model experiments / backtest).
//!
//! Hosts the offline model lab — a separate Python sidecar invoked via
//! `tauri-plugin-shell` to score signal accuracy on historical data.
//! Stores experiment configs and results in `lab_runs`.
//!
//! **Status (v0.3c): stub.** M5.2 "Lab / backtest" milestone.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LabRun {
    pub id: String,
    pub model_version: String,
    pub started_at: i64,
    pub finished_at: Option<i64>,
    pub params_json: String,
    pub metrics_json: Option<String>,
    pub status: String, // 'queued' | 'running' | 'done' | 'error'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lab_run_default_status_is_queued() {
        let r = LabRun {
            id: "r1".into(),
            model_version: "v0.0.1".into(),
            started_at: 0,
            finished_at: None,
            params_json: "{}".into(),
            metrics_json: None,
            status: "queued".into(),
        };
        assert_eq!(r.status, "queued");
        assert!(r.finished_at.is_none());
    }
}
