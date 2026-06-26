//! polyrocket — library crate（命令 + 启动）
//!
//! 规范：polyradar-blueprint-v2-client.md §3
//! 每个命令对应一个 Tauri IPC 入口，React 前端
//! 可通过 `invoke('cmd_name', { args })` 调用。

// v0.76 — codegen bin（src/bin/gen_ts_types.rs）需要
// 访问自动生成的 `__cmd__*` 和
// `__specta__fn__*` 符号（这些是 commands 模块私有的）。
// 将整个 `commands` 模块设置为 pub 会暴露内部 IPC 接口，
// 但这些接口已经可以通过 main.rs 中的 Tauri invoke_handler
// 访问，因此可见性的变更不影响安全性。
#[doc(hidden)]
pub mod commands;
pub mod codegen;
pub mod domain;
pub mod infra;
pub mod lab_state;
mod platform;

// v0.119 — Tauri 应用的 E2E 测试模式：`cargo run --bin polyrocket -- --e2e-football`。
// 启动完整的应用 + React 前端，导航到足球市场，从 webview 调用
// analyze IPC，捕获结果，将其写入
// /tmp/polyrocket-e2e-result.json，然后退出。由 scripts/e2e_football_app.sh 使用。
mod e2e_football;
// v0.121 — UI 驱动的 e2e：当设置了 POLYROCKET_E2E_UI_DRIVE=1 时，
// e2e_football 委托给 e2e_football_ui_drive，后者使用真实的
// DOM click() 来点击 UI（与用户使用鼠标操作的路径相同）。
mod e2e_football_ui_drive;
// v0.121 — 直接多市场 e2e：当设置了
// POLYROCKET_E2E_ALL_FOOTBALL=1 时，在单个进程中
// 为全部 4 个足球种子市场直接调用 `llm_analyze` IPC。
// 跳过 webview eval 以避免 JSON 中的 NaN 导致崩溃。
mod e2e_football_all;

// 为 `tests/` 中的集成测试重新导出。`commands`
// 模块是私有的以保持其 IPC 接口内部化，但
// SidecarState 结构体需要可以从 e2e 测试中访问，
// 以便它们能够驱动真正的 Python 侧车（sidecar）。
pub use commands::sidecar::{SidecarState, SidecarStatus};

use tauri::Manager;
use tracing_subscriber::EnvFilter;

