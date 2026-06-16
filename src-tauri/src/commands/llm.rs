use crate::AppResult;
use crate::state::AppState;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use tauri::State;
use uuid::Uuid;

// ---------- DTOs ----------

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct LlmProviderDto {
    pub id: String,
    pub display_name: String,
    pub enabled: bool,
    pub api_base: Option<String>,
    pub key_alias: String,
    pub default_model: String,
    pub timeout_ms: i64,
    pub cost_per_1k_in: Option<f64>,
    pub cost_per_1k_out: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmRecommendationDto {
    pub id: i64,
    pub analysis_id: String,
    pub provider_id: String,
    pub provider_name: String,
    pub predicted_prob: Option<f64>,
    pub side: Option<String>,
    pub confidence: Option<f64>,
    pub reasoning: Option<String>,
    pub latency_ms: Option<i64>,
    pub tokens_in: Option<i64>,
    pub tokens_out: Option<i64>,
    pub cost_cents: Option<f64>,
    pub parse_ok: bool,
    pub parse_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmAnalysisDto {
    pub id: String,
    pub market_id: String,
    pub signal_id: Option<i64>,
    pub prompt_version: String,
    pub requested_at: i64,
    pub completed_at: Option<i64>,
    pub status: String,
    pub consensus_predicted: Option<f64>,
    pub consensus_side: Option<String>,
    pub consensus_conf: Option<f64>,
    pub total_latency_ms: Option<i64>,
    pub cost_cents: Option<f64>,
    pub triggered_by: String,
    pub recommendations: Vec<LlmRecommendationDto>,
}

#[derive(Debug, Deserialize)]
pub struct AnalyzeArgs {
    pub market_id: String,
    pub signal_id: Option<i64>,
    pub prompt_version: Option<String>,
    pub provider_ids: Option<Vec<String>>, // override which providers to call
    pub triggered_by: Option<String>,        // 'user:<id>' | 'auto:signal_refresh'
}

#[derive(Debug, Serialize)]
pub struct LlmPerformanceRow {
    pub provider_id: String,
    pub provider_name: String,
    pub n_recommendations: i64,
    pub n_evaluated: i64, // where outcome known
    pub win_rate: f64,
    pub brier: f64,
    pub avg_confidence: f64,
    pub total_cost_cents: f64,
}

// ---------- Commands ----------

#[tauri::command]
pub async fn list_llm_providers(state: State<'_, AppState>) -> AppResult<Vec<LlmProviderDto>> {
    let rows = sqlx::query_as::<_, LlmProviderDto>(
        "SELECT id, display_name, enabled, api_base, key_alias, default_model, timeout_ms, cost_per_1k_in, cost_per_1k_out
         FROM llm_providers ORDER BY display_name",
    )
    .fetch_all(&state.db)
    .await?;
    Ok(rows)
}

#[tauri::command]
pub async fn upsert_llm_provider(
    state: State<'_, AppState>,
    provider: LlmProviderDto,
) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO llm_providers (id, display_name, enabled, api_base, key_alias, default_model, timeout_ms, cost_per_1k_in, cost_per_1k_out, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, unixepoch() * 1000)
         ON CONFLICT(id) DO UPDATE SET
           display_name=excluded.display_name,
           enabled=excluded.enabled,
           api_base=excluded.api_base,
           key_alias=excluded.key_alias,
           default_model=excluded.default_model,
           timeout_ms=excluded.timeout_ms,
           cost_per_1k_in=excluded.cost_per_1k_in,
           cost_per_1k_out=excluded.cost_per_1k_out,
           updated_at=unixepoch() * 1000",
    )
    .bind(&provider.id)
    .bind(&provider.display_name)
    .bind(provider.enabled)
    .bind(&provider.api_base)
    .bind(&provider.key_alias)
    .bind(&provider.default_model)
    .bind(provider.timeout_ms)
    .bind(provider.cost_per_1k_in)
    .bind(provider.cost_per_1k_out)
    .execute(&state.db)
    .await?;
    Ok(())
}

