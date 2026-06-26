//! L3 —— 足球上下文：休息天数与疲劳度（P2-2）。
//!
//! 为足球队计算休息天数与疲劳度上下文，可注入到 LLM prompt
//! 中以提升比赛预测质量。
//!
//! 规范：P2-2 Football Context（Rest Days）

use serde::{Deserialize, Serialize};
use specta::Type;

/// 单场比赛的足球上下文。
///
/// `home_rest_days` / `away_rest_days` 在没有已知前一场比赛时为 `None`
///（例如赛季初）。`home_matches_7d` 统计该比赛前 7 天内的比赛数。
/// `home_fatigue` 是派生的 "Low"/"Medium"/"High" 标签。
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct FootballContext {
    pub home_team: String,
    pub away_team: String,
    pub home_rest_days: Option<i64>,
    pub away_rest_days: Option<i64>,
    pub home_matches_7d: i64,
    pub away_matches_7d: i64,
    pub home_fatigue: String,
    pub away_fatigue: String,
}

/// 计算两个比赛日期之间的休息天数。
///
/// `last_match_date` 与 `current_date` 是 Unix 毫秒时间戳。
/// 返回以整天为单位的差值（向下取整）。若结果为负，
/// 表示「上一场」比赛日期晚于当前日期（实际不应发生）。
pub fn compute_rest_days(last_match_date: i64, current_date: i64) -> i64 {
    let diff_ms = current_date - last_match_date;
    diff_ms / (24 * 60 * 60 * 1000)
}

/// 根据休息天数和近期比赛负荷分类疲劳度。
///
/// - "High" 当 rest_days < 3 或 matches_7d >= 3
/// - "Medium" 当 rest_days 3..=5
/// - 其余情况为 "Low"（rest_days >= 6）
pub fn classify_fatigue(rest_days: i64, matches_7d: i64) -> String {
    if rest_days < 3 || matches_7d >= 3 {
        "High".to_string()
    } else if rest_days <= 5 {
        "Medium".to_string()
    } else {
        "Low".to_string()
    }
}

/// 将 `FootballContext` 格式化为适合注入到 LLM prompt 的字符串。
/// 这能为模型提供结构化的球队疲劳信息，而不会让 prompt
/// 被 JSON 撑得过于冗长。
pub fn to_prompt_context(ctx: &FootballContext) -> String {
    let home_rest = ctx
        .home_rest_days
        .map(|d| format!("{} days", d))
        .unwrap_or_else(|| "unknown".to_string());
    let away_rest = ctx
        .away_rest_days
        .map(|d| format!("{} days", d))
        .unwrap_or_else(|| "unknown".to_string());

    format!(
        "Fatigue context:\n\
         - {} : rest={} (last 7d: {} matches, fatigue={})\n\
         - {} : rest={} (last 7d: {} matches, fatigue={})",
        ctx.home_team, home_rest, ctx.home_matches_7d, ctx.home_fatigue,
        ctx.away_team, away_rest, ctx.away_matches_7d, ctx.away_fatigue,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rest_days_basic() {
        let day_ms = 24 * 60 * 60 * 1000;
        assert_eq!(compute_rest_days(0, 3 * day_ms), 3);
        assert_eq!(compute_rest_days(0, 0), 0);
    }

    #[test]
    fn fatigue_high_low_rest() {
        assert_eq!(classify_fatigue(2, 1), "High");
    }

    #[test]
    fn fatigue_high_many_matches() {
        assert_eq!(classify_fatigue(7, 3), "High");
    }

    #[test]
    fn fatigue_medium() {
        assert_eq!(classify_fatigue(4, 1), "Medium");
        assert_eq!(classify_fatigue(3, 1), "Medium");
        assert_eq!(classify_fatigue(5, 1), "Medium");
    }

    #[test]
    fn fatigue_low() {
        assert_eq!(classify_fatigue(6, 1), "Low");
        assert_eq!(classify_fatigue(10, 1), "Low");
    }

    #[test]
    fn prompt_context_contains_teams() {
        let ctx = FootballContext {
            home_team: "Arsenal".into(),
            away_team: "Chelsea".into(),
            home_rest_days: Some(4),
            away_rest_days: None,
            home_matches_7d: 2,
            away_matches_7d: 1,
            home_fatigue: "Medium".into(),
            away_fatigue: "Low".into(),
        };
        let s = to_prompt_context(&ctx);
        assert!(s.contains("Arsenal"));
        assert!(s.contains("Chelsea"));
        assert!(s.contains("unknown"));
    }
}