pub use infra::error::{AppError, AppResult};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // v0.119 — `--e2e-football` 以无头 E2E 模式启动完整应用：
    // 导航到足球市场，调用 analyze IPC，捕获
    // 预测结果，将其写入 /tmp，然后退出。参见 `e2e_football` 模块。
    let args: Vec<String> = std::env::args().collect();
    let e2e_mode = args.iter().any(|a| a == "--e2e-football");

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .init();
    tracing::info!(e2e_mode, "polyrocket starting");

    // v0.42b — telemetry 初始化。默认关闭；可通过
    // 环境变量 `POLYROCKET_TELEMETRY=1` 启用。
    // 必须在调度器启动之前运行，以便捕获循环事件。
    infra::telemetry::init_from_env();

    // 开发环境 .env → OS keyring 同步（L5 platform/env）。
    // 由环境变量控制；仅在 POLYROCKET_ENV=dev +
    // POLYROCKET_KEYRING_ONLY=0 时生效，其他环境下为 no-op。
    platform::env::maybe_load_dev_env();

    // v0.56 — 如果存在，从 `network_proxy.json`
    // 加载网络代理到 `POLYROCKET_PROXY`，
    // 以便共享的 HTTP 客户端能读取它。
    // 我们在 `setup` 中构建 http 客户端之前执行此操作，
    // 以便 `new_http_client()` 运行
    // 时该环境变量已经设置。
    //
    // 这里不能使用 `app.path()`（AppHandle 只在
    // `setup` 内部可用），因此我们通过
    // JSON 文件路径读取配置（Tauri 文档承诺该路径是稳定的）。
    load_proxy_from_json_into_env();
    tracing::info!("polyrocket: load_proxy done, building Tauri app");

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_sql::Builder::default().build())
        .setup(move |app| {
            tracing::info!("polyrocket: setup callback starting");
            let app_handle = app.handle().clone();
            // v0.11b — 存储 AppHandle，使调度器可以查找
            // 受管状态（SidecarState），而无需通过 L2。
            let _ = infra::scheduler::TAURI_APP.set(app_handle.clone());

            // v0.49a — telemetry 文件保留策略。设置日志目录
            // （<app_data_dir>/logs/telemetry）并在启动时
            // 运行扫描，删除早于保留窗口（默认 14 天）的会话文件。
            // 此处的错误不会导致致命问题：telemetry
            // 仍可通过 stderr sink 工作；只有
            // 文件组件会降级。
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
                // 启动后台调度器（健康探测 + 每日简报 + 异常检测 + 镜像执行器）。
                // 它们会在进程生命周期内运行；handle 保留在应用状态中，
                // 用于测试关闭时的信号。
                let http = infra::http::new_http_client();
                let handle = infra::scheduler::start(pool.clone(), http);
                app_handle.manage(infra::state::AppState::new(pool));
                app_handle.manage(handle);
                // v0.6b — Python 侧车（sidecar，M7）单例状态
                app_handle.manage(commands::sidecar::SidecarState::new());

                // v0.123 — 开发模式 .env → LLM provider 行 + PM 钱包行
                // 自注册。幂等：每次启动时重新运行
                // 都会收敛到相同的规范状态。
                // 由 `POLYROCKET_ENV=dev` + `POLYROCKET_KEYRING_ONLY!=1` 控制
                // （与 platform::env::maybe_load_dev_env 中的 keyring 同步门控相同）。
                infra::bootstrap::bootstrap(&app_handle);

                // v0.119 — E2E 足球模式：派生驱动任务，该任务将
                // 导航 webview，调用 analyze IPC，捕获结果，
                // 将其写入 /tmp，然后退出应用。
                if e2e_mode {
                    let h = app_handle.clone();
                    tauri::async_runtime::spawn(async move {
                        // v0.121 — 当 POLYROCKET_E2E_UI_DRIVE=1 时，
                        // 委托给使用真实 DOM 点击
                        // （而非 pushState + 直接 invoke）
                        // 的 UI 驱动模式。
                        let ui_drive = std::env::var("POLYROCKET_E2E_UI_DRIVE")
                            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                            .unwrap_or(false);
                        if ui_drive {
                            e2e_football_ui_drive::run(h).await;
                            return;
                        }
                        // v0.121 — 当 POLYROCKET_E2E_ALL_FOOTBALL=1 时，
                        // 依次为全部 4 个足球种子市场运行 e2e。
                        // 每个市场将单独的结果文件
                        // 写入 /tmp/polyrocket-e2e-{N}.json
                        // （其中 N 为 1..4），以便操作员
                        // 查看应用中全部 4 个市场的预测。
                        let all_football = std::env::var("POLYROCKET_E2E_ALL_FOOTBALL")
                            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                            .unwrap_or(false);
                        if all_football {
                            e2e_football_all::run_all_football(h).await;
                            return;
                        }
                        e2e_football::run(h).await;
                    });
                }

                Ok::<(), Box<dyn std::error::Error>>(())
            })
        })
        .invoke_handler(tauri::generate_handler![
            commands::bankroll::compute_allocation_preview,  // v0.78 — M11
            commands::bankroll::get_bankroll_config,
            commands::bankroll::set_bankroll_config,
            commands::bankroll::apply_allocation,
            commands::wallet::list_wallets,
            commands::wallet::add_wallet,
            commands::wallet_balance::get_wallet_balance,
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
            // v0.54b — 存储迁移工具
            commands::storage_migrate::migrate_storage_path,
            // v0.56 — 网络代理 / Tor 支持
            commands::network::get_proxy_config,
            commands::network::set_proxy_config,
            commands::network::clear_proxy_config,
            commands::network::read_proxy_config_file,
            // v0.54a — tauri-plugin-dialog 包装器
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
            // P1-1 — 新闻关联器
            commands::news::market_news,
            commands::news::list_all_news,
            // P1-2 — NL 查询
            commands::nl_query::nl_query,
            // P1-3 — 套利扫描器
            commands::arb::arb_scan,
            commands::arb::list_arb_opportunities,
            // P1-4 — Kalshi 跨平台套利
            commands::cross_platform_arb::cross_platform_arb_scan,
            // P2-1 — Poisson 得分矩阵
            commands::poisson::poisson_score_matrix,
            // P2-2 — 足球上下文（休息天数与疲劳度）
            commands::football_context::get_football_context,
            // P2-3 — UMA 争议状态
            commands::uma::uma_dispute_status,
            // P0-1 — 聪明钱得分
            commands::smart_money::smart_money_score,
            // P0-2 — 市场日历
            commands::calendar::market_calendar,
            // P0-3 — 尖峰检测
            commands::spike::list_spike_alerts,
            commands::spike::run_spike_scan,
            // Phase 1.1 — 群众智慧（资本加权观点）
            commands::crowd_wisdom::crowd_opinion,
            // Phase 1.2 — 均值回归
            commands::mean_reversion::reversion_signal,
        ])
        .run(tauri::generate_context!())
        .expect("error while running polyrocket");
}

/// v0.56 — 从操作系统默认的 app_data_dir 读取
/// `network_proxy.json` 并从中设置
/// `POLYROCKET_PROXY`。尽力而为：如果文件
/// 缺失或格式错误，则保持
/// 环境变量不变。
///
/// 文件路径为
/// `<app_data_dir>/network_proxy.json`，其中
/// `<app_data_dir>` 遵循 Tauri 的平台
/// 约定（macOS: `~/Library/Application
/// Support/com.polyrocket.app/`）。我们使用
/// `dirs_next` 在没有 AppHandle 的情况下解析数据目录。
fn load_proxy_from_json_into_env() {
    if std::env::var("POLYROCKET_PROXY").is_ok() {
        // 已经设置（例如开发工作流或
        // shell 环境）。不要覆盖。
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
            // SAFETY: 设置环境变量是
            // 线程安全的；多个线程可能
            // 竞争设置同一个变量，但
            // 它们都设置相同的值（JSON 文件
            // 是唯一的真相来源）。
            std::env::set_var("POLYROCKET_PROXY", url);
            tracing::info!(
                "loaded proxy from network_proxy.json: {url}"
            );
        }
    }
}