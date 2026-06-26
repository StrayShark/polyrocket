//! L3 — `train_job` IPC（进程间通信）的进度事件载荷。
//!
//! v0.17a — `commands::sidecar::train_job` 通过 `AppHandle::emit`
//! 发出这些事件，让 L1（前端）的 ModelLab 页面可以显示
//! 训练进行中的指示器以及最终结果。
//!
//! 事件名称（全部位于全局 Tauri 事件总线）：
//!   - `train_job:started`   — IPC 已派发，训练开始
//!   - `train_job:finished`  — 训练完成或失败
//!
//! 与 `llm_analyze`（有 N 个并行 provider，并按 provider 发出事件）不同，
//! `train_job` 是单次 Python 调用内部的顺序 4-trial 扫描。
//! 由于 stdio 协议是一请求一响应，Python 端不会发出按 trial 的事件。
//! L1 在运行期间显示通用的"Training…"指示药丸，
//! 收到 finished 事件后再显示完整结果（每 trial 统计 + 最佳 Brier + 参数）。
//!
//! `job_id` 是我们传递给 Python 端进程的 UUID；
//! 用于关联 started/finished 一对事件。

use serde::{Deserialize, Serialize};

/// `train_job:started` 的载荷。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainStartedEvent {
    pub job_id: String,
    /// Python 端进程将运行的 trial 总数（1-4）。
    /// L1 用它在进度药丸上渲染"N trials"提示。
    pub n_trials: u32,
    /// 每个 trial 的训练轮数。v0.17a 默认 80。
    pub epochs: u32,
    /// 以毫秒为单位的墙钟起始时间。
    pub started_at: i64,
}

/// `train_job:finished` 的载荷。
///
/// `status` 取值之一：
///   - `"completed"` — 全部 trial 完成，最佳模型已落盘
///   - `"failed"`    — 扫描或持久化出错；见 `message`
///
/// 失败时 `best_brier` 为 `None`。失败时 `trials` 为空
/// （我们在错误发生前没能记录任何 trial 统计）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainFinishedEvent {
    pub job_id: String,
    pub status: String,
    /// 越低越好。失败时为 `None`。
    pub best_brier: Option<f64>,
    /// 最佳 trial 的权重，形如 `{"w0", "w1", "w2"}`。失败时为 `None`。
    pub best_params: Option<serde_json::Value>,
    /// 每 trial 的统计（失败时为空）。
    pub trials: Vec<TrainTrialDto>,
    pub duration_ms: i64,
    /// 端进程写入的候选 JSON 的绝对路径。
    /// 失败时为 `None`。
    pub candidate_path: Option<String>,
    /// 人类可读的错误消息。成功时为 `None`。
    pub message: Option<String>,
    pub finished_at: i64,
}

/// finished 事件中单个 trial 的统计。与
/// `domain::lab::sidecar::TrainTrial` 镜像对应，但
/// 放在此处以保持进度模块自包含。
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
