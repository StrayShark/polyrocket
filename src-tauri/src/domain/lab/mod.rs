//! L3 — Lab (model experiments / backtest).
//!
//! Hosts the offline model lab — a separate Python sidecar invoked via
//! `tauri-plugin-shell` to score signal accuracy on historical data.
//! Stores experiment configs and results in `lab_runs`.
//!
//! **Status (v0.3c): stub.** M5.2 "Lab / backtest" milestone.

use serde::{Deserialize, Serialize};

pub mod sidecar;

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

// ============================================================
// ============== Run state machine ===========================
// ============================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RunStatus {
    Queued,
    Running,
    Done,
    Error,
}

impl RunStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            RunStatus::Queued => "queued",
            RunStatus::Running => "running",
            RunStatus::Done => "done",
            RunStatus::Error => "error",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "queued" => Some(RunStatus::Queued),
            "running" => Some(RunStatus::Running),
            "done" => Some(RunStatus::Done),
            "error" => Some(RunStatus::Error),
            _ => None,
        }
    }
    /// Legal transitions: Queued→Running, Running→Done|Error.
    /// Done and Error are terminal.
    pub fn can_transition_to(self, next: RunStatus) -> bool {
        match (self, next) {
            (RunStatus::Queued, RunStatus::Running) => true,
            (RunStatus::Running, RunStatus::Done) => true,
            (RunStatus::Running, RunStatus::Error) => true,
            _ => false,
        }
    }
}

// ============================================================
// ============== Model version naming =======================
// ============================================================

/// Validate a semantic-version-ish model name: vMAJOR.MINOR.PATCH[-tag]
/// Returns Err if not parseable.
pub fn validate_version(v: &str) -> Result<(), String> {
    if !v.starts_with('v') {
        return Err(format!("version must start with 'v': {v}"));
    }
    let rest = &v[1..];
    let parts: Vec<&str> = rest.split('.').collect();
    if parts.len() < 2 || parts.len() > 3 {
        return Err(format!("version must be vMAJOR.MINOR[.PATCH]: {v}"));
    }
    for p in &parts[..parts.len() - 1] {
        if p.parse::<u32>().is_err() {
            return Err(format!("non-numeric version component: {p:?}"));
        }
    }
    let last = parts[parts.len() - 1];
    // last may be a number OR "NUMBER-tag" (e.g. "1-beta")
    let (num, tag) = match last.find('-') {
        Some(i) => (&last[..i], Some(&last[i + 1..])),
        None => (last, None),
    };
    if num.parse::<u32>().is_err() {
        return Err(format!("non-numeric PATCH: {last:?}"));
    }
    if let Some(t) = tag {
        if t.is_empty() {
            return Err("empty tag after dash".into());
        }
    }
    Ok(())
}

/// Sort versions: v0.1.0 < v0.1.1 < v0.2.0 < v1.0.0
/// Returns true if `a` is older than `b`.
pub fn is_older(a: &str, b: &str) -> bool {
    let av = parse_version_tuple(a);
    let bv = parse_version_tuple(b);
    av < bv
}

fn parse_version_tuple(v: &str) -> (u32, u32, u32) {
    let v = v.trim_start_matches('v');
    let (num, _tag) = match v.find('-') {
        Some(i) => (&v[..i], Some(&v[i + 1..])),
        None => (v, None),
    };
    let parts: Vec<u32> = num.split('.').filter_map(|p| p.parse().ok()).collect();
    let maj = parts.first().copied().unwrap_or(0);
    let min = parts.get(1).copied().unwrap_or(0);
    let pat = parts.get(2).copied().unwrap_or(0);
    (maj, min, pat)
}

/// Compare two model performance snapshots; the better one wins.
/// Comparison order: lower brier → higher win rate → higher n.
pub fn is_better(candidate: &ModelPerf, incumbent: &ModelPerf) -> bool {
    if (candidate.brier_score - incumbent.brier_score).abs() > 1e-9 {
        return candidate.brier_score < incumbent.brier_score;
    }
    if (candidate.win_rate - incumbent.win_rate).abs() > 1e-9 {
        return candidate.win_rate > incumbent.win_rate;
    }
    candidate.n_predictions > incumbent.n_predictions
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPerf {
    pub model_version: String,
    pub n_predictions: usize,
    pub win_rate: f64,
    pub brier_score: f64,
    pub log_loss: f64,
    pub avg_edge: f64,
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

    #[test]
    fn run_status_round_trip() {
        for s in [RunStatus::Queued, RunStatus::Running, RunStatus::Done, RunStatus::Error] {
            assert_eq!(RunStatus::parse(s.as_str()), Some(s));
        }
        assert_eq!(RunStatus::parse("garbage"), None);
    }

    #[test]
    fn run_status_legal_transitions() {
        assert!(RunStatus::Queued.can_transition_to(RunStatus::Running));
        assert!(RunStatus::Running.can_transition_to(RunStatus::Done));
        assert!(RunStatus::Running.can_transition_to(RunStatus::Error));
        // illegal
        assert!(!RunStatus::Done.can_transition_to(RunStatus::Running));
        assert!(!RunStatus::Error.can_transition_to(RunStatus::Done));
        assert!(!RunStatus::Queued.can_transition_to(RunStatus::Done));
    }

    #[test]
    fn validate_version_basic() {
        assert!(validate_version("v0.1.0").is_ok());
        assert!(validate_version("v1.2").is_ok());
        assert!(validate_version("v0.1.0-beta").is_ok());
        assert!(validate_version("v10.20.30-rc1").is_ok());
    }

    #[test]
    fn validate_version_rejects() {
        assert!(validate_version("0.1.0").is_err());     // no v
        assert!(validate_version("v1").is_err());         // too few parts
        assert!(validate_version("vX.Y.Z").is_err());     // non-numeric
        assert!(validate_version("v1.0.0-").is_err());    // empty tag
    }

    #[test]
    fn is_older_correct_order() {
        assert!(is_older("v0.1.0", "v0.1.1"));
        assert!(is_older("v0.1.0", "v0.2.0"));
        assert!(is_older("v0.9.9", "v1.0.0"));
        assert!(!is_older("v1.0.0", "v0.9.9"));
    }

    #[test]
    fn is_better_lower_brier_wins() {
        let a = ModelPerf { model_version: "v0.1".into(), n_predictions: 100, win_rate: 0.6, brier_score: 0.10, log_loss: 0.30, avg_edge: 0.05 };
        let b = ModelPerf { model_version: "v0.2".into(), n_predictions: 100, win_rate: 0.5, brier_score: 0.20, log_loss: 0.40, avg_edge: 0.04 };
        assert!(is_better(&a, &b));
        assert!(!is_better(&b, &a));
    }

    #[test]
    fn is_better_higher_win_rate_tiebreak() {
        let a = ModelPerf { model_version: "v0.1".into(), n_predictions: 100, win_rate: 0.7, brier_score: 0.20, log_loss: 0.40, avg_edge: 0.04 };
        let b = ModelPerf { model_version: "v0.2".into(), n_predictions: 100, win_rate: 0.6, brier_score: 0.20, log_loss: 0.40, avg_edge: 0.04 };
        assert!(is_better(&a, &b));
    }
}
