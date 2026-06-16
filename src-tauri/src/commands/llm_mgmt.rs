//! M11 — LLM Management commands
//!
//! Spec: polyrocket-llm-management.md
//! Provides provider CRUD, key rotation, connectivity test, traffic
//! aggregation, and stats queries.

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
    pub provider_kind: String,
    pub request_format: String,
    pub supports_streaming: bool,
    pub enabled: bool,
    pub api_base: Option<String>,
    pub key_alias: String,
    pub default_model: String,
    pub timeout_ms: i64,
    pub request_timeout_ms: i64,
    pub max_retries: i64,
    pub cost_per_1k_in: Option<f64>,
    pub cost_per_1k_out: Option<f64>,
    pub rate_limit_rpm: Option<i64>,
    pub rate_limit_tpm: Option<i64>,
    pub quota_daily_cents: Option<f64>,
    pub quota_monthly_cents: Option<f64>,
    pub key_rotation_strategy: String,
    pub health_status: String,
    pub health_latency_p50_ms: Option<i64>,
    pub health_latency_p95_ms: Option<i64>,
    pub last_health_check_at: Option<i64>,
    pub last_health_error: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct LlmProviderKeyDto {
    pub id: String,
    pub provider_id: String,
    pub alias: String,
    pub keyring_alias: String,
    pub enabled: bool,
    pub priority: i64,
    pub weight: i64,
    pub last_used_at: Option<i64>,
    pub last_error: Option<String>,
    pub last_error_at: Option<i64>,
    pub total_calls: i64,
    pub total_errors: i64,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmTrafficSummary {
    pub provider_id: String,
    pub window: String,
    pub calls_total: i64,
    pub calls_success: i64,
    pub calls_failed: i64,
    pub success_rate: f64,
    pub avg_latency_ms: f64,
    pub p95_latency_ms: f64,
    pub total_tokens_in: i64,
    pub total_tokens_out: i64,
    pub total_cost_cents: f64,
    pub rate_limit_hits: i64,
    pub delta_calls_last_window: i64,
    pub delta_cost_last_window: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectivityTestResult {
    pub provider_id: String,
    pub success: bool,
    pub latency_ms: Option<i64>,
    pub http_status: Option<i64>,
    pub model_used: String,
    pub error_code: Option<String>, // 'auth' | 'rate_limit' | 'timeout' | 'network' | 'parse'
    pub error_message: Option<String>,
    pub checked_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct LlmHealthCheckDto {
    pub id: i64,
    pub provider_id: String,
    pub checked_at: i64,
    pub trigger: String,
    pub success: bool,
    pub latency_ms: Option<i64>,
    pub http_status: Option<i64>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmStatsConfidenceBand {
    pub provider_id: String,
    pub band: String, // '<40%' | '40-50%' | ... | '>80%'
    pub n: i64,
    pub win_rate: f64,
    pub avg_pnl: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmStatsCostEfficiency {
    pub provider_id: String,
    pub total_cost_cents: f64,
    pub total_pnl: f64,
    pub efficiency: f64, // pnl per cent
    pub win_rate: f64,
}

// ---------- Provider CRUD ----------

#[tauri::command]
pub async fn llm_provider_list(state: State<'_, AppState>) -> AppResult<Vec<LlmProviderDto>> {
    let rows = sqlx::query_as::<_, LlmProviderDto>(
        "SELECT id, display_name, provider_kind, request_format, supports_streaming, enabled,
                api_base, key_alias, default_model, timeout_ms, request_timeout_ms, max_retries,
                cost_per_1k_in, cost_per_1k_out, rate_limit_rpm, rate_limit_tpm,
                quota_daily_cents, quota_monthly_cents, key_rotation_strategy,
                health_status, health_latency_p50_ms, health_latency_p95_ms,
                last_health_check_at, last_health_error, notes
         FROM llm_providers ORDER BY display_name",
    )
    .fetch_all(&state.db)
    .await?;
    Ok(rows)
}

#[tauri::command]
pub async fn llm_provider_upsert(
    state: State<'_, AppState>,
    provider: LlmProviderDto,
) -> AppResult<()> {
    let now = chrono::Utc::now().timestamp_millis();
    sqlx::query(
        "INSERT INTO llm_providers (
            id, display_name, provider_kind, request_format, supports_streaming, enabled,
            api_base, key_alias, default_model, timeout_ms, request_timeout_ms, max_retries,
            cost_per_1k_in, cost_per_1k_out, rate_limit_rpm, rate_limit_tpm,
            quota_daily_cents, quota_monthly_cents, key_rotation_strategy,
            health_status, health_latency_p50_ms, health_latency_p95_ms,
            last_health_check_at, last_health_error, notes, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(id) DO UPDATE SET
           display_name=excluded.display_name,
           provider_kind=excluded.provider_kind,
           request_format=excluded.request_format,
           supports_streaming=excluded.supports_streaming,
           enabled=excluded.enabled,
           api_base=excluded.api_base,
           key_alias=excluded.key_alias,
           default_model=excluded.default_model,
           timeout_ms=excluded.timeout_ms,
           request_timeout_ms=excluded.request_timeout_ms,
           max_retries=excluded.max_retries,
           cost_per_1k_in=excluded.cost_per_1k_in,
           cost_per_1k_out=excluded.cost_per_1k_out,
           rate_limit_rpm=excluded.rate_limit_rpm,
           rate_limit_tpm=excluded.rate_limit_tpm,
           quota_daily_cents=excluded.quota_daily_cents,
           quota_monthly_cents=excluded.quota_monthly_cents,
           key_rotation_strategy=excluded.key_rotation_strategy,
           notes=excluded.notes,
           updated_at=?",
    )
    .bind(&provider.id)
    .bind(&provider.display_name)
    .bind(&provider.provider_kind)
    .bind(&provider.request_format)
    .bind(provider.supports_streaming)
    .bind(provider.enabled)
    .bind(&provider.api_base)
    .bind(&provider.key_alias)
    .bind(&provider.default_model)
    .bind(provider.timeout_ms)
    .bind(provider.request_timeout_ms)
    .bind(provider.max_retries)
    .bind(provider.cost_per_1k_in)
    .bind(provider.cost_per_1k_out)
    .bind(provider.rate_limit_rpm)
    .bind(provider.rate_limit_tpm)
    .bind(provider.quota_daily_cents)
    .bind(provider.quota_monthly_cents)
    .bind(&provider.key_rotation_strategy)
    .bind(&provider.health_status)
    .bind(provider.health_latency_p50_ms)
    .bind(provider.health_latency_p95_ms)
    .bind(provider.last_health_check_at)
    .bind(&provider.last_health_error)
    .bind(&provider.notes)
    .bind(now)
    .bind(now)
    .execute(&state.db)
    .await?;

    sqlx::query(
        "INSERT INTO audit_log (actor, action, target, payload, result) VALUES ('user', 'llm.provider.upsert', ?, ?, 'ok')",
    )
    .bind(&provider.id)
    .bind(serde_json::json!({"enabled": provider.enabled, "kind": provider.provider_kind}))
    .execute(&state.db)
    .await?;
    Ok(())
}

#[tauri::command]
pub async fn llm_provider_delete(state: State<'_, AppState>, provider_id: String) -> AppResult<()> {
    let usages: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM llm_recommendations WHERE provider_id = ?",
    )
    .bind(&provider_id)
    .fetch_one(&state.db)
    .await?;
    if usages > 0 {
        sqlx::query(
            "INSERT INTO audit_log (actor, action, target, payload, result) VALUES ('user', 'llm.provider.delete', ?, ?, 'rejected')",
        )
        .bind(&provider_id)
        .bind(serde_json::json!({"reason": "in_use", "n_uses": usages}))
        .execute(&state.db)
        .await?;
        return Err(crate::AppError::Invalid(format!(
            "Provider in use by {} recommendations, delete refused", usages
        )));
    }
    sqlx::query("DELETE FROM llm_provider_keys WHERE provider_id = ?")
        .bind(&provider_id)
        .execute(&state.db)
        .await?;
    sqlx::query("DELETE FROM llm_providers WHERE id = ?")
        .bind(&provider_id)
        .execute(&state.db)
        .await?;
    sqlx::query(
        "INSERT INTO audit_log (actor, action, target, result) VALUES ('user', 'llm.provider.delete', ?, 'ok')",
    )
    .bind(&provider_id)
    .execute(&state.db)
    .await?;
    Ok(())
}

// ---------- Keys CRUD ----------

#[tauri::command]
pub async fn llm_key_list(
    state: State<'_, AppState>,
    provider_id: Option<String>,
) -> AppResult<Vec<LlmProviderKeyDto>> {
    let rows = if let Some(pid) = provider_id {
        sqlx::query_as::<_, LlmProviderKeyDto>(
            "SELECT id, provider_id, alias, keyring_alias, enabled, priority, weight,
                    last_used_at, last_error, last_error_at, total_calls, total_errors, notes
             FROM llm_provider_keys WHERE provider_id = ? ORDER BY priority ASC",
        )
        .bind(pid)
        .fetch_all(&state.db)
        .await?
    } else {
        sqlx::query_as::<_, LlmProviderKeyDto>(
            "SELECT id, provider_id, alias, keyring_alias, enabled, priority, weight,
                    last_used_at, last_error, last_error_at, total_calls, total_errors, notes
             FROM llm_provider_keys ORDER BY provider_id, priority ASC",
        )
        .fetch_all(&state.db)
        .await?
    };
    Ok(rows)
}

#[tauri::command]
pub async fn llm_key_upsert(
    state: State<'_, AppState>,
    key: LlmProviderKeyDto,
) -> AppResult<()> {
    let now = chrono::Utc::now().timestamp_millis();
    sqlx::query(
        "INSERT INTO llm_provider_keys (
            id, provider_id, alias, keyring_alias, enabled, priority, weight,
            last_used_at, last_error, last_error_at, total_calls, total_errors, notes,
            created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(id) DO UPDATE SET
           alias=excluded.alias,
           keyring_alias=excluded.keyring_alias,
           enabled=excluded.enabled,
           priority=excluded.priority,
           weight=excluded.weight,
           notes=excluded.notes,
           updated_at=?",
    )
    .bind(&key.id)
    .bind(&key.provider_id)
    .bind(&key.alias)
    .bind(&key.keyring_alias)
    .bind(key.enabled)
    .bind(key.priority)
    .bind(key.weight)
    .bind(key.last_used_at)
    .bind(&key.last_error)
    .bind(key.last_error_at)
    .bind(key.total_calls)
    .bind(key.total_errors)
    .bind(&key.notes)
    .bind(now)
    .bind(now)
    .bind(now)
    .execute(&state.db)
    .await?;

    sqlx::query(
        "INSERT INTO audit_log (actor, action, target, payload, result) VALUES ('user', 'llm.key.upsert', ?, ?, 'ok')",
    )
    .bind(&key.id)
    .bind(serde_json::json!({"provider_id": key.provider_id, "alias": key.alias}))
    .execute(&state.db)
    .await?;
    Ok(())
}

#[tauri::command]
pub async fn llm_key_delete(state: State<'_, AppState>, key_id: String) -> AppResult<()> {
    sqlx::query("DELETE FROM llm_provider_keys WHERE id = ?")
        .bind(&key_id)
        .execute(&state.db)
        .await?;
    sqlx::query(
        "INSERT INTO audit_log (actor, action, target, result) VALUES ('user', 'llm.key.delete', ?, 'ok')",
    )
    .bind(&key_id)
    .execute(&state.db)
    .await?;
    Ok(())
}

// ---------- Connectivity test ----------

#[derive(Debug, Deserialize)]
pub struct TestConnectivityArgs {
    pub provider_id: String,
    pub key_id: Option<String>,
}

#[tauri::command]
pub async fn llm_test_connectivity(
    state: State<'_, AppState>,
    args: TestConnectivityArgs,
) -> AppResult<ConnectivityTestResult> {
    let provider: LlmProviderDto = sqlx::query_as::<_, LlmProviderDto>(
        "SELECT id, display_name, provider_kind, request_format, supports_streaming, enabled,
                api_base, key_alias, default_model, timeout_ms, request_timeout_ms, max_retries,
                cost_per_1k_in, cost_per_1k_out, rate_limit_rpm, rate_limit_tpm,
                quota_daily_cents, quota_monthly_cents, key_rotation_strategy,
                health_status, health_latency_p50_ms, health_latency_p95_ms,
                last_health_check_at, last_health_error, notes
         FROM llm_providers WHERE id = ?",
    )
    .bind(&args.provider_id)
    .fetch_one(&state.db)
    .await?;

    let key_alias = if let Some(kid) = &args.key_id {
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT keyring_alias FROM llm_provider_keys WHERE id = ? AND enabled = 1",
        )
        .bind(kid)
        .fetch_optional(&state.db)
        .await?;
        row.map(|(a,)| a).unwrap_or_else(|| provider.key_alias.clone())
    } else {
        provider.key_alias.clone()
    };

    // Fetch the actual key from keyring
    let secret = match crate::keyring::get_key(&key_alias) {
        Ok(s) => s,
        Err(e) => {
            let res = ConnectivityTestResult {
                provider_id: provider.id.clone(),
                success: false,
                latency_ms: None,
                http_status: None,
                model_used: provider.default_model.clone(),
                error_code: Some("auth".into()),
                error_message: Some(format!("keyring error: {e}")),
                checked_at: chrono::Utc::now().timestamp_millis(),
            };
            log_health_check(&state, &provider, &args.key_id, false, None, None, Some("auth".to_string()), Some(format!("{e}"))).await?;
            return Ok(res);
        }
    };

    // Pick client based on provider_kind
    let url = build_test_url(&provider);
    let body = build_test_body(&provider);
    let started = std::time::Instant::now();

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(provider.request_timeout_ms as u64))
        .build()
        .map_err(|e| crate::AppError::Internal(format!("http client: {e}")))?;
    let req = client.post(&url).bearer_auth(&secret).json(&body);
    let resp = match req.send().await {
        Ok(r) => r,
        Err(e) if e.is_timeout() => {
            let res = ConnectivityTestResult {
                provider_id: provider.id.clone(),
                success: false,
                latency_ms: Some(started.elapsed().as_millis() as i64),
                http_status: None,
                model_used: provider.default_model.clone(),
                error_code: Some("timeout".into()),
                error_message: Some(e.to_string()),
                checked_at: chrono::Utc::now().timestamp_millis(),
            };
            log_health_check(&state, &provider, &args.key_id, false, Some(started.elapsed().as_millis() as i64), None, Some("timeout".to_string()), Some(e.to_string())).await?;
            return Ok(res);
        }
        Err(e) => {
            let res = ConnectivityTestResult {
                provider_id: provider.id.clone(),
                success: false,
                latency_ms: Some(started.elapsed().as_millis() as i64),
                http_status: None,
                model_used: provider.default_model.clone(),
                error_code: Some("network".into()),
                error_message: Some(e.to_string()),
                checked_at: chrono::Utc::now().timestamp_millis(),
            };
            log_health_check(&state, &provider, &args.key_id, false, Some(started.elapsed().as_millis() as i64), None, Some("network".to_string()), Some(e.to_string())).await?;
            return Ok(res);
        }
    };

    let status = resp.status().as_u16() as i64;
    let latency = started.elapsed().as_millis() as i64;
    let body_text = resp.text().await.unwrap_or_default();
    let (ok, err_code, err_msg): (bool, Option<String>, Option<String>) = if (200..300).contains(&(status as u32)) {
        (true, None, None)
    } else if status == 401 || status == 403 {
        (false, Some("auth".to_string()), Some(format!("HTTP {status}")))
    } else if status == 429 {
        (false, Some("rate_limit".to_string()), Some(format!("HTTP {status}")))
    } else if (500..600).contains(&(status as u32)) {
        (false, Some("network".to_string()), Some(format!("HTTP {status}: {body_text}")))
    } else {
        (false, Some("parse".to_string()), Some(format!("HTTP {status}: {body_text}")))
    };

    let res = ConnectivityTestResult {
        provider_id: provider.id.clone(),
        success: ok,
        latency_ms: Some(latency),
        http_status: Some(status),
        model_used: provider.default_model.clone(),
        error_code: err_code.clone(),
        error_message: err_msg.clone(),
        checked_at: chrono::Utc::now().timestamp_millis(),
    };

    // persist to health_checks + update provider health
    log_health_check(&state, &provider, &args.key_id, ok, Some(latency), Some(status), err_code.clone(), err_msg.clone()).await?;
    update_provider_health(&state, &provider, ok, latency).await?;

    Ok(res)
}

fn build_test_url(p: &LlmProviderDto) -> String {
    let base = p.api_base.clone().unwrap_or_else(|| match p.provider_kind.as_str() {
        "openai" | "openai_compat" => "https://api.openai.com/v1".to_string(),
        "anthropic" | "anthropic_compat" => "https://api.anthropic.com/v1".to_string(),
        "google" => "https://generativelanguage.googleapis.com/v1beta".to_string(),
        "deepseek" => "https://api.deepseek.com/v1".to_string(),
        _ => "https://api.openai.com/v1".to_string(),
    });
    match p.provider_kind.as_str() {
        "google" => format!("{}/models/{}:generateContent", base.trim_end_matches('/'), p.default_model),
        _ => format!("{}/chat/completions", base.trim_end_matches('/')),
    }
}

fn build_test_body(p: &LlmProviderDto) -> serde_json::Value {
    match p.provider_kind.as_str() {
        "anthropic" | "anthropic_compat" => serde_json::json!({
            "model": p.default_model,
            "max_tokens": 1,
            "messages": [{"role": "user", "content": "ping"}]
        }),
        "google" => serde_json::json!({
            "contents": [{"role": "user", "parts": [{"text": "ping"}]}],
            "generationConfig": {"maxOutputTokens": 1}
        }),
        _ => serde_json::json!({
            "model": p.default_model,
            "max_tokens": 1,
            "messages": [{"role": "user", "content": "ping"}]
        }),
    }
}

async fn log_health_check(
    state: &State<'_, AppState>,
    p: &LlmProviderDto,
    key_id: &Option<String>,
    success: bool,
    latency_ms: Option<i64>,
    http_status: Option<i64>,
    error_code: Option<String>,
    error_message: Option<String>,
) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO llm_health_checks (provider_id, key_id, checked_at, trigger, success, latency_ms, http_status, error_code, error_message, model_used)
         VALUES (?, ?, unixepoch() * 1000, 'user', ?, ?, ?, ?, ?, ?)",
    )
    .bind(&p.id)
    .bind(key_id)
    .bind(success)
    .bind(latency_ms)
    .bind(http_status)
    .bind(error_code)
    .bind(error_message)
    .bind(&p.default_model)
    .execute(&state.db)
    .await?;
    Ok(())
}

