//! L4 — v0.123 dev bootstrap.
//!
//! On startup, when `POLYROCKET_ENV=dev` (and keyring-only mode is
//! off — the v0.119 default), inspect the loaded `.env` and
//! self-register LLM provider rows + a PM wallet row so the L1
//! has something to render. This eliminates the "fresh DB has
//! nothing to show" problem that blocked the v0.121 UI E2E
//! (the Welcome flow was the only place these rows got created;
//! skipping Welcome left the dashboard empty).
//!
//! Idempotent: every operation is `INSERT OR IGNORE` or
//! `INSERT OR REPLACE` keyed on a deterministic id, so calling
//! `bootstrap()` on every boot is safe and converges to the
//! canonical state.
//!
//! Layer rules: this module sits at L4 (infrastructure) and
//! uses L4 `infra::db::pool` + L4 `platform::env` (the .env
//! loader already populated the process env). It does NOT
//! call into L2/L3 IPC handlers — the LLM provider upsert
//! goes through the SQL path directly (a literal copy of what
//! `commands::llm_mgmt::llm_provider_upsert` does, minus the
//! specta wrapper).

use crate::infra::error::AppResult;
use crate::infra::state::AppState;
use serde_json::json;
use sqlx::SqlitePool;
use tauri::AppHandle;
use tauri::Manager;

/// Run the bootstrap. Safe to call from `lib.rs::run()`'s
/// setup hook on every boot. Errors are logged but do NOT
/// fail the boot — the app stays usable even if .env is
/// missing or the DB is read-only.
pub fn bootstrap(app: &AppHandle) {
    // Gating: same rules as the .env loader. Without the
    // dev-mode flag we don't auto-register (a real production
    // user expects to paste their own keys).
    let env_name = std::env::var("POLYROCKET_ENV").unwrap_or_default();
    let keyring_only = std::env::var("POLYROCKET_KEYRING_ONLY")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    if env_name != "dev" || keyring_only {
        tracing::info!(
            "v0.123 bootstrap: skipped (env={env_name:?}, keyring_only={keyring_only})"
        );
        return;
    }

    let pool = app.state::<AppState>().db.clone();

    // We're already inside the Tauri async runtime (the
    // setup hook runs on it). Just spawn a task — DO NOT
    // use `block_on` here, that panics with "Cannot start
    // a runtime from within a runtime".
    tauri::async_runtime::spawn(async move {
        match do_bootstrap(&pool).await {
            Ok(n) => tracing::info!("v0.123 bootstrap: {n} action(s) applied"),
            Err(e) => tracing::warn!("v0.123 bootstrap: failed: {e}"),
        }
    });
}

async fn do_bootstrap(pool: &SqlitePool) -> AppResult<usize> {
    let mut actions = 0usize;
    actions += register_llm_providers(pool).await?;
    actions += register_pm_wallet(pool).await?;
    Ok(actions)
}

