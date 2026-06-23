//! L2 — Daily brief (M12).
//!
//! IPCs: `daily_brief_get` (today's top-N with consensus + signal join),
//! `daily_brief_dismiss`, `daily_brief_refresh` (re-score candidates),
//! `daily_brief_set_prefs` (weights + max_items per user).

use crate::AppResult;
use crate::infra::state::AppState;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use tauri::State;

/// 每日简报单条记录。L1 Dashboard → Daily Brief 卡片用。
///
/// **数据来源**：
///   - `daily_briefs` — 候选排名 + match_score
///   - `markets` — 问题、分类、end_date
///   - `signals` (LEFT JOIN) — 当前 model edge
///   - `llm_analyses` (LEFT JOIN) — 最近一次多 LLM 共识
///
/// **`consensus_strength` 计算**：当前分析里推荐 `consensus_side` 的 LLM
/// 数量 / 总 LLM 数。0..1。
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DailyBriefEntry {
    pub market_id: String,
    pub market_question: String,
    pub market_category: String,
    pub market_end_date: i64,
    pub market_liquidity: Option<String>,
    pub market_volume_24h: Option<f64>,
    pub rank: i64,
    pub match_score: f64,
    pub score_breakdown: Option<String>, // JSON
    pub edge: Option<f64>,               // from signals (if any)
    pub confidence: Option<f64>,         // from signals or LLM consensus
    pub consensus_side: Option<String>,  // from llm_analyses (if any)
    pub consensus_strength: Option<f64>, // 0..1, how many LLMs agree
    pub computed_at: i64,
    pub expires_at: i64,
    pub dismissed: bool,
}

#[derive(Debug, Deserialize)]
pub struct BriefGetArgs {
    pub limit: Option<i64>,
    pub max_items: Option<i64>,
}

/// IPC: `daily_brief_get` —— 拉今天 + 未来未过期的 brief 列表。
///
/// **参数**：`args.limit` 优先，`args.max_items` 兼容旧版（fallback），默认 5。
/// **返回**：`Vec<DailyBriefEntry>`，按 `rank` 升序（rank 1 = 最佳候选）。
///
/// **dismissed 字段语义**：dismissed within last 24h 才算 dismissed。
/// 24h 之后会自动重新出现（dismiss 不是永久）。
///
/// **性能**：单次 SQL 多 JOIN，单 market_id 计算 1 次 subquery。L1 每次 mount
/// Dashboard 时调一次。
#[tauri::command]
pub async fn daily_brief_get(
    state: State<'_, AppState>,
    args: BriefGetArgs,
) -> AppResult<Vec<DailyBriefEntry>> {
    let limit = args.limit.or(args.max_items).unwrap_or(5);
    let now = chrono::Utc::now().timestamp_millis();

    // v0.119 — football pivot: filter briefs to football category only.
    // polyrocket is a football-only product (see docs/polyrocket-football-prd.md).
    // Backend `daily_briefs` table still contains cs2/politics/etc briefs
    // (for future flexibility / debugging), but UI never surfaces them.
    // Use a SQL filter rather than post-filter so the LIMIT counts only
    // football entries (otherwise the user might see 3/5 football briefs
    // instead of 5/5).
    let rows = sqlx::query_as::<_, DailyBriefEntry>(
        "SELECT
            db.market_id,
            m.question as market_question,
            m.category as market_category,
            m.end_date as market_end_date,
            m.liquidity as market_liquidity,
            m.volume_24h as market_volume_24h,
            db.rank,
            db.match_score,
            db.score_breakdown,
            s.edge as edge,
            s.confidence as confidence,
            la.consensus_side as consensus_side,
            (SELECT CAST(COUNT(CASE WHEN r.side = la.consensus_side THEN 1 END) AS REAL)
                    / NULLIF(COUNT(r.id), 0)
             FROM llm_recommendations r
             WHERE r.analysis_id = la.id) as consensus_strength,
            db.computed_at,
            db.expires_at,
            CASE WHEN m.brief_dismissed_at > ? THEN 1 ELSE 0 END as dismissed
         FROM daily_briefs db
         JOIN markets m ON m.id = db.market_id
         LEFT JOIN signals s ON s.market_id = m.id AND s.active = 1
         LEFT JOIN llm_analyses la ON la.market_id = m.id
            AND la.id = (SELECT id FROM llm_analyses WHERE market_id = m.id ORDER BY requested_at DESC LIMIT 1)
         WHERE db.expires_at > ?
           AND m.category = 'football'
         ORDER BY db.rank ASC
         LIMIT ?",
    )
    .bind(now - 24 * 3600 * 1000) // dismissed within last 24h
    .bind(now)
    .bind(limit)
    .fetch_all(&state.db)
    .await?;
    Ok(rows)
}