async fn update_provider_health(
    state: &State<'_, AppState>,
    p: &LlmProviderDto,
    success: bool,
    latency_ms: i64,
) -> AppResult<()> {
    let status = if !success { "failing" } else if latency_ms > 3000 { "slow" } else { "ok" };
    let err_msg: String = if success { String::new() } else { "test failed".to_string() };
    sqlx::query(
        "UPDATE llm_providers SET health_status = ?, last_health_check_at = unixepoch() * 1000,
                                  last_health_error = CASE WHEN ? = 0 THEN last_health_error ELSE ? END
         WHERE id = ?",
    )
    .bind(status)
    .bind(if success { 1 } else { 0 })
    .bind(err_msg)
    .bind(&p.id)
    .execute(&state.db)
    .await?;
    Ok(())
}

// ---------- Traffic aggregation ----------

#[derive(Debug, Deserialize)]
pub struct TrafficArgs {
    pub provider_id: Option<String>,
    pub window: Option<String>, // '1h' | '24h' | '30d'
}

#[tauri::command]
pub async fn llm_traffic_summary(
    state: State<'_, AppState>,
    args: TrafficArgs,
) -> AppResult<Vec<LlmTrafficSummary>> {
    let window = args.window.as_deref().unwrap_or("24h");
    let cutoff = window_cutoff(window);

    let rows = sqlx::query_as::<_, (String, i64, i64, i64, Option<f64>, Option<f64>, Option<i64>, Option<i64>, Option<i64>)>(
        "SELECT provider_id,
                COUNT(*) as calls_total,
                SUM(CASE WHEN success = 1 THEN 1 ELSE 0 END) as calls_success,
                SUM(CASE WHEN success = 0 THEN 1 ELSE 0 END) as calls_failed,
                AVG(latency_ms) as avg_latency,
                SUM(cost_cents) as total_cost,
                SUM(tokens_in) as tokens_in,
                SUM(tokens_out) as tokens_out,
                SUM(CASE WHEN error_code = 'rate_limit' OR http_status = 429 THEN 1 ELSE 0 END) as rl_hits
         FROM llm_call_logs
         WHERE called_at >= ?
         GROUP BY provider_id
         ORDER BY provider_id",
    )
    .bind(cutoff)
    .fetch_all(&state.db)
    .await?;

    let previous_cutoff = previous_window_cutoff(window);
    let prev = sqlx::query_as::<_, (String, i64, Option<f64>)>(
        "SELECT provider_id, COUNT(*), SUM(cost_cents)
         FROM llm_call_logs
         WHERE called_at >= ? AND called_at < ?
         GROUP BY provider_id",
    )
    .bind(previous_cutoff)
    .bind(cutoff)
    .fetch_all(&state.db)
    .await?;
    let prev_map: std::collections::HashMap<String, (i64, f64)> = prev
        .into_iter()
        .map(|(p, c, co)| (p, (c, co.unwrap_or(0.0))))
        .collect();

    let out: Vec<LlmTrafficSummary> = rows
        .into_iter()
        .map(|(pid, total, success, failed, avg_lat, total_cost, tok_in, tok_out, rl_hits)| {
            let success_rate = if total > 0 { success as f64 / total as f64 } else { 0.0 };
            let avg_lat = avg_lat.unwrap_or(0.0);
            // p95 is a coarse approximation from avg (v0.3: real percentile via window function)
            let p95 = avg_lat * 1.6;
            let prev_entry = prev_map.get(&pid).cloned().unwrap_or((0, 0.0));
            LlmTrafficSummary {
                provider_id: pid,
                window: window.to_string(),
                calls_total: total,
                calls_success: success,
                calls_failed: failed,
                success_rate,
                avg_latency_ms: avg_lat,
                p95_latency_ms: p95,
                total_tokens_in: tok_in.unwrap_or(0),
                total_tokens_out: tok_out.unwrap_or(0),
                total_cost_cents: total_cost.unwrap_or(0.0),
                rate_limit_hits: rl_hits.unwrap_or(0),
                delta_calls_last_window: total - prev_entry.0,
                delta_cost_last_window: total_cost.unwrap_or(0.0) - prev_entry.1,
            }
        })
        .collect();
    Ok(out)
}

