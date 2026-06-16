//! polyrocket — library crate (commands + setup)
//!
//! Spec: polyradar-blueprint-v2-client.md §3
//! Every command corresponds to a Tauri IPC entry that the React frontend
//! can invoke via `invoke('cmd_name', { args })`.

mod commands;
mod db;
mod error;
mod keyring;
pub mod llm_clients;
mod polymarket;
mod state;

use tauri::Manager;
use tracing_subscriber::EnvFilter;

pub use error::{AppError, AppResult};

/// Dev-mode .env sync — runs at startup, ONLY when:
///   1) POLYROCKET_ENV=dev
///   2) POLYROCKET_KEYRING_ONLY=0
/// Behaviour:
///   - reads `.env` from the current working directory
///   - for each recognised `<PROVIDER>_API_KEY` whose corresponding
///     keychain entry is currently empty, writes the value to the OS
///     keyring under the canonical alias.
///   - never logs the secret, never panics on parse errors.
/// In any other environment this is a no-op — the client-paste path
/// (Settings → LLM Management → Add key) is the only source of truth.
fn maybe_load_dev_env() {
    let env = std::env::var("POLYROCKET_ENV").unwrap_or_default();
    let keyring_only = std::env::var("POLYROCKET_KEYRING_ONLY")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    if env != "dev" {
        tracing::info!("startup: POLYROCKET_ENV={env:?} — .env sync disabled");
        return;
    }
    if keyring_only {
        tracing::info!("startup: POLYROCKET_KEYRING_ONLY=1 — .env sync disabled");
        return;
    }
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let env_path = cwd.join(".env");
    if !env_path.exists() {
        tracing::info!("startup: no .env at {} — nothing to sync", env_path.display());
        return;
    }
    let pairs = keyring::parse_env_file(&env_path);
    tracing::info!("startup: dev .env sync — {} entries from {}", pairs.len(), env_path.display());
    sync_env_to_keyring(&pairs);
}

fn sync_env_to_keyring(pairs: &[(String, String)]) {
    use std::collections::HashMap;
    let map: HashMap<String, String> = pairs.iter().cloned().collect();
    let mut written = 0usize;
    let mut skipped = 0usize;

    // LLM providers — map env var to (provider_id, key_alias)
    let llm_map: &[(&str, &str, &str)] = &[
        ("OPENAI_API_KEY",     "openai",    "openai-prod-1"),
        ("OPENAI_BACKUP_KEY",  "openai",    "openai-backup"),
        ("ANTHROPIC_API_KEY",  "anthropic", "anthropic-prod-1"),
        ("ANTHROPIC_BACKUP_KEY", "anthropic", "anthropic-backup"),
        ("GOOGLE_API_KEY",     "google",    "google-prod-1"),
        ("DEEPSEEK_API_KEY",   "deepseek",  "deepseek-prod-1"),
        ("CUSTOM_LLM_API_KEY", "custom",    "custom-prod-1"),
    ];
    for (env_var, provider_id, key_alias) in llm_map {
        if let Some(v) = map.get(*env_var) {
            let alias = keyring::llm_alias(provider_id, key_alias);
            if keyring::has_key(&alias) {
                skipped += 1;
            } else {
                if let Err(e) = keyring::set_key(&alias, v) {
                    tracing::warn!("startup: failed to write {alias}: {e}");
                } else {
                    written += 1;
                    tracing::info!("startup: seeded keychain {alias} from .env");
                }
            }
        }
    }

    // Polymarket
    if let Some(v) = map.get("POLYMARKET_API_KEY") {
        let a = keyring::pm_api_alias();
        if !keyring::has_key(a) { let _ = keyring::set_key(a, v); written += 1; } else { skipped += 1; }
    }
    if let Some(v) = map.get("POLYMARKET_API_SECRET") {
        let a = keyring::pm_secret_alias();
        if !keyring::has_key(a) { let _ = keyring::set_key(a, v); written += 1; } else { skipped += 1; }
    }
    if let Some(v) = map.get("POLYMARKET_API_PASSPHRASE") {
        let a = keyring::pm_passphrase_alias();
        if !keyring::has_key(a) { let _ = keyring::set_key(a, v); written += 1; } else { skipped += 1; }
    }

    // Wallet
    if let Some(v) = map.get("POLYROCKET_WALLET_PRIVATE_KEY") {
        let alias = std::env::var("POLYROCKET_WALLET_ALIAS").unwrap_or_else(|_| "primary".to_string());
        let a = keyring::wallet_alias(&alias);
        if !keyring::has_key(&a) { let _ = keyring::set_key(&a, v); written += 1; } else { skipped += 1; }
    }

    tracing::info!("startup: .env sync done — {written} written, {skipped} already present");
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .init();

    maybe_load_dev_env();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_sql::Builder::default().build())
        .setup(|app| {
            let app_handle = app.handle().clone();
            tauri::async_runtime::block_on(async move {
                let pool = db::init_pool(&app_handle)
                    .await
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
                app_handle.manage(state::AppState { db: pool });
                Ok::<(), Box<dyn std::error::Error>>(())
            })
        })
        .invoke_handler(tauri::generate_handler![
            commands::wallet::list_wallets,
            commands::wallet::add_wallet,
            commands::market::list_markets,
            commands::market::sync_markets,
            commands::signal::list_active_signals,
            commands::signal::recompute_signals,
            commands::bet::place_jump_link,
            commands::bet::place_signed_order,
            commands::bet::list_bets,
            commands::copy::list_copy_targets,
            commands::copy::add_copy_target,
            commands::copy::recent_copy_events,
            commands::pnl::dashboard_kpis,
            commands::llm::list_llm_providers,
            commands::llm::upsert_llm_provider,
            commands::llm::llm_analyze,
            commands::llm::llm_performance,
            commands::llm::record_llm_decision,
            commands::llm::llm_stats_heatmap,
            commands::llm::llm_stats_scatter,
            commands::llm::llm_stats_timeseries,
            commands::llm::llm_stats_decision,
            commands::llm::llm_get_recommendation,
            commands::llm::llm_list_analyses,
            commands::llm_mgmt::llm_provider_list,
            commands::llm_mgmt::llm_provider_upsert,
            commands::llm_mgmt::llm_provider_delete,
            commands::llm_mgmt::llm_key_list,
            commands::llm_mgmt::llm_key_upsert,
            commands::llm_mgmt::llm_key_set_secret,
            commands::llm_mgmt::llm_key_delete,
            commands::llm_mgmt::llm_test_connectivity,
            commands::llm_mgmt::llm_traffic_summary,
            commands::llm_mgmt::llm_health_history,
            commands::llm_mgmt::llm_stats_by_confidence,
            commands::llm_mgmt::llm_stats_by_prompt,
            commands::llm_mgmt::llm_stats_cost_efficiency,
            commands::llm_mgmt::llm_stats_export,
            commands::secrets::llm_pm_set_credentials,
            commands::secrets::llm_pm_clear_credentials,
            commands::secrets::polyrocket_wallet_set_pk,
            commands::secrets::polyrocket_wallet_clear_pk,
            commands::secrets::secrets_status,
            commands::brief::daily_brief_get,
            commands::brief::daily_brief_dismiss,
            commands::brief::daily_brief_refresh,
            commands::brief::daily_brief_set_prefs,
        ])
        .run(tauri::generate_context!())
        .expect("error while running polyrocket");
}