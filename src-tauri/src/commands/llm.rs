use crate::AppResult;
use crate::llm_clients::{
    self, AnthropicClient, CustomClient, DeepSeekClient, GoogleClient, OpenAIClient, ProviderKind,
    MarketContext, OrderbookTop, PeerView, SignalSummary,
    PROMPT_VERSION_MARKET_ANALYSIS, build_market_analysis_request, parse_recommendation,
};
use crate::state::AppState;
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use std::sync::Arc;
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

// =================================================================
// ============= v0.2 — Real LLM fan-out ==========================
// =================================================================

// Process-wide shared HTTP client (connection pool reused across calls).
static HTTP: OnceCell<reqwest::Client> = OnceCell::new();
fn http_client() -> &'static reqwest::Client {
    HTTP.get_or_init(llm_clients::new_http_client)
}

/// Pick a client implementation for a provider_kind.
fn client_for(kind: ProviderKind, api_base: Option<&str>, model: &str) -> Arc<dyn llm_clients::LlmClient> {
    match kind {
        ProviderKind::Openai => Arc::new(OpenAIClient::new(
            api_base.unwrap_or("https://api.openai.com/v1"),
        )),
        ProviderKind::Anthropic => Arc::new(AnthropicClient::with_base(
            api_base.unwrap_or("https://api.anthropic.com"),
        )),
        ProviderKind::Google => Arc::new(GoogleClient::with_base(
            api_base.unwrap_or("https://generativelanguage.googleapis.com/v1beta"),
        )),
        ProviderKind::Deepseek => Arc::new(DeepSeekClient::new()),
        ProviderKind::OpenaiCompat | ProviderKind::AnthropicCompat => {
            let base = api_base.unwrap_or("https://api.openai.com/v1");
            if matches!(kind, ProviderKind::AnthropicCompat) {
                Arc::new(CustomClient::new_anthropic_compat(base, model))
            } else {
                Arc::new(CustomClient::new_openai_compat(base, model))
            }
        }
    }
}

/// Fetch a market row + last orderbook + last 3 active signals.
async fn build_market_context(
    state: &AppState,
    market_id: &str,
) -> AppResult<MarketContext> {
    let m: Option<(String, String, String, Option<f64>, Option<f64>, Option<f64>, i64, String)> =
        sqlx::query_as(
            "SELECT question, category, COALESCE(resolution_source,''), yes_price, no_price, volume_24h, closes_at, status
             FROM markets WHERE id = ?",
        )
        .bind(market_id)
        .fetch_optional(&state.db)
        .await?;
    let Some((question, category, resolution_source, yes_p, no_p, vol_24h, closes_at, _status)) = m else {
        return Err(crate::AppError::Invalid(format!("market not found: {market_id}")));
    };
    let ob: Option<(Option<i64>, Option<i64>, Option<f64>, Option<f64>)> = sqlx::query_as(
        "SELECT best_bid, best_ask, bid_depth, ask_depth
         FROM orderbook_snapshots WHERE market_id = ? ORDER BY captured_at DESC LIMIT 1",
    )
    .bind(market_id)
    .fetch_optional(&state.db)
    .await?;
    let orderbook_top = ob.and_then(|(b, a, bd, ad)| {
        match (b, a) {
            (Some(bb), Some(aa)) => Some(OrderbookTop {
                best_bid: bb as u32,
                best_ask: aa as u32,
                bid_depth: bd.unwrap_or(0.0),
                ask_depth: ad.unwrap_or(0.0),
            }),
            _ => None,
        }
    });
    let sigs: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT s.kind, s.summary, COALESCE(s.polarity,'neutral')
         FROM signals s
         WHERE s.market_id = ? AND s.active = 1
         ORDER BY s.computed_at DESC LIMIT 3",
    )
    .bind(market_id)
    .fetch_all(&state.db)
    .await?;
    let recent_signals = sigs.into_iter()
        .map(|(name, value, polarity)| SignalSummary { name, value, polarity })
        .collect();

    let yes_cents = (yes_p.unwrap_or(0.5) * 100.0).round() as u32;
    let no_cents  = (no_p.unwrap_or(0.5)  * 100.0).round() as u32;
    Ok(MarketContext {
        market_id: market_id.to_string(),
        question, category,
        yes_price_cents: yes_cents,
        no_price_cents: no_cents,
        volume_24h_usdc: vol_24h.unwrap_or(0.0),
        liquidity_usdc: 0.0,
        closes_at_unix_ms: closes_at,
        resolution_source,
        recent_signals,
        orderbook_top,
    })
}