fn window_cutoff(window: &str) -> i64 {
    let now = chrono::Utc::now().timestamp_millis();
    let ms = match window {
        "1h" => 3600 * 1000,
        "24h" => 24 * 3600 * 1000,
        "30d" => 30 * 24 * 3600 * 1000,
        _ => 24 * 3600 * 1000,
    };
    now - ms
}

fn previous_window_cutoff(window: &str) -> i64 {
    let now = chrono::Utc::now().timestamp_millis();
    let ms = match window {
        "1h" => 2 * 3600 * 1000,
        "24h" => 2 * 24 * 3600 * 1000,
        "30d" => 60 * 24 * 3600 * 1000,
        _ => 2 * 24 * 3600 * 1000,
    };
    now - ms
}

// ---------- Health history ----------

#[tauri::command]
pub async fn llm_health_history(
    state: State<'_, AppState>,
    provider_id: String,
    limit: Option<i64>,
) -> AppResult<Vec<LlmHealthCheckDto>> {
    let rows = sqlx::query_as::<_, LlmHealthCheckDto>(
        "SELECT id, provider_id, checked_at, trigger, success, latency_ms, http_status, error_code, error_message
         FROM llm_health_checks WHERE provider_id = ?
         ORDER BY checked_at DESC LIMIT ?",
    )
    .bind(&provider_id)
    .bind(limit.unwrap_or(100))
    .fetch_all(&state.db)
    .await?;
    Ok(rows)
}

