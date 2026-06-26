//! L2 —— 足球上下文:休息天数与疲劳度（P2-2）。
//!
//! IPC:`get_football_context` —— 返回某足球市场
//! 的休息日与疲劳上下文。目前因尚未接入历史赛程数据,
//! 返回估算/默认值;球队名从 market 的
//! question 字段中提取。

use crate::AppResult;
use crate::domain::football_context::{classify_fatigue, FootballContext};
use crate::infra::state::AppState;
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::State;

/// 供前端使用的足球上下文 DTO。
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct FootballContextDto {
    pub market_id: String,
    pub home_team: String,
    pub away_team: String,
    pub home_rest_days: Option<i64>,
    pub away_rest_days: Option<i64>,
    pub home_matches_7d: i64,
    pub away_matches_7d: i64,
    pub home_fatigue: String,
    pub away_fatigue: String,
}

/// IPC:`get_football_context` —— 返回某市场的疲劳上下文。
///
/// 从市场的 `question` 字段中提取主/客队名。
/// 由于尚未集成历史赛程数据,这里返回默认
/// 估算的休息日与疲劳值。未来版本会查询赛程表
/// 计算真实的休息天数与近 7 天比赛数。
#[tauri::command]
pub async fn get_football_context(
    state: State<'_, AppState>,
    market_id: String,
) -> AppResult<FootballContextDto> {
    // 拉取市场 question,以提取球队名。
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT question FROM markets WHERE id = ?",
    )
    .bind(&market_id)
    .fetch_optional(&state.db)
    .await?;

    let question = row.map(|(q,)| q).unwrap_or_default();
    let (home_team, away_team) = extract_team_names(&question);

    // 默认估算值 —— 暂未接入历史赛程数据。
    // 假设双方均按标准休息 6 天、近 7 天比赛 1 场。
    let home_rest_days: i64 = 6;
    let away_rest_days: i64 = 6;
    let home_matches_7d: i64 = 1;
    let away_matches_7d: i64 = 1;

    let home_fatigue = classify_fatigue(home_rest_days, home_matches_7d);
    let away_fatigue = classify_fatigue(away_rest_days, away_matches_7d);

    let ctx = FootballContext {
        home_team,
        away_team,
        home_rest_days: Some(home_rest_days),
        away_rest_days: Some(away_rest_days),
        home_matches_7d,
        away_matches_7d,
        home_fatigue,
        away_fatigue,
    };

    Ok(FootballContextDto {
        market_id,
        home_team: ctx.home_team,
        away_team: ctx.away_team,
        home_rest_days: ctx.home_rest_days,
        away_rest_days: ctx.away_rest_days,
        home_matches_7d: ctx.home_matches_7d,
        away_matches_7d: ctx.away_matches_7d,
        home_fatigue: ctx.home_fatigue,
        away_fatigue: ctx.away_fatigue,
    })
}

/// 尽力从市场 question 中提取主/客队名。
///
/// Polymarket 足球 question 通常遵循以下格式:
/// - "Arsenal vs Chelsea"
/// - "Arsenal vs. Chelsea"
/// - "Manchester City vs Liverpool"
/// 解析失败时返回 ("Home", "Away") 占位。
fn extract_team_names(question: &str) -> (String, String) {
    // 尝试常见分隔符: " vs ", " vs. ", " v ", " v. ", " @ "
    for sep in &[" vs. ", " vs ", " v. ", " v ", " @ "] {
        if let Some(idx) = question.to_lowercase().find(sep) {
            let home = question[..idx].trim().to_string();
            let away = question[idx + sep.len()..].trim().to_string();
            if !home.is_empty() && !away.is_empty() {
                return (home, away);
            }
        }
    }
    ("Home".to_string(), "Away".to_string())
}