/// Trigger a multi-LLM analysis for a market.
/// Stub: creates the analysis row + per-provider placeholder recommendations.
/// Real implementation (v0.2) will use the LLM client in commands/llm_client.rs
/// to fan out concurrent requests and populate recommendations.
#[tauri::command]
pub async fn llm_analyze(
    state: State<'_, AppState>,
    args: AnalyzeArgs,
) -> AppResult<LlmAnalysisDto> {
    let analysis_id = Uuid::new_v4().to_string();
    let requested_at = chrono::Utc::now().timestamp_millis();
    let prompt_version = args.prompt_version.unwrap_or_else(|| "v3.2".to_string());
    // take all owned values up front to avoid borrow conflicts
    let triggered_by = args.triggered_by.clone().unwrap_or_else(|| "user:anonymous".to_string());
    let market_id = args.market_id.clone();
    let signal_id = args.signal_id;
    let provider_ids_owned = args.provider_ids.clone();

    // determine which providers to call
    let _provider_filter = provider_ids_owned.as_ref().map(|ids| {
        let placeholders = vec!["?"; ids.len()].join(",");
        format!("id IN ({})", placeholders)
    });

    let providers: Vec<LlmProviderDto> = if let Some(ids) = &provider_ids_owned {
        let placeholders = vec!["?"; ids.len()].join(",");
        let q = format!(
            "SELECT id, display_name, enabled, api_base, key_alias, default_model, timeout_ms, cost_per_1k_in, cost_per_1k_out
             FROM llm_providers WHERE enabled = 1 AND id IN ({})",
            placeholders
        );
        let mut query = sqlx::query_as::<_, LlmProviderDto>(&q);
        for id in ids.iter() { query = query.bind(id); }
        query.fetch_all(&state.db).await?
    } else {
        sqlx::query_as::<_, LlmProviderDto>(
            "SELECT id, display_name, enabled, api_base, key_alias, default_model, timeout_ms, cost_per_1k_in, cost_per_1k_out
             FROM llm_providers WHERE enabled = 1 ORDER BY display_name",
        )
        .fetch_all(&state.db)
        .await?
    };

    // 1. insert analysis row (status: pending)
    sqlx::query(
        "INSERT INTO llm_analyses (id, market_id, signal_id, prompt_version, requested_at, status, triggered_by)
         VALUES (?, ?, ?, ?, ?, 'pending', ?)",
    )
    .bind(&analysis_id)
    .bind(&market_id)
    .bind(signal_id)
    .bind(&prompt_version)
    .bind(requested_at)
    .bind(&triggered_by)
    .execute(&state.db)
    .await?;

    // 2. fan out to each provider (placeholder: write parse_ok=0 stub rows)
    //    v0.2 implementation replaces this with actual LLM client calls
    //    via tokio::join! across providers
    for p in &providers {
        sqlx::query(
            "INSERT INTO llm_recommendations (analysis_id, provider_id, parse_ok, parse_error, created_at)
             VALUES (?, ?, 0, 'not yet wired (v0.2 placeholder)', unixepoch() * 1000)",
        )
        .bind(&analysis_id)
        .bind(&p.id)
        .execute(&state.db)
        .await?;
    }

    // 3. mark analysis as failed/partial for now
    sqlx::query(
        "UPDATE llm_analyses SET status = 'failed', completed_at = unixepoch() * 1000 WHERE id = ?",
    )
    .bind(&analysis_id)
    .execute(&state.db)
    .await?;

    // 4. audit
    sqlx::query(
        "INSERT INTO audit_log (actor, action, target, payload, result)
         VALUES ('user', 'llm.analyze', ?, ?, 'partial')",
    )
    .bind(&market_id)
    .bind(serde_json::json!({"providers_requested": providers.len(), "prompt_version": prompt_version}))
    .execute(&state.db)
    .await?;

    // 5. return DTO with empty recommendations (v0.2 wires real ones)
    Ok(LlmAnalysisDto {
        id: analysis_id,
        market_id,
        signal_id,
        prompt_version,
        requested_at,
        completed_at: None,
        status: "failed".to_string(),
        consensus_predicted: None,
        consensus_side: None,
        consensus_conf: None,
        total_latency_ms: None,
        cost_cents: None,
        triggered_by,
        recommendations: vec![],
    })
}

