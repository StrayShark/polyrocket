//! L3 — 共识。
//!
//! 将多个 LLM [`crate::domain::llm::CallOutcome`] 聚合成
//! 单一的方向 + 强度裁决（用于每日简报和信号 UI）。
//!
//! **状态（v0.3c）：存根。** 公共 API 已声明，便于 L2 命令
//! 接入 import。真正的实现将在 M9「多 LLM 共识」里程碑
//! 进入 active 后落地。

use serde::{Deserialize, Serialize};

/// LLM 达成一致的方向（`"YES"` / `"NO"` / `"MAYBE"`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConsensusSide {
    Yes,
    No,
    Maybe,
}

impl ConsensusSide {
    pub fn as_str(self) -> &'static str {
        match self {
            ConsensusSide::Yes => "YES",
            ConsensusSide::No => "NO",
            ConsensusSide::Maybe => "MAYBE",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "YES" => Some(ConsensusSide::Yes),
            "NO" => Some(ConsensusSide::No),
            "MAYBE" => Some(ConsensusSide::Maybe),
            _ => None,
        }
    }
}

/// 对 N 个 LLM 答案执行共识的结果。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Consensus {
    pub side: ConsensusSide,
    /// 与胜出方向一致的 LLM 比例（0..1）。
    pub strength: f64,
    /// 参与本共识的 LLM 数量。
    pub n_models: usize,
    /// 各参与 LLM 置信度的平均值。
    pub avg_confidence: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn consensus_side_round_trip() {
        for s in [ConsensusSide::Yes, ConsensusSide::No, ConsensusSide::Maybe] {
            assert_eq!(ConsensusSide::parse(s.as_str()), Some(s));
        }
    }
}
