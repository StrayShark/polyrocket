//! L3 — `llm_analyze` 的进度事件 payload。
//!
//! v0.15a — `commands::llm::llm_analyze` 通过
//! `AppHandle::emit` 发送这些事件,这样 L1 Analysis 页面能在每次
//! LLM call 完成时显示每个 provider 的状态(而不是阻塞整个 fan-out)。
//!
//! 事件名称 (都在全局 Tauri event bus 上,无 namespace):
//!   - `llm_analyze:started`         — analysis row 已创建,fan-out 开始
//!   - `llm_analyze:provider_done`   — 一个 provider call 完成 (成功或失败)
//!   - `llm_analyze:consensus_done`  — consensus 计算完成 (最后一个 provider 之后)
//!   - `llm_analyze:finished`        — 完整 analyze 完成 (success / partial / failed)
//!
//! `analysis_id` 是 `llm_analyses.id` 的 UUID 字符串,
//! L1 可以按该字段关联(未来多个 analyze 可以从不同 UI 面板并发)。

use serde::{Deserialize, Serialize};

/// `llm_analyze:started` 的 Payload。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyzeStartedEvent {
    pub analysis_id: String,
    pub market_id: String,
    pub prompt_version: String,
    pub providers: Vec<String>,
    pub started_at: i64,
}

/// `llm_analyze:provider_done` 的 Payload。
///
/// 每个 provider 触发一次,无论 call 成功或失败。
/// `error_kind` 是以下之一: `none`, `auth`, `rate_limit`, `timeout`,
/// `network`, `parse`, `model_not_found`, `internal`(对应
/// `domain::llm::dispatch` 中的 `CallErrorKind` enum)。
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

/// `llm_analyze:consensus_done` 的 Payload。
///
/// 每个 analyze 触发一次,在所有 provider 完成且 consensus
/// 计算后。`status` 是以下之一: `completed`,
/// `partial`, `failed`(匹配现有的 `llm_analyses.status`)。
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

/// `llm_analyze:finished` 的 Payload。
///
/// 在最后触发 (在 consensus_done 之后)。这是
/// L1 通常 `await` 的事件,用于在变更最终状态前等待。
/// 携带 totals 让 L1 可以显示 "analyze 用时 4.2s,花费 $0.12"。
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