/// Pick enabled keys for a provider ordered by priority ASC.
async fn pick_keys(state: &AppState, provider_id: &str) -> AppResult<Vec<llm_clients::KeyHandle>> {
    let rows: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT id, alias, keyring_alias
         FROM llm_provider_keys
         WHERE provider_id = ? AND enabled = 1
         ORDER BY priority ASC, created_at ASC",
    )
    .bind(provider_id)
    .fetch_all(&state.db)
    .await?;
    Ok(rows.into_iter()
        .map(|(id, alias, keyring_alias)| llm_clients::KeyHandle { id, alias, keyring_alias })
        .collect())
}

async fn insert_call_log(state: &AppState, log: &llm_clients::CallLog, called_at_ms: i64) -> AppResult<i64> {
    let res = sqlx::query(
        "INSERT INTO llm_call_logs (
            analysis_id, provider_id, key_id, called_at, latency_ms, tokens_in, tokens_out,
            cost_cents, http_status, success, error_code, error_message, prompt_version,
            predicted_prob, recommended_side, caller, retry_count
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&log.analysis_id)
    .bind(&log.provider_id)
    .bind(&log.key_id)
    .bind(called_at_ms)
    .bind(log.latency_ms as i64)
    .bind(log.tokens_in as i64)
    .bind(log.tokens_out as i64)
    .bind(log.cost_cents)
    .bind(log.http_status.map(|s| s as i64))
    .bind(log.success)
    .bind(&log.error_code)
    .bind(&log.error_message)
    .bind(&log.prompt_version)
    .bind(log.predicted_prob)
    .bind(&log.recommended_side)
    .bind(&log.caller)
    .bind(log.retry_count)
    .execute(&state.db)
    .await?;
    Ok(res.last_insert_rowid())
}

async fn insert_recommendation(
    state: &AppState,
    analysis_id: &str,
    provider_id: &str,
    pred_prob: Option<f64>,
    side: Option<&str>,
    confidence: Option<f64>,
    reasoning: Option<&str>,
    latency_ms: u64,
    tokens_in: u32,
    tokens_out: u32,
    cost_cents: f64,
    raw_response: Option<&str>,
    parse_ok: bool,
    parse_error: Option<&str>,
) -> AppResult<i64> {
    let res = sqlx::query(
        "INSERT INTO llm_recommendations (
            analysis_id, provider_id, predicted_prob, side, confidence, reasoning,
            latency_ms, tokens_in, tokens_out, cost_cents, raw_response, parse_ok, parse_error, created_at
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, unixepoch() * 1000)",
    )
    .bind(analysis_id)
    .bind(provider_id)
    .bind(pred_prob)
    .bind(side)
    .bind(confidence)
    .bind(reasoning)
    .bind(latency_ms as i64)
    .bind(tokens_in as i64)
    .bind(tokens_out as i64)
    .bind(cost_cents)
    .bind(raw_response)
    .bind(parse_ok)
    .bind(parse_error)
    .execute(&state.db)
    .await?;
    Ok(res.last_insert_rowid())
}

async fn update_provider_health_from_log(state: &AppState, log: &llm_clients::CallLog) -> AppResult<()> {
    let status = match (log.success, log.latency_ms) {
        (false, _) => "failing",
        (true, ms) if ms > 3_000 => "slow",
        (true, _) => "ok",
    };
    let last_err: Option<&str> = if log.success { None } else { log.error_message.as_deref() };
    sqlx::query(
        "UPDATE llm_providers SET
            health_status = ?,
            last_health_check_at = unixepoch() * 1000,
            last_health_error = COALESCE(?, last_health_error)
         WHERE id = ?",
    )
    .bind(status)
    .bind(last_err)
    .bind(&log.provider_id)
    .execute(&state.db)
    .await?;
    Ok(())
}

fn consensus_from(recs: &[(String, f64, f64, String)]) -> (f64, String, f64) {
    if recs.is_empty() { return (0.5, "skip".into(), 0.0); }
    let mut sorted: Vec<f64> = recs.iter().map(|r| r.1).collect();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median = if sorted.len() % 2 == 1 {
        sorted[sorted.len() / 2]
    } else {
        (sorted[sorted.len() / 2 - 1] + sorted[sorted.len() / 2]) / 2.0
    };
    let avg_conf: f64 = recs.iter().map(|r| r.2).sum::<f64>() / recs.len() as f64;
    let side = if median > 0.5 { "YES" } else { "NO" };
    (median.clamp(0.0, 1.0), side.into(), avg_conf.clamp(0.0, 1.0))
}