/// Aggregate per-provider performance (win rate / Brier / cost).
#[tauri::command]
pub async fn llm_performance(
    state: State<'_, AppState>,
    window_days: Option<i64>,
    _category: Option<String>,
) -> AppResult<Vec<LlmPerformanceRow>> {
    let window = window_days.unwrap_or(30);
    let cutoff = chrono::Utc::now().timestamp_millis() - window * 24 * 3600 * 1000;

    // For each provider, count its recommendations linked to settled bets
    // and compute win rate + Brier score.
    let rows = sqlx::query_as::<_, (String, String, i64, i64, Option<f64>, Option<f64>, Option<f64>, Option<f64>)>(
        "SELECT
            p.id,
            p.display_name,
            COUNT(r.id) as n_recommendations,
            COUNT(b.id) as n_evaluated,
            CAST(SUM(CASE WHEN b.status = 'won' AND r.side = b.side THEN 1 ELSE 0 END) AS REAL) / NULLIF(COUNT(b.id), 0) as win_rate,
            AVG((r.predicted_prob - CASE WHEN b.side = 'YES' THEN 1.0 ELSE 0.0 END) * (r.predicted_prob - CASE WHEN b.side = 'YES' THEN 1.0 ELSE 0.0 END)) as brier,
            AVG(r.confidence) as avg_confidence,
            SUM(r.cost_cents) as total_cost_cents
         FROM llm_providers p
         LEFT JOIN llm_recommendations r ON r.provider_id = p.id
         LEFT JOIN llm_decisions d ON d.followed_llm_id = r.id
         LEFT JOIN bets b ON b.id = d.bet_id AND b.settled_at IS NOT NULL AND b.settled_at >= ?
         GROUP BY p.id, p.display_name
         ORDER BY win_rate DESC NULLS LAST",
    )
    .bind(cutoff)
    .fetch_all(&state.db)
    .await?;

    let out: Vec<LlmPerformanceRow> = rows
        .into_iter()
        .map(|(id, name, n_rec, n_ev, wr, br, conf, cost)| LlmPerformanceRow {
            provider_id: id,
            provider_name: name,
            n_recommendations: n_rec,
            n_evaluated: n_ev,
            win_rate: wr.unwrap_or(0.0),
            brier: br.unwrap_or(0.0),
            avg_confidence: conf.unwrap_or(0.0),
            total_cost_cents: cost.unwrap_or(0.0),
        })
        .collect();
    Ok(out)
}

/// Record a user decision (follow / override / skip) on an analysis.
#[derive(Debug, Deserialize)]
pub struct RecordDecisionArgs {
    pub analysis_id: String,
    pub user_decision: String, // 'follow_top' | 'manual_yes' | 'manual_no' | 'skip' | 're_analyze'
    pub user_decided_side: Option<String>, // 'YES' | 'NO' | null
    pub followed_llm_id: Option<i64>,
    pub bet_id: Option<String>,
    pub context_snapshot: Option<String>, // JSON
}

#[tauri::command]
pub async fn record_llm_decision(
    state: State<'_, AppState>,
    args: RecordDecisionArgs,
) -> AppResult<i64> {
    let result = sqlx::query(
        "INSERT INTO llm_decisions (analysis_id, bet_id, user_decision, user_decided_side, followed_llm_id, decided_at, context_snapshot)
         VALUES (?, ?, ?, ?, ?, unixepoch() * 1000, ?)",
    )
    .bind(&args.analysis_id)
    .bind(&args.bet_id)
    .bind(&args.user_decision)
    .bind(&args.user_decided_side)
    .bind(args.followed_llm_id)
    .bind(&args.context_snapshot)
    .execute(&state.db)
    .await?;
    Ok(result.last_insert_rowid())
}

// helper trait so the line above compiles
// (removed — no longer needed after refactor)

// ---------- v0.2 — LLM Stats (per-LLM × per-category, etc.) ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmStatsCell {
    pub provider_id: String,
    pub provider_name: String,
    pub category: String,
    pub n_recommendations: i64,
    pub n_evaluated: i64,
    pub win_rate: Option<f64>, // null when n_evaluated < 5 (insufficient data)
    pub avg_pnl: Option<f64>,
    pub brier: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct StatsArgs {
    pub provider_id: Option<String>,
    pub category: Option<String>,
    pub window_days: Option<i64>,
}