// ---------- Stats extensions (M11) ----------

#[derive(Debug, Deserialize)]
pub struct StatsByConfidenceArgs {
    pub provider_id: Option<String>,
    pub window_days: Option<i64>,
}

#[tauri::command]
pub async fn llm_stats_by_confidence(
    state: State<'_, AppState>,
    args: StatsByConfidenceArgs,
) -> AppResult<Vec<LlmStatsConfidenceBand>> {
    let window = args.window_days.unwrap_or(30);
    let cutoff = chrono::Utc::now().timestamp_millis() - window * 24 * 3600 * 1000;

    let rows = sqlx::query_as::<_, (String, f64, i64, Option<f64>, Option<f64>)>(
        "SELECT r.provider_id, r.confidence,
                COUNT(b.id) as n,
                CAST(SUM(CASE WHEN b.status = 'won' AND r.side = b.side THEN 1 ELSE 0 END) AS REAL)
                    / NULLIF(COUNT(b.id), 0) as win_rate,
                AVG(CAST(b.pnl AS REAL)) as avg_pnl
         FROM llm_recommendations r
         JOIN llm_decisions d ON d.followed_llm_id = r.id
         JOIN bets b ON b.id = d.bet_id AND b.settled_at IS NOT NULL AND b.settled_at >= ?
         WHERE r.confidence IS NOT NULL
         GROUP BY r.provider_id, r.confidence",
    )
    .bind(cutoff)
    .fetch_all(&state.db)
    .await?;

    let out: Vec<LlmStatsConfidenceBand> = rows
        .into_iter()
        .filter_map(|(pid, conf, n, wr, pnl)| {
            let band = confidence_band(conf);
            if n < 3 { return None; } // require min sample
            Some(LlmStatsConfidenceBand {
                provider_id: pid,
                band,
                n,
                win_rate: wr.unwrap_or(0.0),
                avg_pnl: pnl.unwrap_or(0.0),
            })
        })
        .collect();
    Ok(out)
}

