//! L2 —— 泊松比分矩阵（P2-1）。
//!
//! IPC:poisson_score_matrix —— 为足球市场计算 Dixon-Coles 泊松比分矩阵，
//! 在可获取最新 LLM 分析 `framework_breakdown` 时，从中提取 lambda（预期进球）。

use crate::AppResult;
use crate::domain::poisson::{compute_poisson_matrix, ScoreMatrixResult};
use crate::infra::state::AppState;
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::State;

/// 包装泊松比分矩阵结果、供前端使用的 DTO。
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ScoreMatrixResultDto {
    pub market_id: String,
    pub lambda_h: f64,
    pub lambda_a: f64,
    pub matrix: [[f64; 5]; 5],
    pub most_likely: Vec<(String, f64)>,
}

/// IPC:poisson_score_matrix —— 为某市场计算泊松比分矩阵。
///
/// 尝试从该市场最新的 LLM 推荐 `framework_breakdown` 中提取
/// `lambda_home_goals` / `lambda_away_goals`。若没有 LLM 分析，
/// 或 framework_breakdown 缺少这些字段，则回退到合理默认值
/// （lambda_h=1.5，lambda_a=1.0）。
#[tauri::command]
pub async fn poisson_score_matrix(
    state: State<'_, AppState>,
    market_id: String,
) -> AppResult<ScoreMatrixResultDto> {
    // 默认 lambda —— 体现足球比赛中典型的主场优势。
    let mut lambda_h = 1.5;
    let mut lambda_a = 1.0;

    // 尝试从最新 LLM 推荐的 raw_response 中提取 lambda，
    // 该 JSON 内含带 `lambda_home_goals` 和 `lambda_away_goals`
    // 字段的 `framework_breakdown` 对象（v0.118+）。
    let raw_response: Option<(String,)> = sqlx::query_as(
        "SELECT r.raw_response
         FROM llm_recommendations r
         JOIN llm_analyses a ON a.id = r.analysis_id
         WHERE a.market_id = ? AND r.raw_response IS NOT NULL AND r.parse_ok = 1
         ORDER BY r.created_at DESC
         LIMIT 1",
    )
    .bind(&market_id)
    .fetch_optional(&state.db)
    .await?;

    if let Some((raw,)) = raw_response {
        // 尽力解析：从 JSON 中提取 framework_breakdown 下的
        // lambda_home_goals 和 lambda_away_goals。使用 serde_json::Value
        // 是为了保持鲁棒性 —— 不同版本的 prompt 字段结构不一样。
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&raw) {
            if let Some(fb) = val.get("framework_breakdown") {
                if let Some(lh) = fb.get("lambda_home_goals").and_then(|v| v.as_f64()) {
                    lambda_h = lh;
                }
                if let Some(la) = fb.get("lambda_away_goals").and_then(|v| v.as_f64()) {
                    lambda_a = la;
                }
            }
        }
    }

    let result: ScoreMatrixResult = compute_poisson_matrix(lambda_h, lambda_a);

    Ok(ScoreMatrixResultDto {
        market_id,
        lambda_h: result.lambda_h,
        lambda_a: result.lambda_a,
        matrix: result.matrix,
        most_likely: result.most_likely,
    })
}
