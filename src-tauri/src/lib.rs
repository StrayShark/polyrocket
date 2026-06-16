//! polyrocket — library crate (commands + setup)
//!
//! Spec: polyradar-blueprint-v2-client.md §3
//! Every command corresponds to a Tauri IPC entry that the React frontend
//! can invoke via `invoke('cmd_name', { args })`.

mod commands;
mod db;
mod error;
mod keyring;
mod polymarket;
mod state;

use tauri::Manager;
use tracing_subscriber::EnvFilter;

pub use error::{AppError, AppResult};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .init();

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
            commands::llm_mgmt::llm_provider_list,
            commands::llm_mgmt::llm_provider_upsert,
            commands::llm_mgmt::llm_provider_delete,
            commands::llm_mgmt::llm_key_list,
            commands::llm_mgmt::llm_key_upsert,
            commands::llm_mgmt::llm_key_delete,
            commands::llm_mgmt::llm_test_connectivity,
            commands::llm_mgmt::llm_traffic_summary,
            commands::llm_mgmt::llm_health_history,
            commands::llm_mgmt::llm_stats_by_confidence,
            commands::llm_mgmt::llm_stats_by_prompt,
            commands::llm_mgmt::llm_stats_cost_efficiency,
            commands::llm_mgmt::llm_stats_export,
            commands::brief::daily_brief_get,
            commands::brief::daily_brief_dismiss,
            commands::brief::daily_brief_refresh,
            commands::brief::daily_brief_set_prefs,
        ])
        .run(tauri::generate_context!())
        .expect("error while running polyrocket");
}