//! L2 —— 活跃信号（M5）。
//!
//! IPC:list_active_signals（按最小 edge + 类别过滤），
//! `recompute_signals`（通过 Rust inference 模块计算信号，
//! 在 v0.122c 之前是返回 0 的空实现）。

use crate::AppResult;
use crate::infra::state::AppState;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use tauri::State;

/// Signal 数据传输对象。JOIN 了 `markets` 表的 `question` / `slug` 字段，
/// 让 L1 「Signals」页不用再二次查 market。
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct SignalDto {
    pub id: i64,
    pub market_id: String,
    pub computed_at: i64,
    pub model_version: String,
    pub predicted_prob: f64,
    pub market_prob: f64,
    pub edge: f64,
    pub confidence: f64,
    pub horizon_hours: i64,
    pub rationale: Option<String>,
    pub market_question: Option<String>,
    pub market_slug: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ListSignalsArgs {
    pub min_edge: Option<f64>,
    pub category: Option<String>,
    pub limit: Option<i64>,
}

/// IPC: `list_active_signals` —— 拉所有 active signals（按 |edge| 降序）。
///
/// **`min_edge` 默认 0.05**：过滤 5% 以下的低确信度信号。
/// **`limit` 默认 50**：L1 dashboard 一次渲染 ≤ 50 行。
/// **`category` 当前未过滤**：UI 留 filter 但 SQL 不带（v0.62+ 加）。
///
/// **`active = 1` 含义**：信号没被 `domain::signal::expire_for_closed_markets`
/// 标记为 expired。M2 每日重算会刷新。
#[tauri::command]
pub async fn list_active_signals(
    state: State<'_, AppState>,
    args: ListSignalsArgs,
) -> AppResult<Vec<SignalDto>> {
    let min_edge = args.min_edge.unwrap_or(0.05);
    let limit = args.limit.unwrap_or(50);

    let rows = sqlx::query_as::<_, SignalDto>(
        "SELECT s.id, s.market_id, s.computed_at, s.model_version, s.predicted_prob, s.market_prob, s.edge, s.confidence, s.horizon_hours, s.rationale,
                m.question AS market_question, m.slug AS market_slug
         FROM signals s
         JOIN markets m ON m.id = s.market_id
         WHERE s.active = 1 AND ABS(s.edge) >= ?
         ORDER BY ABS(s.edge) DESC
         LIMIT ?",
    )
    .bind(min_edge)
    .bind(limit)
    .fetch_all(&state.db)
    .await?;

    Ok(rows)
}

/// IPC:recompute_signals —— 手动触发 signal 重算。
///
/// **v0.122c 实现**：运行 v0.122b 的 Rust `inference::predict_from_markets`
/// 拿到所有 active markets 的预测，并写入 `signals` 表。**此前是一个
/// 返回 0 的 stub** —— Signals 页的 "Recompute" 按钮点击后没有任何效果，
/// UI 上 football markets 始终显示 "No active signals"。现在端到端
/// E2E 测试可以走通：
///   1. 用户在 Markets → football market detail 点击 "Run analysis"
///   2. LLM 调 `llm_analyze` IPC 写入 `llm_recommendations`
///   3. 用户在 Signals 页点击 "Recompute"
///   4. `recompute_signals` 跑 inference 算出所有 markets 的 edge
///   5. 写入 `signals` 表，Signals 页 + Dashboard + MarketDetail 都会更新
///
/// **Returns**：写入的 signal 数量（包含对老 markets 的覆盖更新）。
#[tauri::command]
pub async fn recompute_signals(state: State<'_, AppState>) -> AppResult<usize> {
    use crate::commands::active_model::read_active_model_from_disk;
    use crate::domain::lab::inference::{self, InferenceWeights, MarketInput};

    // 1) 从 active.json 解析模型权重 + 版本（或回退）。
    //    `read_active_model_from_disk` 在文件缺失时返回 Ok(None)——
    //    这是常见的开发场景（尚未 promote 任何模型）。
    //    我们回退到 Python 侧车使用的同一三元组,
    //    这样在全新 DB 上 e2e 流程也能工作。
    let (weights, model_version, brier_score): (InferenceWeights, String, Option<f64>) =
        match read_active_model_from_disk() {
            Ok(Some(active)) => {
                // v0.50+ active.json 把 weights 存为顶层
                // `weights: [w0, w1, w2]` 数组。我们在这里
                // 合成 `InferenceWeights::from_active_json_best`
                // 所期望的 `best` 块，使解析路径可以复用。
                let best = if let Some(ws) = active.weights.as_ref().filter(|v| v.len() == 3) {
                    serde_json::json!({
                        "w0": ws[0],
                        "w1": ws[1],
                        "w2": ws[2],
                        "horizon_norm_hours": 168.0,
                    })
                } else {
                    serde_json::json!({})
                };
                let w = InferenceWeights::from_active_json_best(&best);
                (w, active.model_version, active.best_brier)
            }
            _ => (
                InferenceWeights::fallback(),
                "logistic-fallback-0.1.0".to_string(),
                None,
            ),
        };

    // 2) 拉取所有 active 且未结算、且具备有效 YES 价的市场。
    //    `yes_price REAL` 由 `sync_markets` 写入；NULL / 0
    //    视为「无价格」并跳过（推理层会把 0 钳制成
    //    一个退化的 edge，没有意义）。
    let rows: Vec<(String, Option<f64>, i64)> = sqlx::query_as(
        "SELECT id, yes_price, created_at
         FROM markets
         WHERE active = 1 AND resolved = 0 AND yes_price IS NOT NULL AND yes_price > 0",
    )
    .fetch_all(&state.db)
    .await?;

    if rows.is_empty() {
        return Ok(0);
    }

    // 3) 构造 MarketInput 列表。
    //    `market_age_hours` = (当前时间 - created_at) 的小时数。
    let now_ms = chrono::Utc::now().timestamp_millis();
    let inputs: Vec<MarketInput> = rows
        .iter()
        .map(|(id, yes, created_at_ms)| {
            let age_ms = (now_ms - *created_at_ms).max(0);
            let age_h = age_ms as f64 / 3_600_000.0;
            // 类型转换：DB 中 yes_price 是 REAL；NULL 时退到 0.5。
            let price = yes.unwrap_or(0.5).clamp(0.0, 1.0);
            MarketInput {
                market_id: id.as_str(),
                price,
                market_age_hours: age_h,
            }
        })
        .collect();

    // 4) 运行推理。这是纯函数调用（无 IO），开销很低。
    let result = inference::predict_from_markets(
        &weights,
        &model_version,
        brier_score,
        &inputs,
    );

    // 5) 刷新该 model_version 的 signals。我们在一个事务里
    //    先 DELETE 同一 `(model_version)` 的所有旧行，
    //    再 INSERT 新批次，保证部分失败可回滚。
    //    `signals` 表没有唯一约束，`ON CONFLICT` 不适用；
    //    DELETE+INSERT 模式更简单，且契合「Recompute」
    //    的「最新为准」语义。
    //
    //    `kind` 与 `polarity` 在 v0.62 新增；v0.122c 写它们，
    //    这样 Dashboard 「Recent activity」可按
    //    `kind='inference'` 过滤。
    let mut tx = state.db.begin().await?;
    sqlx::query("DELETE FROM signals WHERE model_version = ?")
        .bind(&model_version)
        .execute(&mut *tx)
        .await?;
    let mut written = 0usize;
    for pred in result.predictions.iter() {
        let market_id = pred.market_id.clone();
        let market_yes = rows
            .iter()
            .find(|(id, _, _)| id == &market_id)
            .and_then(|(_, y, _)| *y)
            .unwrap_or(0.5);
        let edge = pred.prob - market_yes;
        let polarity = if edge.abs() < 0.005 {
            "neutral"
        } else if edge > 0.0 {
            "bullish"
        } else {
            "bearish"
        };
        let summary: String = pred.rationale.chars().take(200).collect();
        // 48 小时 horizon 与现有 signals schema 默认值一致，
        // 也是 `list_active_signals` 渲染所使用的值。
        let horizon_hours: i64 = 48;

        sqlx::query(
            "INSERT INTO signals (
                market_id, computed_at, model_version, predicted_prob, market_prob,
                edge, confidence, horizon_hours, rationale, kind, summary, polarity, active
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 1)",
        )
        .bind(&market_id)
        .bind(now_ms)
        .bind(&model_version)
        .bind(pred.prob)
        .bind(market_yes)
        .bind(edge)
        .bind(pred.confidence)
        .bind(horizon_hours)
        .bind(&pred.rationale)
        .bind("inference")
        .bind(&summary)
        .bind(polarity)
        .execute(&mut *tx)
        .await?;
        written += 1;
    }
    tx.commit().await?;
    Ok(written)
}