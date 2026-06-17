//! L3 — Progress event payloads for `llm_analyze`.
//!
//! v0.15a — `commands::llm::llm_analyze` emits these events via
//! `AppHandle::emit` so the L1 Analysis page can show per-provider
//! status as each LLM call completes (rather than blocking on
//! the entire fan-out).
//!
//! Event names (all on the global Tauri event bus, no namespace):
//!   - `llm_analyze:started`         — analysis row created, fan-out begins
//!   - `llm_analyze:provider_done`   — one provider call finished (ok or failed)
//!   - `llm_analyze:consensus_done`  — consensus computed (after last provider)
//!   - `llm_analyze:finished`        — full analyze completed (success / partial / failed)
//!
//! The `analysis_id` is the UUID string from `llm_analyses.id`,
//! so the L1 can correlate by that field (multiple analyzes can
//! be in-flight from different UI panels in the future).

use serde::{Deserialize, Serialize};

/// Payload for `llm_analyze:started`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyzeStartedEvent {
    pub analysis_id: String,
    pub market_id: String,
    pub prompt_version: String,
    pub providers: Vec<String>,
    pub started_at: i64,
}

/// Payload for `llm_analyze:provider_done`.
///
/// Emitted once per provider, whether the call succeeded or failed.
/// `error_kind` is one of: `none`, `auth`, `rate_limit`, `timeout`,
/// `network`, `parse`, `model_not_found`, `internal` (mirrors the
/// `CallErrorKind` enum in `domain::llm::dispatch`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderDoneEvent {
    pub analysis_id: String,
    pub provider_id: String,
    pub ok: bool,
    pub latency_ms: u64,
    pub tokens_in: Option<u32>,
    pub tokens_out: Option<u32>,
    pub cost_cents: f64,
    pub error_kind: String,
    pub error_message: Option<String>,
    pub finished_at: i64,
}

/// Payload for `llm_analyze:consensus_done`.
///
/// Emitted once per analyze, after all providers have finished and
/// the consensus is computed. `status` is one of: `completed`,
/// `partial`, `failed` (matches the existing `llm_analyses.status`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusDoneEvent {
    pub analysis_id: String,
    pub status: String,
    pub n_success: usize,
    pub n_failed: usize,
    pub consensus_pred: Option<f64>,
    pub consensus_side: Option<String>,
    pub consensus_conf: Option<f64>,
}

/// Payload for `llm_analyze:finished`.
///
/// Emitted at the very end (after consensus_done). This is the
/// event the L1 typically `await`s before mutating the final state.
/// Carries totals so the L1 can show "analyze took 4.2s, cost $0.12".
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyzeFinishedEvent {
    pub analysis_id: String,
    pub status: String,
    pub total_latency_ms: i64,
    pub total_cost_cents: f64,
    pub n_success: usize,
    pub n_failed: usize,
    pub finished_at: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn started_event_round_trip() {
        let e = AnalyzeStartedEvent {
            analysis_id: "a1".into(),
            market_id: "m1".into(),
            prompt_version: "v1".into(),
            providers: vec!["anthropic".into(), "openai".into()],
            started_at: 1_000,
        };
        let s = serde_json::to_string(&e).unwrap();
        let d: AnalyzeStartedEvent = serde_json::from_str(&s).unwrap();
        assert_eq!(d.analysis_id, "a1");
        assert_eq!(d.providers, vec!["anthropic", "openai"]);
    }

    #[test]
    fn provider_done_event_with_error() {
        let e = ProviderDoneEvent {
            analysis_id: "a1".into(),
            provider_id: "openai".into(),
            ok: false,
            latency_ms: 1234,
            tokens_in: None,
            tokens_out: None,
            cost_cents: 0.0,
            error_kind: "rate_limit".into(),
            error_message: Some("429 Too Many Requests".into()),
            finished_at: 2_000,
        };
        let s = serde_json::to_string(&e).unwrap();
        let d: ProviderDoneEvent = serde_json::from_str(&s).unwrap();
        assert!(!d.ok);
        assert_eq!(d.error_kind, "rate_limit");
    }

    #[test]
    fn consensus_event_partial() {
        let e = ConsensusDoneEvent {
            analysis_id: "a1".into(),
            status: "partial".into(),
            n_success: 2,
            n_failed: 1,
            consensus_pred: Some(0.62),
            consensus_side: Some("YES".into()),
            consensus_conf: Some(0.71),
        };
        let s = serde_json::to_string(&e).unwrap();
        let d: ConsensusDoneEvent = serde_json::from_str(&s).unwrap();
        assert_eq!(d.status, "partial");
        assert_eq!(d.n_success, 2);
        assert_eq!(d.n_failed, 1);
    }

    #[test]
    fn finished_event_carries_totals() {
        let e = AnalyzeFinishedEvent {
            analysis_id: "a1".into(),
            status: "completed".into(),
            total_latency_ms: 4200,
            total_cost_cents: 0.12,
            n_success: 3,
            n_failed: 0,
            finished_at: 5_000,
        };
        let s = serde_json::to_string(&e).unwrap();
        let d: AnalyzeFinishedEvent = serde_json::from_str(&s).unwrap();
        assert_eq!(d.total_latency_ms, 4200);
        assert!((d.total_cost_cents - 0.12).abs() < 1e-9);
    }
}