fn confidence_band(c: f64) -> String {
    let pct = c * 100.0;
    if pct < 40.0 { "<40%".to_string() }
    else if pct < 50.0 { "40-50%".to_string() }
    else if pct < 60.0 { "50-60%".to_string() }
    else if pct < 70.0 { "60-70%".to_string() }
    else if pct < 80.0 { "70-80%".to_string() }
    else { ">80%".to_string() }
}

#[derive(Debug, Deserialize)]
pub struct StatsByPromptArgs {
    pub prompt_version: String,
    pub window_days: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmStatsByPrompt {
    pub provider_id: String,
    pub prompt_version: String,
    pub n_recommendations: i64,
    pub n_evaluated: i64,
    pub win_rate: f64,
    pub brier: f64,
}

#[tauri::command]
pub async fn llm_stats_by_prompt(
    state: State<'_, AppState>,
    args: StatsByPromptArgs,
) -> AppResult<Vec<LlmStatsByPrompt>> {
    let window = args.window_days.unwrap_or(30);
    let cutoff = chrono::Utc::now().timestamp_millis() - window * 24 * 3600 * 1000;

    let rows = sqlx::query_as::<_, (String, String, i64, i64, Option<f64>, Option<f64>)>(
        "SELECT a.prompt_version, r.provider_id,
                COUNT(r.id) as n_recs,
                COUNT(b.id) as n_ev,
                CAST(SUM(CASE WHEN b.status = 'won' AND r.side = b.side THEN 1 ELSE 0 END) AS REAL)
                    / NULLIF(COUNT(b.id), 0) as win_rate,
                AVG((r.predicted_prob - CASE WHEN b.side = 'YES' THEN 1.0 ELSE 0.0 END)
                    * (r.predicted_prob - CASE WHEN b.side = 'YES' THEN 1.0 ELSE 0.0 END)) as brier
         FROM llm_analyses a
         JOIN llm_recommendations r ON r.analysis_id = a.id
         LEFT JOIN llm_decisions d ON d.followed_llm_id = r.id
         LEFT JOIN bets b ON b.id = d.bet_id AND b.settled_at IS NOT NULL AND b.settled_at >= ?
         WHERE a.prompt_version = ?
         GROUP BY a.prompt_version, r.provider_id
         ORDER BY r.provider_id",
    )
    .bind(cutoff)
    .bind(&args.prompt_version)
    .fetch_all(&state.db)
    .await?;

    let out: Vec<LlmStatsByPrompt> = rows.into_iter().map(|(pv, pid, n_rec, n_ev, wr, brier)| {
        LlmStatsByPrompt {
            prompt_version: pv,
            provider_id: pid,
            n_recommendations: n_rec,
            n_evaluated: n_ev,
            win_rate: wr.unwrap_or(0.0),
            brier: brier.unwrap_or(0.0),
        }
    }).collect();
    Ok(out)
}

#[tauri::command]
pub async fn llm_stats_cost_efficiency(
    state: State<'_, AppState>,
    window_days: Option<i64>,
) -> AppResult<Vec<LlmStatsCostEfficiency>> {
    let window = window_days.unwrap_or(30);
    let cutoff = chrono::Utc::now().timestamp_millis() - window * 24 * 3600 * 1000;

    let rows = sqlx::query_as::<_, (String, Option<f64>, Option<f64>, Option<f64>, Option<f64>)>(
        "SELECT r.provider_id,
                SUM(c.cost_cents) as total_cost,
                SUM(CAST(b.pnl AS REAL)) as total_pnl,
                CAST(SUM(CASE WHEN b.status = 'won' AND r.side = b.side THEN 1 ELSE 0 END) AS REAL)
                    / NULLIF(COUNT(b.id), 0) as win_rate,
                COUNT(b.id) as n
         FROM llm_call_logs c
         JOIN llm_recommendations r ON r.id = (
             SELECT r2.id FROM llm_recommendations r2
             JOIN llm_analyses a ON a.id = r2.analysis_id
             WHERE a.id = c.analysis_id AND r2.provider_id = c.provider_id LIMIT 1
         )
         LEFT JOIN llm_decisions d ON d.followed_llm_id = r.id
         LEFT JOIN bets b ON b.id = d.bet_id AND b.settled_at IS NOT NULL AND b.settled_at >= ?
         WHERE c.called_at >= ? AND c.cost_cents > 0
         GROUP BY r.provider_id",
    )
    .bind(cutoff)
    .bind(cutoff)
    .fetch_all(&state.db)
    .await?;

    let out: Vec<LlmStatsCostEfficiency> = rows.into_iter().map(|(pid, cost, pnl, wr, _n)| {
        let cost_v = cost.unwrap_or(0.0);
        let pnl_v = pnl.unwrap_or(0.0);
        let eff = if cost_v > 0.0 { pnl_v / cost_v } else { 0.0 };
        LlmStatsCostEfficiency {
            provider_id: pid,
            total_cost_cents: cost_v,
            total_pnl: pnl_v,
            efficiency: eff,
            win_rate: wr.unwrap_or(0.0),
        }
    }).collect();
    Ok(out)
}

#[derive(Debug, Deserialize)]
pub struct ExportStatsArgs {
    pub format: String, // 'csv' | 'json'
    pub window_days: Option<i64>,
}

/// Export LLM recommendation data joined with bet outcomes.
/// Returns a string the frontend saves via tauri-plugin-fs.
#[tauri::command]
pub async fn llm_stats_export(
    state: State<'_, AppState>,
    args: ExportStatsArgs,
) -> AppResult<String> {
    let window = args.window_days.unwrap_or(90);
    let cutoff = chrono::Utc::now().timestamp_millis() - window * 24 * 3600 * 1000;

    let rows = sqlx::query_as::<_, (String, String, Option<i64>, String, Option<f64>, Option<String>, Option<f64>, Option<i64>, Option<i64>, Option<i64>, Option<f64>, Option<String>, Option<String>, Option<String>, Option<i64>)>(
        "SELECT a.id, a.prompt_version, a.requested_at,
                r.provider_id, r.predicted_prob, r.side, r.confidence, r.latency_ms,
                r.tokens_in, r.tokens_out, r.cost_cents,
                b.status, b.pnl, b.side, b.settled_at
         FROM llm_analyses a
         JOIN llm_recommendations r ON r.analysis_id = a.id
         LEFT JOIN llm_decisions d ON d.followed_llm_id = r.id
         LEFT JOIN bets b ON b.id = d.bet_id
         WHERE a.requested_at >= ?
         ORDER BY a.requested_at DESC",
    )
    .bind(cutoff)
    .fetch_all(&state.db)
    .await?;

    let body = match args.format.as_str() {
        "csv" => {
            let mut s = String::from("analysis_id,prompt_version,requested_at,provider,predicted_prob,recommended_side,confidence,latency_ms,tokens_in,tokens_out,cost_cents,bet_status,bet_pnl,followed_side,settled_at\n");
            for r in &rows {
                s.push_str(&format!("{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
                    r.0, r.1, r.2.map(|n| n.to_string()).unwrap_or_default(), r.3,
                    r.4.map(|n| n.to_string()).unwrap_or_default(),
                    r.5.clone().unwrap_or_default(),
                    r.6.map(|n| n.to_string()).unwrap_or_default(),
                    r.7.map(|n| n.to_string()).unwrap_or_default(),
                    r.8.map(|n| n.to_string()).unwrap_or_default(),
                    r.9.map(|n| n.to_string()).unwrap_or_default(),
                    r.10.map(|n| n.to_string()).unwrap_or_default(),
                    r.11.clone().unwrap_or_default(),
                    r.12.clone().unwrap_or_default(),
                    r.13.clone().unwrap_or_default(),
                    r.14.map(|n| n.to_string()).unwrap_or_default()));
            }
            s
        }
        _ => {
            let arr: Vec<_> = rows.into_iter().map(|r| serde_json::json!({
                "analysis_id": r.0, "prompt_version": r.1, "requested_at": r.2,
                "provider": r.3, "predicted_prob": r.4, "recommended_side": r.5,
                "confidence": r.6, "latency_ms": r.7, "tokens_in": r.8, "tokens_out": r.9, "cost_cents": r.10,
                "bet_status": r.11, "bet_pnl": r.12, "followed_side": r.13, "settled_at": r.14,
            })).collect();
            serde_json::to_string_pretty(&arr).unwrap_or_else(|_| "[]".to_string())
        }
    };
    Ok(body)
}