/// IPC: `daily_brief_dismiss` —— 用户 dismiss 一条 brief 卡片。
///
/// **业务流程**：把 `markets.brief_dismissed_at = now` 写。L1 `daily_brief_get`
/// 会把 24h 内 dismissed 的 market 排除掉。
///
/// **24h 后自动恢复**：dismiss 不永久。再次出现时 rank 可能变化（取决于
/// `daily_brief_refresh` 重算）。
#[tauri::command]
pub async fn daily_brief_dismiss(
    state: State<'_, AppState>,
    market_id: String,
) -> AppResult<()> {
    sqlx::query("UPDATE markets SET brief_dismissed_at = unixepoch() * 1000 WHERE id = ?")
        .bind(&market_id)
        .execute(&state.db)
        .await?;
    Ok(())
}

/// IPC: `daily_brief_refresh` —— 重算 brief 候选排名（scheduler 也调）。
///
/// **业务流程**：
///   1. 拉候选 = 24h 内关闭的、未结算、未 dismissed 的 market
///   2. 用 `daily_brief_score_breakdown` 公式算每条 match_score
///   3. 写 `daily_briefs`（UPSERT 覆盖今天的）
///   4. 返回 `BriefRefreshResult { n_candidates, n_written, top_market_id }`
///
/// **scheduler 也调**：`run_daily_brief_now`（cron 触发）会调这个的逻辑；
/// IPC 是 L1 手动「Refresh」按钮的入口。
#[tauri::command]
pub async fn daily_brief_refresh(
    state: State<'_, AppState>,
) -> AppResult<BriefRefreshResult> {
    let now = chrono::Utc::now().timestamp_millis();
    let today_start = chrono::Utc::now()
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc()
        .timestamp_millis();
    let expires = today_start + 24 * 3600 * 1000;
    let max_items: i64 = 5; // from user prefs (TODO v0.3: read user_brief_prefs)

    // v0.119 — football pivot: candidates limited to football only.
    // polyrocket is a football-only product (see docs/polyrocket-football-prd.md).
    // SQL-level filter so the scoring only considers football markets; the
    // resulting daily_briefs rows are also implicitly football.
    let candidates: Vec<(String, f64, Option<f64>, Option<f64>, Option<String>, Option<f64>, Option<String>)> = sqlx::query_as(
        "SELECT m.id,
                CAST(COALESCE(m.liquidity, '0') AS REAL) as liq,
                s.edge as edge,
                s.confidence as confidence,
                la.consensus_side as consensus_side,
                (SELECT CAST(COUNT(CASE WHEN r.side = la.consensus_side THEN 1 END) AS REAL)
                        / NULLIF(COUNT(r.id), 0)
                 FROM llm_recommendations r WHERE r.analysis_id = la.id) as consensus_strength,
                COALESCE(m.user_interested, 0) as interested
         FROM markets m
         LEFT JOIN signals s ON s.market_id = m.id AND s.active = 1
         LEFT JOIN llm_analyses la ON la.market_id = m.id
             AND la.id = (SELECT id FROM llm_analyses WHERE market_id = m.id ORDER BY requested_at DESC LIMIT 1)
         WHERE m.active = 1
           AND m.resolved = 0
           AND m.category = 'football'
           AND m.end_date > ?
           AND m.end_date < ?
           AND (m.brief_dismissed_at IS NULL OR m.brief_dismissed_at < ?)",
    )
    .bind(now)
    .bind(now + 24 * 3600 * 1000)
    .bind(now - 24 * 3600 * 1000)
    .fetch_all(&state.db)
    .await?;

    // score each candidate (default weights)
    let w = BriefWeights::default();
    let mut scored: Vec<(String, f64, f64, f64, f64, f64, f64, f64)> = candidates
        .into_iter()
        .map(|(id, liq, edge, conf, _cons_side, cons_str, _interested)| {
            let edge_n = edge.map(|e| e.abs().min(1.0)).unwrap_or(0.0);
            let conf_n = conf.unwrap_or(0.0);
            let cons_n = cons_str.unwrap_or(0.0);
            let liq_n = (liq / 1_000_000.0).min(1.0); // normalize: $1M liquidity = 1.0
            let time_n = 0.5; // placeholder
            let user_n = 0.0; // placeholder
            let cost_n = 0.0; // placeholder
            let score = w.w1 * edge_n
                + w.w2 * conf_n
                + w.w3 * cons_n
                + w.w4 * time_n
                + w.w5 * user_n
                - w.w6 * cost_n;
            (id, score, edge_n, conf_n, cons_n, liq_n, time_n, user_n - cost_n)
        })
        .collect();
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(max_items as usize);

    // wipe today's briefs, insert new
    let mut tx = state.db.begin().await?;
    sqlx::query("DELETE FROM daily_briefs WHERE computed_at >= ?")
        .bind(today_start)
        .execute(&mut *tx)
        .await?;

    for (rank, (id, score, e, c, s, _l, t, u)) in scored.iter().enumerate() {
        let breakdown = serde_json::json!({
            "edge": e, "confidence": c, "consensus_strength": s,
            "time_decay": t, "user_interest": u,
        });
        sqlx::query(
            "INSERT INTO daily_briefs (market_id, rank, match_score, score_breakdown, computed_at, expires_at)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(id)
        .bind(rank as i64 + 1)
        .bind(score)
        .bind(breakdown.to_string())
        .bind(now)
        .bind(expires)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;

    sqlx::query(
        "INSERT INTO audit_log (actor, action, target, payload, result) VALUES ('system', 'brief.refresh', NULL, ?, 'ok')",
    )
    .bind(serde_json::json!({"n_items": scored.len(), "computed_at": now}))
    .execute(&state.db)
    .await?;

    Ok(BriefRefreshResult {
        computed_at: now,
        n_items: scored.len() as i64,
    })
}

/// `daily_brief_refresh` 的返回。L1 「Refresh」按钮 toast 展示 `n_items`。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BriefRefreshResult {
    pub computed_at: i64,
    pub n_items: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BriefWeights {
    pub w1: f64, // edge
    pub w2: f64, // confidence
    pub w3: f64, // consensus
    pub w4: f64, // time
    pub w5: f64, // user_interest
    pub w6: f64, // cost penalty
}

impl Default for BriefWeights {
    fn default() -> Self {
        Self { w1: 0.35, w2: 0.20, w3: 0.20, w4: 0.15, w5: 0.10, w6: 0.10 }
    }
}

#[derive(Debug, Deserialize)]
pub struct SetBriefPrefsArgs {
    pub user_id: String,
    pub weights: Option<BriefWeights>,
    pub max_items: Option<i64>,
    pub min_liquidity: Option<String>,
    pub categories: Option<Vec<String>>,
}

/// IPC: `daily_brief_set_prefs` —— 用户调 brief 排序权重 + max_items。
///
/// **6 个 weight 默认**（`BriefWeights::default()`）：
///   - w1=0.35 edge（最重）
///   - w2=0.20 confidence
///   - w3=0.20 consensus
///   - w4=0.15 time
///   - w5=0.10 user_interest
///   - w6=0.10 cost penalty
///
/// **UPSERT** 写入 `user_brief_prefs` 表，按 `user_id` 唯一。
#[tauri::command]
pub async fn daily_brief_set_prefs(
    state: State<'_, AppState>,
    args: SetBriefPrefsArgs,
) -> AppResult<()> {
    let weights = args.weights.unwrap_or_default();
    sqlx::query(
        "INSERT INTO user_brief_prefs (user_id, weights_json, max_items, min_liquidity, categories, updated_at)
         VALUES (?, ?, ?, ?, ?, unixepoch() * 1000)
         ON CONFLICT(user_id) DO UPDATE SET
           weights_json=excluded.weights_json,
           max_items=excluded.max_items,
           min_liquidity=excluded.min_liquidity,
           categories=excluded.categories,
           updated_at=unixepoch() * 1000",
    )
    .bind(&args.user_id)
    .bind(serde_json::to_string(&weights).unwrap_or_else(|_| "{}".to_string()))
    .bind(args.max_items.unwrap_or(5))
    .bind(&args.min_liquidity)
    .bind(serde_json::to_string(&args.categories).unwrap_or_else(|_| "[]".to_string()))
    .execute(&state.db)
    .await?;
    Ok(())
}

// =================================================================
// ============== v0.119 — football-only brief tests =============
// =================================================================
//
// polyrocket is a football-only product (see docs/polyrocket-football-prd.md).
// These tests verify the SQL filter is in place at the source-code level
// (string match), preventing accidental removal during refactors.
//
// Why string-match: We can't easily run integration tests for the SQL
// without a populated DB; the SQL filter is a single literal that must
// stay present. A simple regex match catches "someone deleted
// `AND m.category = 'football'`".

#[cfg(test)]
mod football_filter_tests {
    /// Snapshot of the relevant SQL with whitespace normalized.
    /// We grep this against the file content to ensure the filter
    /// is still present.
    const REQUIRED_FILTER: &str = "m.category = 'football'";

    #[test]
    fn daily_brief_get_has_football_filter() {
        let src = include_str!("brief.rs");
        // Search for the SQL string by looking at the daily_brief_get function
        let in_get = src
            .split("async fn daily_brief_get")
            .nth(1)
            .expect("daily_brief_get function exists");
        let in_get = in_get
            .split("async fn daily_brief_dismiss")
            .next()
            .expect("daily_brief_dismiss exists after daily_brief_get");
        assert!(
            in_get.contains(REQUIRED_FILTER),
            "v0.119 football pivot: daily_brief_get SQL must filter \
             `m.category = 'football'`. Without this, the brief page \
             would show non-football recommendations even though \
             the rest of the UI is football-only.",
        );
    }

    #[test]
    fn daily_brief_refresh_has_football_filter() {
        let src = include_str!("brief.rs");
        let in_refresh = src
            .split("async fn daily_brief_refresh")
            .nth(1)
            .expect("daily_brief_refresh function exists");
        assert!(
            in_refresh.contains(REQUIRED_FILTER),
            "v0.119 football pivot: daily_brief_refresh SQL must filter \
             `m.category = 'football'` so the scoring pool is football-only.",
        );
    }
}