/// Walk the candidate LLM provider list. For each one with a
/// `*_API_KEY` env var set, INSERT OR IGNORE a row into
/// `llm_providers` + `llm_provider_keys` (the two tables the
/// LLM dispatch IPC reads from).
async fn register_llm_providers(pool: &SqlitePool) -> AppResult<usize> {
    // Candidates mirrored from `e2e_football::bootstrap_providers`.
    // The id here is the SHORT form (e.g. `MiniMax`); the
    // `llm_providers.id` column stores the same short form, and
    // `pick_keys` matches on it. The env var name is
    // `{id_uppercase}_API_KEY` (e.g. `MINIMAX_API_KEY`).
    let candidates: &[(&str, &str, &str, &str, f64, f64)] = &[
        ("MiniMax", "MiniMax (M2.7)",         "https://api.minimax.chat/v1",                "MiniMax-M2.7",                 0.4,  1.2),
        ("doubao",  "Doubao (火山方舟)",        "https://ark.cn-beijing.volces.com/api/coding/v3",   "doubao-seed-2-0-pro-260215",   0.08, 0.08),
        ("qwen",    "Qwen (DashScope)",        "https://dashscope.aliyuncs.com/compatible-mode/v1","qwen-plus",                    0.4,  1.2),
        ("moonshot","Moonshot (Kimi)",         "https://api.moonshot.cn/v1",                 "kimi-k2-0711-preview",          0.6,  0.6),
        ("zhipu",   "Zhipu (GLM)",             "https://open.bigmodel.cn/api/paas/v4",       "glm-4-plus",                  5.0,  5.0),
    ];

    let now_ms = chrono::Utc::now().timestamp_millis();
    let mut count = 0usize;

    for (id, display, api_base, model, ci, co) in candidates {
        let env_var = format!("{}_API_KEY", id.to_uppercase());
        let secret = match std::env::var(&env_var).ok().filter(|v| !v.is_empty()) {
            Some(s) => s,
            None => continue,
        };

        // 1) upsert into llm_providers. The id column stores
        //    the SHORT form (e.g. `MiniMax`) — same form as
        //    `llm_provider_keys.provider_id`.
        let upsert_ok = sqlx::query(
            "INSERT INTO llm_providers
                (id, display_name, provider_kind, request_format, supports_streaming,
                 enabled, api_base, key_alias, default_model, timeout_ms,
                 request_timeout_ms, max_retries, cost_per_1k_in, cost_per_1k_out,
                 key_rotation_strategy, health_status, updated_at, notes)
             VALUES (?, ?, 'openai_compat', 'openai_compat', 0, 1, ?, 'prod-1',
                     ?, 120000, 120000, 2, ?, ?, 'round_robin', 'unknown', ?, 'bootstrapped by v0.123 dev-env sync')
             ON CONFLICT(id) DO UPDATE SET
                api_base = excluded.api_base,
                default_model = excluded.default_model,
                enabled = 1,
                updated_at = excluded.updated_at",
        )
        .bind(id)
        .bind(display)
        .bind(api_base)
        .bind(model)
        .bind(*ci)
        .bind(*co)
        .bind(now_ms)
        .execute(pool)
        .await;

        if let Err(e) = upsert_ok {
            tracing::warn!("bootstrap: llm_providers upsert for {id} failed: {e}");
            continue;
        }

        // 2) insert a matching llm_provider_keys row (the dispatch IPC
        //    reads from this table — without a row, "no enabled keys").
        let key_id = format!("{id}-prod-1-bootstrap");
        let keyring_alias = format!("llm/{id}/prod-1");
        let ins = sqlx::query(
            "INSERT OR IGNORE INTO llm_provider_keys
                (id, provider_id, alias, keyring_alias, enabled, priority, weight,
                 created_at, updated_at, notes)
             VALUES (?, ?, 'prod-1', ?, 1, 0, 1.0, ?, ?, 'bootstrapped by v0.123 dev-env sync')",
        )
        .bind(&key_id)
        .bind(id)
        .bind(&keyring_alias)
        .bind(now_ms)
        .bind(now_ms)
        .execute(pool)
        .await;

        match ins {
            Ok(r) if r.rows_affected() > 0 => {
                count += 1;
                tracing::info!("bootstrap: registered {id} (key_len={})", secret.len());
            }
            Ok(_) => {
                // already present — idempotent skip
            }
            Err(e) => {
                tracing::warn!("bootstrap: llm_provider_keys insert for {id} failed: {e}");
            }
        }
    }

    Ok(count)
}

/// Insert a placeholder `wallets` row for the L2 PM address so
/// the L1 has a wallet to display. We do NOT have the address
/// from the .env (the .env only has the L2 API key/secret/passphrase,
/// not the L1 polygon address); we leave address blank and let the
/// Welcome flow fill it in if the user wants to use Mode A.
///
/// For the simplified v0.123 UI this row is only used to make
/// `list_wallets` return at least one entry (so the sidebar
/// footer can show "1 wallet"). It carries no balance — the
/// balance IPC hits the CLOB API directly.
async fn register_pm_wallet(pool: &SqlitePool) -> AppResult<usize> {
    // Skip if any wallet already exists.
    let existing: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM wallets")
        .fetch_one(pool)
        .await?;
    if existing > 0 {
        return Ok(0);
    }
    let now_ms = chrono::Utc::now().timestamp_millis();
    let id = "wallet-bootstrap-pm".to_string();
    let label = "PM (auto-bootstrapped)".to_string();
    sqlx::query(
        "INSERT INTO wallets (id, address, label, chain_id, wallet_type, created_at)
         VALUES (?, '', ?, 137, 'pm-l2', ?)",
    )
    .bind(&id)
    .bind(&label)
    .bind(now_ms)
    .execute(pool)
    .await?;
    tracing::info!("bootstrap: registered placeholder PM wallet (L2 creds only, no L1 address)");
    Ok(1)
}

// Suppress unused warnings on the `json!` macro import (kept for
// future use when we expand this to write richer provider metadata).
#[allow(dead_code)]
fn _force_json_use() -> serde_json::Value {
    json!({})
}
