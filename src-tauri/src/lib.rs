//! polyrocket — library crate (commands + setup)
//!
//! Spec: polyradar-blueprint-v2-client.md §3
//! Every command corresponds to a Tauri IPC entry that the React frontend
//! can invoke via `invoke('cmd_name', { args })`.

mod commands;
pub mod domain;
pub mod infra;
pub mod lab_state;
mod platform;

// Re-exports for integration tests in `tests/`. The `commands`
// module is private to keep its IPC surface internal, but the
// SidecarState struct needs to be reachable from e2e tests so
// they can drive a real Python sidecar.
pub use commands::sidecar::{SidecarState, SidecarStatus};

use tauri::Manager;
use tracing_subscriber::EnvFilter;

pub use infra::error::{AppError, AppResult};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .init();

    // Dev .env → OS keyring sync (L5 platform/env). Gated by env vars;
    // no-op in any environment other than POLYROCKET_ENV=dev +
    // POLYROCKET_KEYRING_ONLY=0.
    platform::env::maybe_load_dev_env();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_sql::Builder::default().build())
        .setup(|app| {
            let app_handle = app.handle().clone();
            // v0.11b — store the AppHandle so the scheduler can look
            // up managed state (SidecarState) without going through L2.
            let _ = infra::scheduler::TAURI_APP.set(app_handle.clone());
            tauri::async_runtime::block_on(async move {
                let pool = infra::db::init_pool(&app_handle)
                    .await
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
                // Start background schedulers (health probe + daily brief + anomaly detect + mirror executor).
                // They run for the process lifetime; the handle is kept in app state
                // for test shutdown signaling.
                let http = infra::http::new_http_client();
                let handle = infra::scheduler::start(pool.clone(), http);
                app_handle.manage(infra::state::AppState { db: pool });
                app_handle.manage(handle);
                // v0.6b — Python sidecar (M7) singleton state
                app_handle.manage(commands::sidecar::SidecarState::new());
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
            commands::audit::list_audit_log,
            commands::audit::audit_count_for_actor,
            commands::audit::purge_audit_log_now,
            commands::audit::get_audit_retention,
            commands::audit::set_audit_retention,
            commands::notify::send_notification,
            commands::notify::request_notification_permission,
            commands::notify::notification_permission_state,
            commands::mirror_executor::enqueue_mirror,
            commands::mirror_executor::list_mirrors,
            commands::mirror_executor::run_mirror_executor_pass,
            commands::mirror_executor::mirror_queue_stats,
            commands::sidecar::start_sidecar,
            commands::sidecar::stop_sidecar,
            commands::sidecar::sidecar_status,
            commands::sidecar::sidecar_predict,
            commands::sidecar::sidecar_predict_async,
            commands::sidecar::train_job,
            commands::sidecar::promote_model,
            commands::sidecar::sidecar_request,
            commands::sidecar_health::sidecar_health_now,
            commands::sidecar_health::sidecar_health_snapshot,
            commands::scheduler::scheduler_status,
            commands::scheduler::scheduler_run_health_probe_now,
            commands::scheduler::scheduler_run_daily_brief_now,
            commands::brief::daily_brief_get,
            commands::brief::daily_brief_dismiss,
            commands::brief::daily_brief_refresh,
            commands::brief::daily_brief_set_prefs,
            commands::seed::seed_demo_data,
            commands::seed::is_seeded,
        ])
        .run(tauri::generate_context!())
        .expect("error while running polyrocket");
}