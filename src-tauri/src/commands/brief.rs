use crate::AppResult;
use crate::state::AppState;
use chrono::TimeZone;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use tauri::State;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DailyBriefEntry {
    pub market_id: String,
    pub market_question: String,
    pub market_category: String,
    pub market_end_date: i64,
    pub market_liquidity: Option<String>,
    pub market_volume_24h: Option<String>,
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

#[tauri::command]
pub async fn daily_brief_get(
    state: State<'_, AppState>,
    args: BriefGetArgs,
) -> AppResult<Vec<DailyBriefEntry>> {
    let limit = args.limit.or(args.max_items).unwrap_or(5);
    let now = chrono::Utc::now().timestamp_millis();

    // join daily_briefs with markets, left join latest signal + llm analysis
    // simplified: only show briefs not dismissed, sorted by rank
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

    // candidate set: markets closing within 24h, active, not resolved, not dismissed
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