/// Resolve provider_kind for a known provider id, including aliases.
fn kind_for_provider_id(id: &str) -> ProviderKind {
    match id {
        "openai" => ProviderKind::Openai,
        "anthropic" => ProviderKind::Anthropic,
        "google" => ProviderKind::Google,
        "deepseek" => ProviderKind::Deepseek,
        "custom" | "openai_compat" => ProviderKind::OpenaiCompat,
        "anthropic_compat" => ProviderKind::AnthropicCompat,
        _ => ProviderKind::Openai, // safe default
    }
}

// ---------- the real llm_analyze (v0.2) ----------

#[tauri::command]
pub async fn llm_analyze(
    state: State<'_, AppState>,
    args: AnalyzeArgs,
) -> AppResult<LlmAnalysisDto> {
    let analysis_id = Uuid::new_v4().to_string();
    let requested_at = chrono::Utc::now().timestamp_millis();
    let prompt_version = args.prompt_version.clone().unwrap_or_else(|| PROMPT_VERSION_MARKET_ANALYSIS.to_string());
    let triggered_by = args.triggered_by.clone().unwrap_or_else(|| "user:anonymous".to_string());
    let market_id = args.market_id.clone();
    let signal_id = args.signal_id;
    let provider_filter = args.provider_ids.clone();

    // 1. Pull enabled providers
    let providers: Vec<LlmProviderDto> = if let Some(ids) = &provider_filter {
        if ids.is_empty() {
            return Err(crate::AppError::Invalid("provider_ids is empty".into()));
        }
        let placeholders = vec!["?"; ids.len()].join(",");
        let q = format!(
            "SELECT id, display_name, enabled, api_base, key_alias, default_model, timeout_ms, cost_per_1k_in, cost_per_1k_out
             FROM llm_providers WHERE enabled = 1 AND id IN ({placeholders})"
        );
        let mut query = sqlx::query_as::<_, LlmProviderDto>(&q);
        for id in ids.iter() { query = query.bind(id); }
        query.fetch_all(&state.db).await?
    } else {
        sqlx::query_as::<_, LlmProviderDto>(
            "SELECT id, display_name, enabled, api_base, key_alias, default_model, timeout_ms, cost_per_1k_in, cost_per_1k_out
             FROM llm_providers WHERE enabled = 1 ORDER BY display_name",
        )
        .fetch_all(&state.db).await?
    };

    if providers.is_empty() {
        return Err(crate::AppError::Invalid("no enabled providers".into()));
    }

    // 2. Insert analysis row (status: pending)
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

    // 3. Build context + pick keys (BEFORE the join — needs &state.db)
    let ctx = build_market_context(&state, &market_id).await?;
    let mut handles = Vec::new();
    for p in &providers {
        let keys = pick_keys(&state, &p.id).await?;
        let p_clone = p.clone();
        let ctx_clone = ctx.clone();
        let prompt_version_clone = prompt_version.clone();
        let analysis_id_clone = analysis_id.clone();
        handles.push(tokio::spawn(async move {
            let kind = kind_for_provider_id(&p_clone.id);
            let client = client_for(kind, p_clone.api_base.as_deref(), &p_clone.default_model);
            let cost = llm_clients::CostRate {
                per_1k_in_cents: p_clone.cost_per_1k_in.unwrap_or(0.0),
                per_1k_out_cents: p_clone.cost_per_1k_out.unwrap_or(0.0),
            };
            let policy = llm_clients::RetryPolicy::from_provider_row(2);
            let req = build_market_analysis_request(&p_clone.default_model, &ctx_clone);
            let outcome = llm_clients::dispatch(
                client.as_ref(),
                http_client(),
                &keys,
                &req, cost, policy,
                &p_clone.id,
                Some(&analysis_id_clone),
                Some(&prompt_version_clone),
                "m10.llm_analyze",
            ).await;
            (p_clone, outcome)
        }));
    }

    // 4. Await all in parallel — they're already running, just collect
    let mut recommendations: Vec<LlmRecommendationDto> = Vec::new();
    let mut total_latency: i64 = 0;
    let mut total_cost: f64 = 0.0;
    let mut consensus_inputs: Vec<(String, f64, f64, String)> = Vec::new();
    let mut any_success = false;

    for h in handles {
        let (p, outcome) = match h.await {
            Ok(x) => x,
            Err(e) => {
                tracing::error!("llm_analyze task join error: {e}");
                continue;
            }
        };

        let call_log_with_ts = outcome.log; // already-built; ts is now()
        let now_ms = chrono::Utc::now().timestamp_millis();
        // Persist call log
        let _ = insert_call_log(&state, &call_log_with_ts, now_ms).await?;
        // Update health
        let _ = update_provider_health_from_log(&state, &call_log_with_ts).await?;

        match outcome.outcome {
            Ok(oc) => {
                any_success = true;
                total_latency = total_latency.max(call_log_with_ts.latency_ms as i64);
                total_cost += call_log_with_ts.cost_cents;
                let raw = Some(oc.text.as_str());
                let (pred_prob, side, confidence, reasoning) = match parse_recommendation(&oc.text) {
                    Ok((p, s, c, r)) => (Some(p), Some(s), Some(c), Some(r)),
                    Err(e) => (None, None, None, Some(format!("[parse error] {e}"))),
                };
                let parse_ok = pred_prob.is_some();
                let parse_error_msg = if parse_ok { None } else { Some("model output not in expected JSON shape") };

                let rec_id = insert_recommendation(
                    &state, &analysis_id, &p.id,
                    pred_prob, side.as_deref(), confidence, reasoning.as_deref(),
                    call_log_with_ts.latency_ms, call_log_with_ts.tokens_in, call_log_with_ts.tokens_out,
                    call_log_with_ts.cost_cents, raw, parse_ok, parse_error_msg,
                ).await?;

                if let (Some(prob), Some(s), Some(conf)) = (pred_prob, side.clone(), confidence) {
                    consensus_inputs.push((p.id.clone(), prob, conf, s));
                }

                recommendations.push(LlmRecommendationDto {
                    id: rec_id,
                    analysis_id: analysis_id.clone(),
                    provider_id: p.id.clone(),
                    provider_name: p.display_name.clone(),
                    predicted_prob: pred_prob,
                    side: side,
                    confidence: confidence,
                    reasoning: reasoning,
                    latency_ms: Some(call_log_with_ts.latency_ms as i64),
                    tokens_in: Some(call_log_with_ts.tokens_in as i64),
                    tokens_out: Some(call_log_with_ts.tokens_out as i64),
                    cost_cents: Some(call_log_with_ts.cost_cents),
                    parse_ok,
                    parse_error: parse_error_msg.map(|s| s.to_string()),
                });
            }
            Err(_e) => {
                // still record a stub row so the UI can show the failure
                let rec_id = insert_recommendation(
                    &state, &analysis_id, &p.id,
                    None, None, None, None,
                    call_log_with_ts.latency_ms, 0, 0, 0.0, None, false,
                    call_log_with_ts.error_message.as_deref(),
                ).await?;
                recommendations.push(LlmRecommendationDto {
                    id: rec_id,
                    analysis_id: analysis_id.clone(),
                    provider_id: p.id.clone(),
                    provider_name: p.display_name.clone(),
                    predicted_prob: None,
                    side: None,
                    confidence: None,
                    reasoning: None,
                    latency_ms: Some(call_log_with_ts.latency_ms as i64),
                    tokens_in: Some(0),
                    tokens_out: Some(0),
                    cost_cents: Some(0.0),
                    parse_ok: false,
                    parse_error: call_log_with_ts.error_message.clone(),
                });
            }
        }
    }

    // 5. Compute consensus + finalize analysis row
    let (consensus_pred, consensus_side, consensus_conf) = consensus_from(&consensus_inputs);
    let (status_label, final_consensus) = if !any_success {
        ("failed".to_string(), (None, None, None))
    } else if recommendations.iter().all(|r| r.parse_ok) {
        ("completed".to_string(), (Some(consensus_pred), Some(consensus_side.clone()), Some(consensus_conf)))
    } else {
        ("partial".to_string(), (Some(consensus_pred), Some(consensus_side.clone()), Some(consensus_conf)))
    };
    let (fc_pred, fc_side, fc_conf) = final_consensus;

    sqlx::query(
        "UPDATE llm_analyses SET
            status = ?, completed_at = unixepoch() * 1000,
            consensus_predicted = ?, consensus_side = ?, consensus_conf = ?,
            total_latency_ms = ?, cost_cents = ?
         WHERE id = ?",
    )
    .bind(&status_label)
    .bind(fc_pred)
    .bind(fc_side.as_deref())
    .bind(fc_conf)
    .bind(total_latency)
    .bind(total_cost)
    .bind(&analysis_id)
    .execute(&state.db)
    .await?;

    // 6. audit
    sqlx::query(
        "INSERT INTO audit_log (actor, action, target, payload, result) VALUES ('user', 'llm.analyze', ?, ?, ?)",
    )
    .bind(&market_id)
    .bind(serde_json::json!({
        "analysis_id": analysis_id,
        "providers_requested": providers.len(),
        "providers_succeeded": recommendations.iter().filter(|r| r.parse_ok).count(),
        "prompt_version": prompt_version,
        "consensus": fc_pred,
    }))
    .bind(&status_label)
    .execute(&state.db)
    .await?;

    Ok(LlmAnalysisDto {
        id: analysis_id,
        market_id,
        signal_id,
        prompt_version,
        requested_at,
        completed_at: Some(chrono::Utc::now().timestamp_millis()),
        status: status_label,
        consensus_predicted: fc_pred,
        consensus_side: fc_side,
        consensus_conf: fc_conf,
        total_latency_ms: Some(total_latency),
        cost_cents: Some(total_cost),
        triggered_by,
        recommendations,
    })
}