#[tauri::command]
pub async fn llm_stats_heatmap(
    state: State<'_, AppState>,
    args: StatsArgs,
) -> AppResult<Vec<LlmStatsCell>> {
    let window = args.window_days.unwrap_or(30);
    let cutoff = chrono::Utc::now().timestamp_millis() - window * 24 * 3600 * 1000;

    // per (provider, category) win rate + avg P&L
    let mut q = String::from(
        "SELECT r.provider_id, p.display_name, m.category,
                COUNT(r.id) as n_recommendations,
                COUNT(b.id) as n_evaluated,
                CAST(SUM(CASE WHEN b.status = 'won' AND r.side = b.side THEN 1 ELSE 0 END) AS REAL)
                    / NULLIF(COUNT(b.id), 0) as win_rate,
                AVG(CAST(b.pnl AS REAL)) as avg_pnl,
                AVG((r.predicted_prob - CASE WHEN b.side = 'YES' THEN 1.0 ELSE 0.END)
                    * (r.predicted_prob - CASE WHEN b.side = 'YES' THEN 1.0 ELSE 0.END)) as brier
         FROM llm_recommendations r
         JOIN llm_providers p ON p.id = r.provider_id
         JOIN llm_decisions d ON d.followed_llm_id = r.id
         JOIN bets b ON b.id = d.bet_id AND b.settled_at IS NOT NULL AND b.settled_at >= ?
         JOIN markets m ON m.id = b.market_id
         WHERE 1=1",
    );
    if args.provider_id.is_some() { q.push_str(" AND r.provider_id = ?"); }
    if args.category.is_some() { q.push_str(" AND m.category = ?"); }
    q.push_str(" GROUP BY r.provider_id, p.display_name, m.category");

    let mut query = sqlx::query_as::<_, (String, String, String, i64, i64, Option<f64>, Option<f64>, Option<f64>)>(&q);
    query = query.bind(cutoff);
    if let Some(p) = &args.provider_id { query = query.bind(p); }
    if let Some(c) = &args.category { query = query.bind(c); }
    let rows = query.fetch_all(&state.db).await?;

    let out: Vec<LlmStatsCell> = rows.into_iter().map(|(pid, pname, cat, n_rec, n_ev, wr, pnl, brier)| {
        // hide win rate if insufficient data
        let win_rate = if n_ev < 5 { None } else { wr };
        LlmStatsCell {
            provider_id: pid,
            provider_name: pname,
            category: cat,
            n_recommendations: n_rec,
            n_evaluated: n_ev,
            win_rate,
            avg_pnl: pnl,
            brier,
        }
    }).collect();
    Ok(out)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmStatsScatterPoint {
    pub provider_id: String,
    pub provider_name: String,
    pub n_evaluated: i64,
    pub win_rate: f64,
    pub total_pnl: f64,
    pub avg_pnl: f64,
}

#[tauri::command]
pub async fn llm_stats_scatter(
    state: State<'_, AppState>,
    window_days: Option<i64>,
) -> AppResult<Vec<LlmStatsScatterPoint>> {
    let window = window_days.unwrap_or(30);
    let cutoff = chrono::Utc::now().timestamp_millis() - window * 24 * 3600 * 1000;

    let rows = sqlx::query_as::<_, (String, String, i64, Option<f64>, Option<f64>, Option<f64>)>(
        "SELECT r.provider_id, p.display_name,
                COUNT(b.id) as n_evaluated,
                CAST(SUM(CASE WHEN b.status = 'won' AND r.side = b.side THEN 1 ELSE 0 END) AS REAL)
                    / NULLIF(COUNT(b.id), 0) as win_rate,
                SUM(CAST(b.pnl AS REAL)) as total_pnl,
                AVG(CAST(b.pnl AS REAL)) as avg_pnl
         FROM llm_recommendations r
         JOIN llm_providers p ON p.id = r.provider_id
         JOIN llm_decisions d ON d.followed_llm_id = r.id
         JOIN bets b ON b.id = d.bet_id AND b.settled_at IS NOT NULL AND b.settled_at >= ?
         GROUP BY r.provider_id, p.display_name
         ORDER BY total_pnl DESC NULLS LAST",
    )
    .bind(cutoff)
    .fetch_all(&state.db)
    .await?;

    let out: Vec<LlmStatsScatterPoint> = rows.into_iter().map(|(pid, pname, n, wr, total, avg)| {
        LlmStatsScatterPoint {
            provider_id: pid,
            provider_name: pname,
            n_evaluated: n,
            win_rate: wr.unwrap_or(0.0),
            total_pnl: total.unwrap_or(0.0),
            avg_pnl: avg.unwrap_or(0.0),
        }
    }).collect();
    Ok(out)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmStatsTimeseriesPoint {
    pub provider_id: String,
    pub bucket: String, // ISO date 'YYYY-MM-DD'
    pub n_evaluated: i64,
    pub win_rate: f64,
    pub brier: f64,
}

#[tauri::command]
pub async fn llm_stats_timeseries(
    state: State<'_, AppState>,
    window_days: Option<i64>,
) -> AppResult<Vec<LlmStatsTimeseriesPoint>> {
    let window = window_days.unwrap_or(30);
    let cutoff = chrono::Utc::now().timestamp_millis() - window * 24 * 3600 * 1000;

    let rows = sqlx::query_as::<_, (String, String, i64, Option<f64>, Option<f64>)>(
        "SELECT r.provider_id,
                strftime('%Y-%m-%d', b.settled_at / 1000, 'unixepoch') as bucket,
                COUNT(b.id) as n_evaluated,
                CAST(SUM(CASE WHEN b.status = 'won' AND r.side = b.side THEN 1 ELSE 0 END) AS REAL)
                    / NULLIF(COUNT(b.id), 0) as win_rate,
                AVG((r.predicted_prob - CASE WHEN b.side = 'YES' THEN 1.0 ELSE 0.END)
                    * (r.predicted_prob - CASE WHEN b.side = 'YES' THEN 1.0 ELSE 0.END)) as brier
         FROM llm_recommendations r
         JOIN llm_decisions d ON d.followed_llm_id = r.id
         JOIN bets b ON b.id = d.bet_id AND b.settled_at IS NOT NULL AND b.settled_at >= ?
         GROUP BY r.provider_id, bucket
         ORDER BY bucket ASC",
    )
    .bind(cutoff)
    .fetch_all(&state.db)
    .await?;

    let out: Vec<LlmStatsTimeseriesPoint> = rows.into_iter().map(|(pid, bucket, n, wr, brier)| {
        LlmStatsTimeseriesPoint {
            provider_id: pid,
            bucket,
            n_evaluated: n,
            win_rate: wr.unwrap_or(0.0),
            brier: brier.unwrap_or(0.0),
        }
    }).collect();
    Ok(out)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmDecisionStats {
    pub category: String,
    pub decision_type: String, // 'follow_llm' | 'follow_consensus' | 'follow_specific:<provider>' | 'override' | 'manual'
    pub n: i64,
    pub win_rate: f64,
    pub avg_pnl: f64,
}

#[tauri::command]
pub async fn llm_stats_decision(
    state: State<'_, AppState>,
    window_days: Option<i64>,
) -> AppResult<Vec<LlmDecisionStats>> {
    let window = window_days.unwrap_or(30);
    let cutoff = chrono::Utc::now().timestamp_millis() - window * 24 * 3600 * 1000;

    // per decision_type (joined with followed LLM provider if any) win rate
    let rows = sqlx::query_as::<_, (String, String, i64, Option<f64>, Option<f64>)>(
        "SELECT
            CASE
                WHEN d.followed_llm_id IS NULL THEN 'manual'
                WHEN d.user_decision = 'override' THEN 'override'
                WHEN d.user_decided_side = r.side AND d.user_decision LIKE 'follow_%' THEN 'follow_llm'
                ELSE 'follow_other'
            END as decision_type,
            COALESCE(r.provider_id, 'none') as category,
            COUNT(b.id) as n,
            CAST(SUM(CASE WHEN b.status = 'won' THEN 1 ELSE 0 END) AS REAL) / NULLIF(COUNT(b.id), 0) as win_rate,
            AVG(CAST(b.pnl AS REAL)) as avg_pnl
         FROM llm_decisions d
         LEFT JOIN llm_recommendations r ON r.id = d.followed_llm_id
         JOIN bets b ON b.id = d.bet_id AND b.settled_at IS NOT NULL AND b.settled_at >= ?
         GROUP BY decision_type, category
         ORDER BY decision_type",
    )
    .bind(cutoff)
    .fetch_all(&state.db)
    .await?;

    let out: Vec<LlmDecisionStats> = rows.into_iter().map(|(dt, cat, n, wr, pnl)| {
        LlmDecisionStats {
            decision_type: dt,
            category: cat,
            n,
            win_rate: wr.unwrap_or(0.0),
            avg_pnl: pnl.unwrap_or(0.0),
        }
    }).collect();
    Ok(out)
}