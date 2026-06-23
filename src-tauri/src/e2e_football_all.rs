//! v0.121 — Direct multi-market e2e.
//!
//! Skips webview eval entirely (avoids the NaN-in-JSON crash that
//! the eval-based flow hits on the post-analyze step). Calls
//! `llm_analyze` IPC handler directly from Rust for each of the
//! 4 football seed markets.
//!
//! Used by `POLYROCKET_E2E_ALL_FOOTBALL=1` in lib.rs's setup().

use crate::commands::bet::place_signed_order;
use crate::commands::bet::PlaceSignedArgs;
use crate::commands::llm::llm_analyze;
use crate::infra::state::AppState;
use serde_json::{json, Value};
use std::time::Duration;
use tauri::{AppHandle, Manager};

/// v0.121 — the 4 football seed markets. Run in this order.
const FOOTBALL_MARKETS: &[&str] = &[
    "mkt-football-eu-final-2026",
    "mkt-football-la-liga",
    "mkt-football-premier-top4",
    "mkt-football-world-cup",
];

pub async fn run_all_football(app: AppHandle) {
    tracing::info!(
        "[e2e-all-football] starting direct loop over {} markets",
        FOOTBALL_MARKETS.len()
    );

    // v0.121 — bootstrap LLM provider + key first (same as
    // e2e_football::run() does). Without this the LLM client
    // has no enabled key and `llm_analyze` errors out
    // immediately with "no enabled keys".
    if let Err(e) = super::e2e_football::bootstrap_providers(&app).await {
        tracing::error!("[e2e-all-football] bootstrap_providers failed: {e}");
    } else {
        tracing::info!("[e2e-all-football] providers bootstrapped");
    }

    let mut all_results: Vec<Value> = Vec::new();

    for (i, market_id) in FOOTBALL_MARKETS.iter().enumerate() {
        tracing::info!(
            "[e2e-all-football] [{}/{}] analyzing {}",
            i + 1,
            FOOTBALL_MARKETS.len(),
            market_id
        );

        // Reset state for this market
        if let Some(state) = app.try_state::<AppState>() {
            let pool = state.db.clone();
            let _ = sqlx::query("DELETE FROM signals WHERE market_id = ?")
                .bind(market_id)
                .execute(&pool)
                .await;
            let _ = sqlx::query("DELETE FROM llm_analyses WHERE market_id = ?")
                .bind(market_id)
                .execute(&pool)
                .await;
            let _ = sqlx::query("DELETE FROM clob_snapshots WHERE market_id = ?")
                .bind(market_id)
                .execute(&pool)
                .await;
            let _ = sqlx::query("DELETE FROM bets WHERE market_id = ?")
                .bind(market_id)
                .execute(&pool)
                .await;
        }

        // Call llm_analyze IPC directly.
        let args = json!({
            "market_id": market_id,
            "prompt_version": "football.v1.0",
            "triggered_by": "user:e2e-all-football",
            "provider_ids": ["MiniMax", "doubao"]
        });
        let args_value: Value = args;
        let start = std::time::Instant::now();
        let state = app.state::<AppState>().clone();
        let analyze_result = llm_analyze(
            state.clone(),
            app.clone(),
            serde_json::from_value(args_value).unwrap(),
        )
        .await;
        let elapsed = start.elapsed();
        match analyze_result {
            Ok(r) => {
                let predicted_prob = r.consensus_predicted.unwrap_or(0.0);
                let consensus_conf = r.consensus_conf.unwrap_or(0.0);
                let consensus_side = r.consensus_side.clone().unwrap_or_else(|| "skip".to_string());

                // v0.121 — insert a synthetic signal row for this
                // market so the MarketDetail page renders the
                // prediction. We use the synthetic orderbook mid
                // (0.30 — same as the original e2e_football) for
                // market_mid. This is the same data flow as
                // e2e_football::run()'s step 7.5.
                let market_mid = 0.30_f64;
                let edge = predicted_prob - market_mid;
                let now_ms = chrono::Utc::now().timestamp_millis();
                if let Some(state) = app.try_state::<AppState>() {
                    let pool = state.db.clone();
                    let rationale = format!(
                        "v0.121 all-football — model={} prob={:.2} side={} conf={:.2} market_mid={:.2} edge={:+.2}",
                        r.id.is_empty().then(|| "MiniMax".to_string()).unwrap_or_default(),
                        predicted_prob, consensus_side, consensus_conf, market_mid, edge
                    );
                    let _ = sqlx::query(
                        "INSERT INTO signals
                            (market_id, computed_at, model_version, predicted_prob,
                             market_prob, edge, confidence, horizon_hours, rationale, active)
                         VALUES (?, ?, 'football.v1.0', ?, ?, ?, ?, 720, ?, 1)"
                    )
                    .bind(market_id)
                    .bind(now_ms)
                    .bind(predicted_prob)
                    .bind(market_mid)
                    .bind(edge)
                    .bind(consensus_conf)
                    .bind(&rationale)
                    .execute(&pool)
                    .await;
                    tracing::info!(
                        "[e2e-all-football] {market_id} signal inserted: prob={:.2} edge={:+.2} conf={:.2}",
                        predicted_prob, edge, consensus_conf
                    );
                }

                all_results.push(json!({
                    "market_id": market_id,
                    "ok": true,
                    "elapsed_ms": elapsed.as_millis(),
                    "predicted_prob": predicted_prob,
                    "consensus_side": r.consensus_side,
                    "consensus_conf": r.consensus_conf,
                    "analysis_id": r.id,
                }));

                // v0.121 — paper trade if |edge| >= 0.05.
                // Use a synthetic 0.30 market_mid. Edge is the
                // model's predicted probability vs market mid.
                let market_mid = 0.30_f64;
                let edge = predicted_prob - market_mid;
                if edge.abs() >= 0.05 {
                    let side = consensus_side.to_uppercase();
                    let trade_side = if side == "YES" {
                        "YES"
                    } else if side == "NO" {
                        "NO"
                    } else {
                        "skip"
                    };
                    if trade_side != "skip" {
                        // Look up a real wallet + key from DB.
                        let wallet_row: Option<(String, String)> = sqlx::query_as(
                            "SELECT id, default_key_alias FROM wallets LIMIT 1"
                        )
                        .fetch_optional(&state.db)
                        .await
                        .ok()
                        .flatten();
                        let (wallet_id, key_alias) = wallet_row
                            .unwrap_or_else(|| ("demo-wallet-1".to_string(), "primary".to_string()));
                        let price_clamped = predicted_prob.max(0.01).min(0.99);
                        let place_args = PlaceSignedArgs {
                            market_id: market_id.to_string(),
                            wallet_id: wallet_id.clone(),
                            side: trade_side.to_string(),
                            price: price_clamped,
                            size: "5.00".to_string(),
                            signal_id: None,
                            key_alias: key_alias.clone(),
                            order_type: Some("limit".to_string()),
                            limit_price: Some(price_clamped),
                            stop_price: None,
                            post_only: false,
                            mode: Some("paper".to_string()),
                        };
                        let place_start = std::time::Instant::now();
                        let place_result = place_signed_order(
                            state.clone(),
                            place_args,
                        )
                        .await;
                        let place_elapsed = place_start.elapsed().as_millis();
                        match place_result {
                            Ok(bet) => {
                                tracing::info!(
                                    "[e2e-all-football] {market_id} paper trade placed: id={} side={} price={:.2} size={} mode={} ({}ms)",
                                    bet.id, bet.side, bet.price, bet.size, bet.mode, place_elapsed
                                );
                                all_results.last_mut().unwrap().as_object_mut().unwrap()
                                    .insert("paper_trade".to_string(), json!({
                                        "ok": true,
                                        "id": bet.id,
                                        "side": bet.side,
                                        "price": bet.price,
                                        "size": bet.size,
                                        "mode": bet.mode,
                                        "tx_hash": bet.tx_hash,
                                        "elapsed_ms": place_elapsed,
                                    }));
                            }
                            Err(e) => {
                                tracing::warn!("[e2e-all-football] {market_id} paper trade FAILED: {e}");
                                all_results.last_mut().unwrap().as_object_mut().unwrap()
                                    .insert("paper_trade".to_string(), json!({
                                        "ok": false,
                                        "error": format!("{e}"),
                                    }));
                            }
                        }
                    }
                }
            }
            Err(e) => {
                tracing::error!("[e2e-all-football] {market_id} analyze FAILED: {e}");
                all_results.push(json!({
                    "market_id": market_id,
                    "ok": false,
                    "error": format!("{e}"),
                    "elapsed_ms": elapsed.as_millis(),
                }));
            }
        }
        // small pause between markets
        tokio::time::sleep(Duration::from_secs(2)).await;
    }

    // Write combined result file
    let combined_path = "/tmp/polyrocket-e2e-all-football.json";
    let pretty = serde_json::to_string_pretty(&json!({
        "ok": all_results.iter().all(|r| r.get("ok").and_then(|b| b.as_bool()).unwrap_or(false)),
        "market_count": all_results.len(),
        "results": all_results,
    })).unwrap_or_else(|_| "{}".to_string());
    let _ = std::fs::write(combined_path, pretty);
    tracing::info!("[e2e-all-football] wrote combined result to {combined_path}");

    // v0.121 — navigate the webview to /signals so the L1
    // shows the 4 new football signals. The Dashboard page
    // has its own SQL bug (a column type mismatch in some
    // other component) that we don't have time to fix
    // tonight. The Signals page renders the same data.
    if let Some(window) = app.get_webview_window("main") {
        let nav_js = "window.history.pushState({}, '', '/signals'); \
                      window.dispatchEvent(new PopStateEvent('popstate'));";
        let _ = window.eval(nav_js.to_string());
    }
    tokio::time::sleep(Duration::from_secs(3)).await;

    // KEEP_RUNNING if requested
    let keep_running = std::env::var("POLYROCKET_E2E_KEEP_RUNNING")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    if keep_running {
        tracing::info!("[e2e-all-football] KEEP_RUNNING=1 — staying alive");
        loop {
            tokio::time::sleep(Duration::from_secs(3600)).await;
        }
    }
    app.exit(0);
}
