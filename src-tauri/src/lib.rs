//! polyrocket — library crate (commands + setup)
//!
//! Spec: polyradar-blueprint-v2-client.md §3
//! Every command corresponds to a Tauri IPC entry that the React frontend
//! can invoke via `invoke('cmd_name', { args })`.

// v0.76 — codegen bin (src/bin/gen_ts_types.rs) needs
// access to the auto-generated `__cmd__*` and
// `__specta__fn__*` symbols (private to the commands
// module). Making the whole `commands` module pub
// exposes the internal IPC surface, but it's already
// reachable via Tauri's invoke_handler in main.rs, so
// the visibility change has no security impact.
#[doc(hidden)]
pub mod commands;
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

    // v0.42b — telemetry init. Default off; enable with
    // `POLYROCKET_TELEMETRY=1` in the env. Must run BEFORE
    // the schedulers start so loop events are captured.
    infra::telemetry::init_from_env();

    // Dev .env → OS keyring sync (L5 platform/env). Gated by env vars;
    // no-op in any environment other than POLYROCKET_ENV=dev +
    // POLYROCKET_KEYRING_ONLY=0.
    platform::env::maybe_load_dev_env();

    // v0.56 — load the network proxy from
    // `network_proxy.json` (if present) into
    // `POLYROCKET_PROXY` so the shared HTTP
    // client picks it up. We do this BEFORE
    // building the http client in `setup` so
    // the env var is in scope when
    // `new_http_client()` runs.
    //
    // We can't use `app.path()` here (the
    // AppHandle is only available inside
    // `setup`), so we read the config via the
    // JSON file path (which the Tauri docs
    // promise is stable).
    load_proxy_from_json_into_env();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_sql::Builder::default().build())
        .setup(|app| {
            let app_handle = app.handle().clone();
            // v0.11b — store the AppHandle so the scheduler can look
            // up managed state (SidecarState) without going through L2.
            let _ = infra::scheduler::TAURI_APP.set(app_handle.clone());

            // v0.49a — telemetry file retention. Set up the
            // log dir (<app_data_dir>/logs/telemetry) and run
            // a sweep on startup to delete session files
            // older than the retention window (default 14d).
            // Errors here are non-fatal: telemetry remains
            // functional via the stderr sink; only the file
            // component is degraded.
            match platform::paths::log_dir(&app_handle) {
                Ok(d) => {
                    let tel_dir = d.join("telemetry");
                    if let Err(e) = infra::telemetry::set_log_dir(tel_dir) {
                        tracing::warn!(
                            "telemetry log dir setup failed: {e} (file sink disabled)"
                        );
                    } else {
                        match infra::telemetry::purge_telemetry_logs() {
                            Ok(n) => tracing::info!(
                                "telemetry retention sweep on startup: {n} file(s) deleted"
                            ),
                            Err(e) => tracing::warn!(
                                "telemetry retention sweep failed: {e}"
                            ),
                        }
                    }
                }
                Err(e) => tracing::warn!(
                    "telemetry log_dir setup failed (no app_data_dir): {e}"
                ),
            }

            tauri::async_runtime::block_on(async move {
                let pool = infra::db::init_pool(&app_handle)
                    .await
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
                // Start background schedulers (health probe + daily brief + anomaly detect + mirror executor).
                // They run for the process lifetime; the handle is kept in app state
                // for test shutdown signaling.
                let http = infra::http::new_http_client();
                let handle = infra::scheduler::start(pool.clone(), http);
                app_handle.manage(infra::state::AppState::new(pool));
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
            commands::market::list_resolved_markets_for_backtest,
            commands::signal::list_active_signals,
            commands::signal::recompute_signals,
            commands::bet::place_jump_link,
            commands::bet::place_signed_order,
            commands::bet::list_bets,
            commands::bet::validate_order_args,
            commands::copy::list_copy_targets,
            commands::copy::add_copy_target,
            commands::copy::recent_copy_events,
            commands::pnl::dashboard_kpis,
            commands::pnl::paper_pnl_summary,
            commands::pnl::fill_analytics,
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
            commands::mirror_executor::set_mirror_paper_mode,
            commands::mirror_executor::get_mirror_paper_mode,
            commands::mirror_executor::list_paper_fills,
            commands::sidecar::start_sidecar,
            commands::sidecar::stop_sidecar,
            commands::sidecar::sidecar_status,
            commands::sidecar::sidecar_predict,
            commands::sidecar::sidecar_predict_async,
            commands::sidecar::train_job,
            commands::sidecar::promote_model,
            commands::sidecar::list_promote_history,
            commands::sidecar::rollback_model,
            commands::sidecar::auto_promote_if_better,
            commands::sidecar::promote_all_trials,
            commands::sidecar::backtest_model,
            commands::sidecar::explain_model, // v0.55
            commands::sidecar::shap_explain, // v0.59
            commands::sidecar::sidecar_request,
            commands::sidecar::set_auto_promote_config,
            commands::sidecar::get_auto_promote_config,
            commands::sidecar::list_promote_history_archive,
            commands::sidecar::set_telemetry_enabled,
            commands::sidecar::get_telemetry_enabled,
            commands::telemetry::list_telemetry_logs,
            commands::telemetry::purge_telemetry_logs,
            commands::active_model::get_active_model,
            commands::clob::clob_feed_status,
            commands::clob::record_clob_snapshot_now,
            commands::clob::latest_clob_snapshot,
            commands::storage::get_storage_info,
            commands::storage::set_storage_path,
            commands::storage::reset_storage_path,
            // v0.54b — storage migration tool
            commands::storage_migrate::migrate_storage_path,
            // v0.56 — network proxy / Tor support
            commands::network::get_proxy_config,
            commands::network::set_proxy_config,
            commands::network::clear_proxy_config,
            commands::network::read_proxy_config_file,
            // v0.54a — tauri-plugin-dialog wrappers
            commands::dialog::pick_directory,
            commands::dialog::pick_file,
            commands::sidecar_health::sidecar_health_now,
            commands::sidecar_health::sidecar_health_snapshot,
            commands::scheduler::scheduler_status,
            commands::scheduler::scheduler_run_health_probe_now,
            commands::scheduler::scheduler_run_daily_brief_now,
            commands::scheduler::degradation_check_now,
            commands::scheduler::scheduler_self_test_now,
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

/// v0.56 — read `network_proxy.json` from the
/// OS-default app_data_dir and set
/// `POLYROCKET_PROXY` from it. Best-effort: if
/// the file is missing or malformed, we leave
/// the env var untouched.
///
/// The file path is
/// `<app_data_dir>/network_proxy.json` where
/// `<app_data_dir>` follows Tauri's platform
/// convention (macOS: `~/Library/Application
/// Support/com.polyrocket.app/`). We use
/// `dirs_next` to resolve the data dir without
/// an AppHandle.
fn load_proxy_from_json_into_env() {
    if std::env::var("POLYROCKET_PROXY").is_ok() {
        // Already set (e.g. dev workflow or
        // shell). Don't override.
        return;
    }
    let Some(app_dir) = platform::paths::default_app_data_dir() else {
        return;
    };
    let file = app_dir.join("network_proxy.json");
    if !file.exists() {
        return;
    }
    let Ok(raw) = std::fs::read_to_string(&file) else {
        return;
    };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return;
    };
    if let Some(url) = v.get("url").and_then(|u| u.as_str()) {
        if !url.is_empty() {
            // SAFETY: setting an env var is
            // thread-safe; multiple threads might
            // race to set the same var, but
            // they all set the same value (the
            // JSON file is the source of truth).
            std::env::set_var("POLYROCKET_PROXY", url);
            tracing::info!(
                "loaded proxy from network_proxy.json: {url}"
            );
        }
    }
}