// ---------- new IPC: get one recommendation by id ----------

#[tauri::command]
pub async fn llm_get_recommendation(
    state: State<'_, AppState>,
    rec_id: i64,
) -> AppResult<Option<LlmRecommendationDto>> {
    let row: Option<(i64, String, String, String, Option<f64>, Option<String>, Option<f64>, Option<String>, Option<i64>, Option<i64>, Option<i64>, Option<f64>, Option<String>, bool, Option<String>)> = sqlx::query_as(
        "SELECT r.id, r.analysis_id, r.provider_id, p.display_name,
                r.predicted_prob, r.side, r.confidence, r.reasoning,
                r.latency_ms, r.tokens_in, r.tokens_out, r.cost_cents,
                r.raw_response, r.parse_ok, r.parse_error
         FROM llm_recommendations r
         JOIN llm_providers p ON p.id = r.provider_id
         WHERE r.id = ?",
    )
    .bind(rec_id)
    .fetch_optional(&state.db)
    .await?;
    Ok(row.map(|(id, analysis_id, provider_id, provider_name, predicted_prob, side, confidence, reasoning, latency_ms, tokens_in, tokens_out, cost_cents, _raw, parse_ok, parse_error)| {
        LlmRecommendationDto {
            id, analysis_id, provider_id, provider_name,
            predicted_prob, side, confidence, reasoning,
            latency_ms, tokens_in, tokens_out, cost_cents, parse_ok, parse_error,
        }
    }))
}

