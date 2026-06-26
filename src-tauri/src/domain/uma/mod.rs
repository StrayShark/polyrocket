//! L3 — UMA 争议状态（P2-3）。
//!
//! 对 Polymarket UMA（Universal Market Adapter）争议状态
//! 在单个市场上的简化表示。完整的 Polymarket UMA API 集成是
//! 未来的里程碑；本模块目前只提供领域类型与分类逻辑。
//!
//! 规范：P2-3 UMA Dispute Status

use serde::{Deserialize, Serialize};
use specta::Type;

/// 对某个市场的简化 UMA 争议状态。
///
/// `status` 取值之一:
/// - `"clear"` —— 无争议,市场干净地结算
/// - `"disputed"` —— 已发起争议
/// - `"resolving"` —— 争议正在仲裁中
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct UmaDisputeStatus {
    pub market_id: String,
    pub status: String,
    pub detail: Option<String>,
    pub raised_at: Option<i64>,
    pub raised_by: Option<String>,
}

/// 将原始的 Polymarket UMA API 状态字符串映射为我们的简化状态。
///
/// Polymarket / UMA 根据 resolve 阶段使用多种状态字符串。
/// 我们将其归并为三个桶：
///
/// | 原始状态                          | 我们的状态  |
/// |----------------------------------|-------------|
/// | "resolved", "confirmed", "final"   | "clear"（清晰）    |
/// | "disputed", "proposed", "challenged"| "disputed"（争议中）|
/// | "resolving", "pending", "liveness" | "resolving"（解决中）|
/// | 其他任意值                          | "clear"（清晰）    |
pub fn classify_uma_status(raw_status: &str) -> String {
    let lower = raw_status.to_lowercase();
    match lower.as_str() {
        "disputed" | "proposed" | "challenged" => "disputed".to_string(),
        "resolving" | "pending" | "liveness" => "resolving".to_string(),
        _ => "clear".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_clear() {
        assert_eq!(classify_uma_status("resolved"), "clear");
        assert_eq!(classify_uma_status("confirmed"), "clear");
        assert_eq!(classify_uma_status("final"), "clear");
        assert_eq!(classify_uma_status(""), "clear");
    }

    #[test]
    fn classify_disputed() {
        assert_eq!(classify_uma_status("disputed"), "disputed");
        assert_eq!(classify_uma_status("proposed"), "disputed");
        assert_eq!(classify_uma_status("challenged"), "disputed");
    }

    #[test]
    fn classify_resolving() {
        assert_eq!(classify_uma_status("resolving"), "resolving");
        assert_eq!(classify_uma_status("pending"), "resolving");
        assert_eq!(classify_uma_status("liveness"), "resolving");
    }

    #[test]
    fn classify_case_insensitive() {
        assert_eq!(classify_uma_status("DISPUTED"), "disputed");
        assert_eq!(classify_uma_status("Pending"), "resolving");
    }
}