// ---------- new IPC: list recent analyses (per market or all) ----------

#[derive(Debug, Deserialize)]
pub struct ListAnalysesArgs {
    pub market_id: Option<String>,
    pub limit: Option<i64>,
}

#[tauri::command]
pub async fn llm_list_analyses(
    state: State<'_, AppState>,
    args: ListAnalysesArgs,
) -> AppResult<Vec<LlmAnalysisDto>> {
    let limit = args.limit.unwrap_or(20).clamp(1, 200);
    let rows: Vec<(String, String, Option<i64>, String, i64, Option<i64>, String, Option<f64>, Option<String>, Option<f64>, Option<i64>, Option<f64>, String)> = if let Some(mid) = &args.market_id {
        sqlx::query_as(
            "SELECT id, market_id, signal_id, prompt_version, requested_at, completed_at, status,
                    consensus_predicted, consensus_side, consensus_conf, total_latency_ms, cost_cents, triggered_by
             FROM llm_analyses WHERE market_id = ? ORDER BY requested_at DESC LIMIT ?",
        )
        .bind(mid)
        .bind(limit)
        .fetch_all(&state.db)
        .await?
    } else {
        sqlx::query_as(
            "SELECT id, market_id, signal_id, prompt_version, requested_at, completed_at, status,
                    consensus_predicted, consensus_side, consensus_conf, total_latency_ms, cost_cents, triggered_by
             FROM llm_analyses ORDER BY requested_at DESC LIMIT ?",
        )
        .bind(limit)
        .fetch_all(&state.db)
        .await?
    };

    let mut out: Vec<LlmAnalysisDto> = Vec::with_capacity(rows.len());
    for r in rows {
        out.push(LlmAnalysisDto {
            id: r.0,
            market_id: r.1,
            signal_id: r.2,
            prompt_version: r.3,
            requested_at: r.4,
            completed_at: r.5,
            status: r.6,
            consensus_predicted: r.7,
            consensus_side: r.8,
            consensus_conf: r.9,
            total_latency_ms: r.10,
            cost_cents: r.11,
            triggered_by: r.12,
            recommendations: vec![], // detail load on click
        });
    }
    Ok(out)
}