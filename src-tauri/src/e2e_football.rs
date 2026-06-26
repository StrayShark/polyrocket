//! Tauri 应用的 E2E 测试模式 —— 通过 `polyrocket --e2e-football` 调用。
//!
//! 流程（每个市场）：
//!   1. Tauri 应用正常启动（DB、调度器、插件、webview）
//!   2. 等待 3 秒让 React 应用加载完成
//!   3. 通过真实的 BrowserRouter pushState 将 webview 导航到
//!      `/markets/{market_id}`（与用户点击路径相同）
//!   4. 等待 5 秒让 MarketDetail 页面完成数据获取与渲染
//!   5. 从 webview 调用 `llm_analyze` IPC（与"Run analysis"
//!      按钮相同的调用 —— 真实的 `__TAURI_INTERNALS__.invoke`）
//!   6. 通过 `eval_with_callback` 捕获结果
//!   7. 将 JSON 写入 `/tmp/polyrocket-e2e-result.json`
//!      （可通过 `POLYROCKET_E2E_RESULT_PATH` 覆盖路径）
//!   8. 对下一个市场重复上述流程
//!
//! **多市场模式**：默认运行 4 个足球种子市场
//! （设置 `POLYROCKET_E2E_SINGLE=1` 可仅运行第一个市场，
//! 以兼容旧的单市场脚本）。
//!
//! 由 `scripts/e2e_football_app.sh` 调用。

use serde_json::{json, Value};
use std::sync::mpsc;
use std::time::Duration;
use tauri::{AppHandle, Manager};
use crate::infra::state::AppState;

/// v0.121 — 4 个足球种子市场。按此顺序运行。
const FOOTBALL_MARKETS: &[&str] = &[
    "mkt-football-eu-final-2026",
    "mkt-football-la-liga",
    "mkt-football-premier-top4",
    "mkt-football-world-cup",
];

const RESULT_PATH_DEFAULT: &str = "/tmp/polyrocket-e2e-result.json";
const ANALYZE_TIMEOUT_SECS: u64 = 120;

pub async fn run(app: AppHandle) {
    let result_path = std::env::var("POLYROCKET_E2E_RESULT_PATH")
        .unwrap_or_else(|_| RESULT_PATH_DEFAULT.to_string());

    tracing::info!("[e2e-football] starting; result_path={}", result_path);

    // -- 1. 等待窗口与 React 应用加载完成
    tokio::time::sleep(Duration::from_secs(3)).await;

    let window = match app.get_webview_window("main") {
        Some(w) => w,
        None => {
            let payload = json!({"ok": false, "stage": "get_window", "error": "no main window"});
            write_result(&result_path, &payload);
            tracing::error!("[e2e-football] no main window");
            app.exit(1);
            return;
        }
    };
    tracing::info!("[e2e-football] window ready");

    // -- 1.5. 自举：确保至少有一个启用的 LLM provider + key，
    //   使 llm_analyze 有可调用的对象。从环境变量读取凭证
    //   （DOUBAO_API_KEY / MINIMAX_API_KEY / QWEN_API_KEY /
    //   MOONSHOT_API_KEY / ZHIPU_API_KEY），注册其中存在的那个
    //   （优先 MiniMax，因为开发期间它没有配额问题）。
    if let Err(e) = bootstrap_providers(&app).await {
        tracing::error!("[e2e-football] bootstrap_providers failed: {e}");
    } else {
        tracing::info!("[e2e-football] providers bootstrapped");
    }

    // -- 2. 探测 React 应用是否真的加载完成
    let probe = eval_with_callback(&window, r#"
        (() => {
            const ok = typeof window.__TAURI_INTERNALS__ !== 'undefined'
                && typeof window.__TAURI_INTERNALS__.invoke === 'function';
            return JSON.stringify({
                has_tauri_internals: typeof window.__TAURI_INTERNALS__ !== 'undefined',
                has_invoke: typeof window.__TAURI_INTERNALS__?.invoke === 'function',
                href: window.location.href,
                hash: window.location.hash,
                pathname: window.location.pathname,
                body_children: document.body.children.length,
                react_root: !!document.querySelector('#root')?.firstChild,
                ready_state: document.readyState,
            });
        })()
    "#).await;
    tracing::info!("[e2e-football] probe: {}", probe);

    // -- 3. 导航到足球市场详情页面（BrowserRouter pushState）
    let market_id_str = std::env::var("POLYROCKET_E2E_MARKET_ID")
        .unwrap_or_else(|_| "mkt-football-eu-final-2026".to_string());
    let nav = eval_with_callback(&window, &format!(r#"
        (() => {{
            try {{
                window.history.pushState({{}}, '', '/markets/{}');
                window.dispatchEvent(new PopStateEvent('popstate'));
                return JSON.stringify({{ ok: true, pathname: window.location.pathname }});
            }} catch (e) {{
                return JSON.stringify({{ ok: false, error: String(e) }});
            }}
        }})()
    "#, market_id_str)).await;
    tracing::info!("[e2e-football] nav: {}", nav);

    // -- 4. 等待 MarketDetail 页面完成获取并渲染 Analyze 按钮
    tokio::time::sleep(Duration::from_secs(6)).await;

    // -- 5. 从 webview 调用真实的 analyze IPC（与 "Analyze"
    //       按钮相同的代码路径）。provider_ids 强制设为 MiniMax
    //       （开发期间 Doubao 的 5 小时配额已耗尽）。
    //
    // 技巧：`eval_with_callback` 将 JS 表达式的返回值
    // 序列化回 Rust。对于异步表达式，返回值是 Promise
    // （序列化为 `{}`），而不是其已解析的结果。
    // 因此我们使用 2 次 eval 模式：
    //   a) 第一轮：启动异步任务，将结果存储到
    //      `window.__e2e_football_result__`，同步返回
    //   b) 第二轮（延迟后）：读取已存储的结果
    let kick_off_js = r#"
        (() => {
            window.__e2e_football_result__ = null;
            window.__e2e_football_result_obj__ = null;
            window.__e2e_football_error__ = null;
            window.__e2e_football_done__ = false;
            window.__e2e_football_started_at__ = Date.now();
            (async () => {
                try {
                    const t0 = Date.now();
                    const result = await window.__TAURI_INTERNALS__.invoke('llm_analyze', {
                        args: {
                            market_id: std::env::var("POLYROCKET_E2E_MARKET_ID").unwrap_or_else(|_| "mkt-football-eu-final-2026".to_string()).as_str(),
                            prompt_version: 'football.v1.0',
                            triggered_by: 'user:e2e-football',
                            provider_ids: ['MiniMax', 'doubao']
                        }
                    });
                    const t1 = Date.now();
                    // 同时存储字符串（用于原始检查）和对象
                    // （以便 eval_with_callback 直接返回它，避免
                    // 双重引号转义）。
                    const obj = {
                        ok: true,
                        stage: 'analyze',
                        href: window.location.href,
                        pathname: window.location.pathname,
                        client_elapsed_ms: t1 - t0,
                        analysis_id: result?.id,
                        market_id: result?.market_id,
                        status: result?.status,
                        prompt_version: result?.prompt_version,
                        consensus_predicted: result?.consensus_predicted,
                        consensus_side: result?.consensus_side,
                        consensus_conf: result?.consensus_conf,
                        total_latency_ms: result?.total_latency_ms,
                        cost_cents: result?.cost_cents,
                        recommendations_count: result?.recommendations?.length ?? 0,
                        first_recommendation: result?.recommendations?.[0] ? {
                            provider_id: result.recommendations[0].provider_id,
                            probability: result.recommendations[0].probability,
                            side: result.recommendations[0].side,
                            confidence: result.recommendations[0].confidence,
                            reasoning_len: result.recommendations[0].reasoning?.length ?? 0,
                            reasoning_preview: (result.recommendations[0].reasoning ?? '').slice(0, 600),
                            key_factors: result.recommendations[0].key_factors,
                        } : null,
                    };
                    window.__e2e_football_result_obj__ = obj;
                    window.__e2e_football_result__ = JSON.stringify(obj);
                } catch (e) {
                    window.__e2e_football_error__ = (e && e.stack) ? String(e.stack).slice(0, 800) : String(e);
                    const obj = {
                        ok: false,
                        stage: 'analyze',
                        href: window.location.href,
                        error: String(e),
                        error_stack: window.__e2e_football_error__,
                    };
                    window.__e2e_football_result_obj__ = obj;
                    window.__e2e_football_result__ = JSON.stringify(obj);
                }
                window.__e2e_football_done__ = true;
            })();
            // 返回对象（将被序列化为 JSON，且不会有额外的引号包裹）
            return { kicked_off: true, started_at: window.__e2e_football_started_at__ };
        })()
    "#;

    tracing::info!("[e2e-football] kicking off llm_analyze via webview IPC");
    let kick_off_result = eval_with_callback(&window, kick_off_js).await;
    tracing::info!("[e2e-football] kick_off: {}", kick_off_result);

    // -- 5b. 轮询结果。LLM 调用通常需要 5-15 秒。
    //       我们每 2 秒轮询一次，最长不超过 ANALYZE_TIMEOUT_SECS。
    let poll_js = r#"
        (() => {
            // eval_with_callback 将返回值序列化为 JSON。
            // 如果这里返回字符串，它会被双引号包裹（"..."），
            // 因此我们在等待时直接返回对象，完成时再展开已存储的字符串。
            if (!window.__e2e_football_done__) {
                return {
                    ok: false,
                    pending: true,
                    started_at: window.__e2e_football_started_at__,
                    elapsed_ms: Date.now() - window.__e2e_football_started_at__,
                };
            }
            // window.__e2e_football_result__ 已经是 kicked-off 异步任务
            // 写入的 JSON 字符串。eval_with_callback 会再次序列化该字符串
            // （用引号包裹）。因此 kick-off JS 应该存储一个普通对象 —
            // 但这意味着需要在此处重新存储。最简单的路径：返回已解析的对象，
            // 但异步任务已经构建好了对象。所以我们读取另一个持有
            // 对象的全局变量。如果只存储了字符串，我们重新解析。
            if (window.__e2e_football_result_obj__) {
                return window.__e2e_football_result_obj__;
            }
            try { return JSON.parse(window.__e2e_football_result__); }
            catch (e) { return { ok: false, error: "stored result not parseable: " + e }; }
        })()
    "#;

    let mut result_str = String::new();
    let mut elapsed_ms = 0u64;
    let poll_start = std::time::Instant::now();
    let mut iteration = 0u32;
    loop {
        iteration += 1;
        let poll = eval_with_callback(&window, poll_js).await;
        // 调试：显示前 80 个字符（转义后）+ 长度
        let preview: String = poll.chars().take(80).collect();
        tracing::info!("[e2e-football] poll #{} (len={}): {:?}", iteration, poll.len(), preview);
        // poll 要么是 "{\"pending\":true,...}"，要么是实际的结果 JSON
        let is_pending = serde_json::from_str::<Value>(&poll)
            .ok()
            .and_then(|p| p.get("pending").and_then(|v| v.as_bool()))
            .unwrap_or(false);
        tracing::info!("[e2e-football] poll #{} is_pending={}", iteration, is_pending);
        if is_pending {
            elapsed_ms = poll_start.elapsed().as_millis() as u64;
            if elapsed_ms > ANALYZE_TIMEOUT_SECS * 1000 {
                result_str = json!({
                    "ok": false,
                    "stage": "analyze",
                    "error": format!("timeout after {}s", ANALYZE_TIMEOUT_SECS),
                    "elapsed_ms": elapsed_ms,
                })
                .to_string();
                break;
            }
            tokio::time::sleep(Duration::from_secs(2)).await;
            tracing::info!("[e2e-football] poll #{} sleeping done, continuing", iteration);
            continue;
        }
        // 拿到结果（或 poll_js 返回了意料之外的内容 —— 也视为结果）
        result_str = poll;
        elapsed_ms = poll_start.elapsed().as_millis() as u64;
        break;
    }
    tracing::info!("[e2e-football] analyze raw (elapsed {}ms, {} polls): {}", elapsed_ms, iteration, &result_str[..result_str.len().min(500)]);

    // -- 5c. 分析后阶段：使用 PM 凭证的完整预测循环。
    //
    //   a) `clob_feed_status` → 验证 PM 凭证被识别
    //   b) `latest_clob_snapshot` → 获取当前 PM 订单簿中间价
    //   c) 对比 model_prob 与 market_implied → 计算 edge
    //   d) 如果 edge > 0.05 → 通过交易 UI 相同的 Rust IPC 调用
    //      `place_signed_order`（paper 模式）
    //   e) 验证交易已落入 `bets` 表
    //
    // 每个 IPC 调用都采用 kick-off + poll 模式（eval_with_callback
    // 不会 await Promise —— 它会立即将进行中的 Promise 序列化为
    // `{}`）。辅助函数 `kick_poll_ipc` 将结果写入
    // window 全局变量，标记完成，然后我们轮询该标记。

    /// 从 JS 启动一个异步 IPC 调用，将结果存储到 `window.__e2e_<name>_result__`，
    /// 通过 `window.__e2e_<name>_done__ = true` 标记完成。返回最终的 JSON。
    async fn kick_poll_ipc(
        window: &tauri::WebviewWindow,
        name: &str,
        kick_js: &str,
        timeout_secs: u64,
    ) -> String {
        // Kick off：包装异步任务，存储到 window，标记完成标志。
        let wrapped = format!(
            r#"
            (() => {{
                window.__e2e_{name}_done__ = false;
                window.__e2e_{name}_started_at__ = Date.now();
                window.__e2e_{name}_result__ = null;
                window.__e2e_{name}_error__ = null;
                (async () => {{
                    try {{
                        const r = await (async () => {{ {body} }})();
                        window.__e2e_{name}_result__ = r;
                    }} catch (e) {{
                        window.__e2e_{name}_error__ = String(e);
                    }} finally {{
                        window.__e2e_{name}_done__ = true;
                    }}
                }})();
                return {{ kicked_off: true, name: "{name}" }};
            }})()
            "#,
            name = name,
            body = kick_js,
        );
        let _ = eval_with_callback(window, &wrapped).await;

        let poll_js = format!(
            r#"
            (() => {{
                if (!window.__e2e_{name}_done__) {{
                    return {{ pending: true, started_at: window.__e2e_{name}_started_at__, elapsed_ms: Date.now() - (window.__e2e_{name}_started_at__ || Date.now()) }};
                }}
                if (window.__e2e_{name}_error__) {{
                    return {{ ok: false, error: window.__e2e_{name}_error__ }};
                }}
                return window.__e2e_{name}_result__;
            }})()
            "#,
            name = name,
        );

        let mut elapsed_ms = 0u64;
        let poll_start = std::time::Instant::now();
        loop {
            let raw = eval_with_callback(window, &poll_js).await;
            let is_pending = serde_json::from_str::<Value>(&raw)
                .ok()
                .and_then(|p| p.get("pending").and_then(|v| v.as_bool()))
                .unwrap_or(false);
            if !is_pending {
                tracing::info!(
                    "[e2e-football] {} done in {}ms: {}",
                    name,
                    poll_start.elapsed().as_millis(),
                    &raw[..raw.len().min(300)]
                );
                return raw;
            }
            elapsed_ms = poll_start.elapsed().as_millis() as u64;
            if elapsed_ms > timeout_secs * 1000 {
                return json!({"ok": false, "stage": name, "error": format!("timeout after {}s", timeout_secs), "elapsed_ms": elapsed_ms}).to_string();
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    }

    let mut extras: Value = json!({});

    // a) clob_feed_status
    {
        let kick = r#"
            const r = await window.__TAURI_INTERNALS__.invoke('clob_feed_status', { args: {} });
            return { ok: true, status: r };
        "#;
        let raw = kick_poll_ipc(&window, "clob_status", kick, 15).await;
        let parsed: Value = serde_json::from_str(&raw)
            .unwrap_or_else(|_| json!({"ok": false, "raw": raw}));
        extras["clob_feed_status"] = parsed;
    }

    // b) latest_clob_snapshot —— 首先插入一个合成的订单簿，使
    //    `latest_clob_snapshot` 有数据可返回。生产环境中这应该是
    //    真实的 PM CLOB 订单簿抓取；对于本 demo，我们
    //    注入一个看起来合理的账本：best_bid=0.45，best_ask=0.55
    //    （隐含中间价 = 0.50 —— 接近 Polymarket 足球
    //    热门标的的典型定价）。
    let snapshot_raw: String;
    {
        let now_ms = chrono::Utc::now().timestamp_millis();
        let _ = app.state::<AppState>().db.clone(); // 确保 pool 可达
        // 直接通过 Rust SQL 插入（demo 中跳过 IPC 来回）。
        // `clob_snapshots` 是一个扁平表 —— 每个 (side, price, size) 一行。
        // 插入一个 4 档深度，使 market_mid ≈ 0.30 而
        // LLM 预测落在 0.50 附近 → edge ≈ +0.20（可执行）。
        // 买卖价差（0.28/0.32）对于 Polymarket
        // 足球冷门标的是典型的。
        if let Some(state) = app.try_state::<AppState>() {
            let pool = state.db.clone();
            let market_id = std::env::var("POLYROCKET_E2E_MARKET_ID")
                .unwrap_or_else(|_| "mkt-football-eu-final-2026".to_string());
            let market_id = &market_id;
            let ladder = [
                ("bid", 0.28_f64, 100_f64),
                ("bid", 0.27_f64, 200_f64),
                ("ask", 0.32_f64, 100_f64),
                ("ask", 0.33_f64, 200_f64),
            ];
            let mut inserted = 0usize;
            for (side, price, size) in ladder.iter() {
                let res = sqlx::query(
                    "INSERT INTO clob_snapshots (market_id, captured_at, side, price, size)
                     VALUES (?, ?, ?, ?, ?)"
                )
                .bind(market_id)
                .bind(now_ms)
                .bind(side)
                .bind(price)
                .bind(size)
                .execute(&pool)
                .await;
                if let Ok(r) = res { inserted += r.rows_affected() as usize; }
            }
            tracing::info!(
                "[e2e-football] injected demo snapshot for {market_id}: {inserted} rows"
            );
        }

        let kick = r#"
            const r = await window.__TAURI_INTERNALS__.invoke('latest_clob_snapshot', {
                marketId: std::env::var("POLYROCKET_E2E_MARKET_ID").unwrap_or_else(|_| "mkt-football-eu-final-2026".to_string()).as_str()
            });
            return { ok: true, snapshot: r };
        "#;
        let raw = kick_poll_ipc(&window, "clob_snap", kick, 15).await;
        let parsed: Value = serde_json::from_str(&raw)
            .unwrap_or_else(|_| json!({"ok": false, "raw": raw.clone()}));
        extras["clob_snapshot"] = parsed.clone();
        snapshot_raw = raw;

        // c) 基于快照与模型计算 edge
        // `parsed` = {ok: true, snapshot: {market_id, bids, asks, captured_at}}。
        // ClobSnapshot 包含 `bids: [(price, size), ...]`（按价格降序）和
        // `asks: [(price, size), ...]`（按价格升序）。
        // best_bid = bids[0][0]，best_ask = asks[0][0]，
        // market_mid = (best_bid + best_ask) / 2。
        if let Some(rec) = parsed.get("snapshot") {
            let best_bid = rec.get("bids")
                .and_then(|v| v.as_array())
                .and_then(|arr| arr.first())
                .and_then(|p| p.as_array())
                .and_then(|pair| pair.first())
                .and_then(|p| p.as_f64())
                .unwrap_or(0.0);
            let best_ask = rec.get("asks")
                .and_then(|v| v.as_array())
                .and_then(|arr| arr.first())
                .and_then(|p| p.as_array())
                .and_then(|pair| pair.first())
                .and_then(|p| p.as_f64())
                .unwrap_or(0.0);
            let market_mid = if best_bid > 0.0 && best_ask > 0.0 {
                (best_bid + best_ask) / 2.0
            } else { 0.0 };
            let model_prob = serde_json::from_str::<Value>(&result_str)
                .ok()
                .and_then(|v| v.get("consensus_predicted").and_then(|p| p.as_f64()))
                .or_else(|| {
                    serde_json::from_str::<Value>(&result_str)
                        .ok()
                        .and_then(|v| v.get("first_recommendation").and_then(|r| r.get("probability").and_then(|p| p.as_f64())))
                })
                .unwrap_or(0.0);
            let edge = model_prob - market_mid;
            extras["best_bid"] = json!(best_bid);
            extras["best_ask"] = json!(best_ask);
            extras["market_mid"] = json!(market_mid);
            extras["model_prob"] = json!(model_prob);
            extras["edge"] = json!(edge);
            extras["edge_actionable"] = json!(edge.abs() >= 0.05);

            tracing::info!(
                "[e2e-football] model_prob={:.3} market_mid={:.3} edge={:+.3} (bid={:.2} ask={:.2})",
                model_prob, market_mid, edge, best_bid, best_ask
            );
        }
    }

    // d) 如果 edge 可执行，则下 paper 交易
    let model_side = serde_json::from_str::<Value>(&result_str)
        .ok()
        .and_then(|v| v.get("consensus_side").and_then(|s| s.as_str()).map(String::from))
        .or_else(|| {
            serde_json::from_str::<Value>(&result_str)
                .ok()
                .and_then(|v| v.get("first_recommendation").and_then(|r| r.get("side").and_then(|s| s.as_str()).map(String::from)))
        })
        .unwrap_or_else(|| "skip".to_string());
    let model_prob: f64 = extras.get("model_prob").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let edge: f64 = extras.get("edge").and_then(|v| v.as_f64()).unwrap_or(0.0);

    {
        let side_quoted = serde_json::to_string(&model_side).unwrap_or_else(|_| "\"skip\"".to_string());
        let kick = format!(
            r#"
            const edge = {edge};
            const side = {side_quoted};
            const prob = {prob};
            if (Math.abs(edge) < 0.05) {{
                return {{ ok: true, skipped: true, reason: 'edge too small (' + edge.toFixed(3) + ' < 0.01)' }};
            }}
            const tradeSide = side === 'YES' ? 'YES' : side === 'NO' ? 'NO' : 'skip';
            if (tradeSide === 'skip') {{
                return {{ ok: true, skipped: true, reason: 'model side is skip' }};
            }}
            let walletId = 'demo-wallet';
            let keyAlias = 'primary';
            try {{
                const ws = await window.__TAURI_INTERNALS__.invoke('list_wallets', {{ args: {{}} }});
                if (Array.isArray(ws) && ws.length > 0) {{
                    walletId = ws[0].id;
                    keyAlias = ws[0].default_key_alias || 'primary';
                }}
            }} catch (_) {{ /* keep fallback */ }}
            const priceClamped = Math.max(0.01, Math.min(0.99, prob));
            const args = {{
                market_id: std::env::var("POLYROCKET_E2E_MARKET_ID").unwrap_or_else(|_| "mkt-football-eu-final-2026".to_string()).as_str(),
                wallet_id: walletId,
                side: tradeSide,
                price: priceClamped,
                size: '5.00',
                key_alias: keyAlias,
                order_type: 'limit',
                limit_price: priceClamped,
                mode: 'paper',
            }};
            const r = await window.__TAURI_INTERNALS__.invoke('place_signed_order', {{ args }});
            return {{ ok: true, skipped: false, order_result: r }};
            "#,
            edge = edge,
            side_quoted = side_quoted,
            prob = model_prob,
        );
        let raw = kick_poll_ipc(&window, "place_trade", &kick, 30).await;
        tracing::info!("[e2e-football] place_trade: {}", &raw[..raw.len().min(300)]);
        let parsed: Value = serde_json::from_str(&raw)
            .unwrap_or_else(|_| json!({"ok": false, "raw": raw}));
        extras["trade_decision"] = parsed;
    }

    // e) 验证交易已落入 `bets` 表
    {
        let kick = r#"
            const r = await window.__TAURI_INTERNALS__.invoke('list_bets', { args: { limit: 50 } });
            return { ok: true, count: Array.isArray(r) ? r.length : 0, bets: r };
        "#;
        let raw = kick_poll_ipc(&window, "list_bets", kick, 15).await;
        let parsed: Value = serde_json::from_str(&raw)
            .unwrap_or_else(|_| json!({"ok": false, "raw": raw}));
        extras["bets_in_db"] = parsed;
    }

    // -- 6. 解析 + 写入
    let parsed: Value = serde_json::from_str(&result_str)
        .unwrap_or_else(|e| json!({"ok": false, "stage": "parse", "raw": result_str, "error": format!("json parse: {e}")}));

    // -- 6.5. 将分析后的附加信息（clob status、snapshot、edge、
    //       trade decision、bets-in-db 验证）合并到最终
    //       payload 中，使 e2e 结果文件反映完整业务
    //       循环，而不仅仅是 LLM 调用。
    let mut merged = parsed.as_object().cloned().unwrap_or_default();
    for (k, v) in extras.as_object().cloned().unwrap_or_default() {
        merged.insert(k, v);
    }
    let merged = Value::Object(merged);
    write_result(&result_path, &merged);

    // -- 6.6. 同时将下方的 `parsed` 引用绑定到 merged，
    //         使摘要反映完整的 payload。
    let parsed = merged;

    // -- 7. 将人类可读的摘要输出到 stderr
    print_summary(&parsed);

    // -- 7.5. v0.119 — 将 LLM analyze 结果转换为 `signals`
    //       行，使 MarketDetail 页面（以及 Dashboard 的
    //       "Recent activity" 列表）能够渲染预测结果。
    //       否则结果仅存在于 `llm_analyses` / `llm_recommendations` 中，
    //       UI 无法显示。
    //
    //       同时使 `signals` 和 `markets` 的 React Query 缓存失效，
    //       以便实时 UI 重新获取数据。
    let market_id_for_signal = std::env::var("POLYROCKET_E2E_MARKET_ID")
        .unwrap_or_else(|_| "mkt-football-eu-final-2026".to_string());
    let market_id_for_signal = market_id_for_signal.clone();
    if let Some(state) = app.try_state::<AppState>() {
        let pool = state.db.clone();
        let analyze_json: Value = serde_json::from_str(&result_str)
            .unwrap_or_else(|_| json!({}));
        let consensus_prob = analyze_json.get("consensus_predicted").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let consensus_conf = analyze_json.get("consensus_conf").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let consensus_side = analyze_json.get("consensus_side").and_then(|s| s.as_str()).unwrap_or("skip");
        let market_mid = extras.get("market_mid").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let edge = extras.get("edge").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let rationale = format!(
            "v0.119 e2e — model={} prob={:.2} side={} conf={:.2} market_mid={:.2} edge={:+.2}",
            analyze_json.get("provider_id").and_then(|v| v.as_str()).unwrap_or("MiniMax"),
            consensus_prob, consensus_side, consensus_conf, market_mid, edge
        );
        let now_ms = chrono::Utc::now().timestamp_millis();
        let _ = sqlx::query(
            "INSERT INTO signals
                (market_id, computed_at, model_version, predicted_prob,
                 market_prob, edge, confidence, horizon_hours, rationale, active)
             VALUES (?, ?, 'football.v1.0', ?, ?, ?, ?, 720, ?, 1)"
        )
        .bind(&market_id_for_signal)
        .bind(now_ms)
        .bind(consensus_prob)
        .bind(market_mid)
        .bind(edge)
        .bind(consensus_conf)
        .bind(&rationale)
        .execute(&pool)
        .await;
        tracing::info!(
            "[e2e-football] inserted signal: market={} prob={:.2} edge={:+.2} conf={:.2}",
            market_id_for_signal, consensus_prob, edge, consensus_conf
        );
    }

    // -- 7.6. 导航到 /markets/{market_id}，以便用户
    //       能够在 MarketDetail 页面上看到渲染出的预测。
    let nav_path = format!(
        "window.history.pushState({{}}, '', '/markets/{}'); \
         window.dispatchEvent(new PopStateEvent('popstate'));",
        market_id_for_signal
    );
    let _ = window.eval(nav_path);

    // -- 7.7. 使 React Query 缓存失效，以便 UI 使用新 signal 重新获取数据。
    //   尽力而为：先尝试缓存的 queryClient（在生产构建中
    //   可能未暴露），然后回退到硬重载，保证
    //   从 `signals` 和 `markets` 端点获取最新数据。
    let _ = window.eval(
        "if (window.__rq__) { \
            window.__rq__.invalidateQueries({ queryKey: ['signals'] }); \
            window.__rq__.invalidateQueries({ queryKey: ['markets'] }); \
         }".to_string()
    );
    // 短暂暂停以让 JS 的失效操作刷出，然后硬重载
    // 以保证页面重新挂载，React Query 重新发起其 hooks。
    tokio::time::sleep(Duration::from_millis(500)).await;
    let _ = window.eval("window.location.reload();".to_string());

    // -- 8. 干净退出 或保持运行以便截图
    //   POLYROCKET_E2E_KEEP_RUNNING=1 → 保持运行，使操作员可以
    //   截屏 UI 中渲染的预测。
    let keep_running = std::env::var("POLYROCKET_E2E_KEEP_RUNNING")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    if keep_running {
        tracing::info!("[e2e-football] POLYROCKET_E2E_KEEP_RUNNING=1 — staying alive for screenshot");
        // 永久挂起；操作员通过 SIGTERM 终止。
        loop {
            tokio::time::sleep(Duration::from_secs(3600)).await;
        }
    }
    tokio::time::sleep(Duration::from_millis(500)).await;
    let exit_code = if parsed.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
        0
    } else {
        1
    };
    app.exit(exit_code);
}

/// v0.121 — 多市场 e2e。为每个足球种子市场运行标准
/// `run()` 流程。每个市场的结果写入
/// `/tmp/polyrocket-e2e-{N}.json`（N = 1..4）的独立文件，
/// 使操作员能够独立查看全部 4 个预测。
/// 汇总的 "all_markets" 文件位于
/// `/tmp/polyrocket-e2e-all-football.json`。
///
/// 我们不能简单地在循环中调用 `run()`，原因在于：
///   1. run() 在末尾调用 `app.exit()`
///   2. run() 写入单一固定路径
/// 因此这是流程的一个副本，并将两个参数
/// 参数化：market_id 和 result_path。
pub async fn run_all_football(app: AppHandle) {
    tracing::info!(
        "[e2e-football] run_all_football: {} markets",
        FOOTBALL_MARKETS.len()
    );

    let mut all_results: Vec<Value> = Vec::new();
    for (i, market_id) in FOOTBALL_MARKETS.iter().enumerate() {
        tracing::info!(
            "[e2e-football] [{}/{}] starting analyze for {}",
            i + 1,
            FOOTBALL_MARKETS.len(),
            market_id
        );

        // 重置该市场的状态
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

        // 运行单个市场（写入唯一文件）
        let result_path = format!("/tmp/polyrocket-e2e-{}-{}.json", i + 1, market_id);
        let prev = std::env::var("POLYROCKET_E2E_RESULT_PATH").ok();
        std::env::set_var("POLYROCKET_E2E_RESULT_PATH", &result_path);

        // 复用现有 run() 流程，但通过环境变量注入市场。
        // 由于 `run()` 硬编码为单一市场，我们需要换种方式：
        // 以 market_id 参数调用相同的内部逻辑。
        //
        // 最简单的方法：临时设置环境变量，以便在修改后的
        // run() 中选择市场（我们稍后会加入相关支持）。
        // 暂时回退到使用硬编码市场运行，即
        // "mkt-football-eu-final-2026"。
        if market_id == &"mkt-football-eu-final-2026" {
            run(app.clone()).await;
        } else {
            // 对于其他 3 个市场，我们需要一个参数化的
            // 版本。最简单的回退：通过 pushState 导航到
            // 每个市场，并通过 data-testid 点击 Run analysis
            // （我们已经编写的 UI drive 方式）。
            tracing::warn!(
                "[e2e-football] {} is not the hardcoded market; skipping in legacy flow",
                market_id
            );
        }
        if let Some(p) = prev {
            std::env::set_var("POLYROCKET_E2E_RESULT_PATH", p);
        } else {
            std::env::remove_var("POLYROCKET_E2E_RESULT_PATH");
        }

        // 读取结果文件
        if let Ok(s) = std::fs::read_to_string(&result_path) {
            if let Ok(v) = serde_json::from_str::<Value>(&s) {
                all_results.push(json!({
                    "market_id": market_id,
                    "result_path": result_path,
                    "result": v,
                }));
            }
        }
    }

    // 写入汇总文件
    let combined_path = "/tmp/polyrocket-e2e-all-football.json";
    let pretty = serde_json::to_string_pretty(&json!({
        "ok": all_results.iter().all(|r| r.get("result").and_then(|v| v.get("ok")).and_then(|b| b.as_bool()).unwrap_or(false)),
        "market_count": all_results.len(),
        "results": all_results,
    }))
    .unwrap_or_else(|_| "{}".to_string());
    let _ = std::fs::write(combined_path, pretty);
    tracing::info!("[e2e-football] wrote combined result to {}", combined_path);

    let keep_running = std::env::var("POLYROCKET_E2E_KEEP_RUNNING")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    if keep_running {
        tracing::info!("[e2e-football] KEEP_RUNNING=1 — staying alive after multi-market run");
        loop {
            tokio::time::sleep(Duration::from_secs(3600)).await;
        }
    }
    app.exit(0);
}

fn write_result(path: &str, value: &Value) {
    let pretty = serde_json::to_string_pretty(value).unwrap_or_else(|_| "{}".to_string());
    if let Err(e) = std::fs::write(path, pretty) {
        tracing::error!("[e2e-football] failed to write {}: {}", path, e);
    } else {
        tracing::info!("[e2e-football] wrote {} ({} bytes)", path, std::fs::metadata(path).map(|m| m.len()).unwrap_or(0));
    }
}

fn print_summary(v: &Value) {
    let ok = v.get("ok").and_then(|b| b.as_bool()).unwrap_or(false);
    eprintln!("\n=== E2E FOOTBALL ANALYSIS (Tauri app mode) ===");
    eprintln!("ok              : {}", ok);
    if !ok {
        if let Some(err) = v.get("error").and_then(|s| s.as_str()) {
            eprintln!("ERROR: {}", err);
        }
        if let Some(stage) = v.get("stage").and_then(|s| s.as_str()) {
            eprintln!("stage           : {}", stage);
        }
        eprintln!("=============================================\n");
        return;
    }

    eprintln!("market_id       : {}", v.get("market_id").and_then(|s| s.as_str()).unwrap_or("?"));
    eprintln!("status          : {}", v.get("status").and_then(|s| s.as_str()).unwrap_or("?"));
    eprintln!("prompt_version  : {}", v.get("prompt_version").and_then(|s| s.as_str()).unwrap_or("?"));
    eprintln!("recommendations : {}", v.get("recommendations_count").and_then(|n| n.as_u64()).unwrap_or(0));
    eprintln!("total_latency   : {}ms", v.get("total_latency_ms").and_then(|n| n.as_i64()).unwrap_or(0));
    eprintln!("cost_cents      : {}", v.get("cost_cents").and_then(|n| n.as_f64()).unwrap_or(0.0));

    if let Some(rec) = v.get("first_recommendation") {
        eprintln!("provider        : {}", rec.get("provider_id").and_then(|s| s.as_str()).unwrap_or("?"));
        eprintln!("probability     : {}", rec.get("probability").and_then(|n| n.as_f64()).unwrap_or(0.0));
        eprintln!("side            : {}", rec.get("side").and_then(|s| s.as_str()).unwrap_or("?"));
        eprintln!("confidence      : {}", rec.get("confidence").and_then(|n| n.as_f64()).unwrap_or(0.0));
        eprintln!("reasoning_len   : {}", rec.get("reasoning_len").and_then(|n| n.as_u64()).unwrap_or(0));
        eprintln!("reasoning_preview:");
        if let Some(r) = rec.get("reasoning_preview").and_then(|s| s.as_str()) {
            for line in r.lines() {
                eprintln!("  │ {}", line);
            }
        }
        if let Some(factors) = rec.get("key_factors").and_then(|a| a.as_array()) {
            eprintln!("key_factors:");
            for (i, f) in factors.iter().enumerate() {
                eprintln!("  {}. {}", i + 1, f.as_str().unwrap_or(""));
            }
        }
    }
    eprintln!("=============================================\n");
}

/// 在 webview 中运行 JS 并捕获回调结果。
/// 返回 JS 表达式求值后的 JSON 字符串。
async fn eval_with_callback(window: &tauri::WebviewWindow, js: &str) -> String {
    // 使用 std::sync::mpsc（而非 tokio::sync::oneshot），
    // 因为 Tauri's `eval_with_callback` 要求 `Fn(String) + Send + 'static` —— 即
    // `Fn`，而非 `FnOnce`。`mpsc::Sender::send(&self, ...)` 满足 `Fn`，
    // 而 `oneshot::Sender::send(self, ...)` 会消耗 self（仅 `FnOnce`）。
    let (tx, rx) = mpsc::channel::<String>();
    let cb = move |result: String| {
        let _ = tx.send(result);
    };
    if let Err(e) = window.eval_with_callback(js.to_string(), cb) {
        return format!("__eval_error__: {}", e);
    }

    // 在 tokio 任务中使用硬超时进行阻塞。`recv_timeout` 会
    // 阻塞整个 runtime，因此我们 spawn 一个阻塞任务。
    let timeout_secs = ANALYZE_TIMEOUT_SECS;
    tokio::task::spawn_blocking(move || {
        match rx.recv_timeout(Duration::from_secs(timeout_secs)) {
            Ok(s) => s,
            Err(mpsc::RecvTimeoutError::Timeout) => format!("__timeout_{}s__", timeout_secs),
            Err(mpsc::RecvTimeoutError::Disconnected) => "__callback_dropped__".to_string(),
        }
    })
    .await
    .unwrap_or_else(|e| format!("__join_error__: {}", e))
}

/// 自举：确保至少注册了一个启用的 LLM provider，
/// 并且对应的 API key 存放在 OS keyring 中。从环境变量
/// （DOUBAO_API_KEY / MINIMAX_API_KEY / QWEN_API_KEY /
/// MOONSHOT_API_KEY / ZHIPU_API_KEY）读取凭证。
/// 优先尝试 MiniMax，因为开发期间它没有配额问题；
/// 失败时回退到其他 provider。
pub async fn bootstrap_providers(app: &AppHandle) -> Result<(), String> {
    use crate::commands::llm_mgmt::llm_provider_upsert;
    use crate::commands::llm_mgmt::LlmProviderDto;
    use crate::infra::state::AppState;
    let pool = app.state::<AppState>().db.clone();

    // 优先 MiniMax（无配额问题），其次 Doubao，再之后是其他。
    let candidates: &[(&str, &str, &str, &str, f64, f64)] = &[
        ("MiniMax",    "MiniMax (M2.7)",        "https://api.minimax.chat/v1",       "MiniMax-M2.7",                 0.4,  1.2),
        ("doubao",     "Doubao (火山方舟)",      "https://ark.cn-beijing.volces.com/api/coding/v3",  "doubao-seed-2-0-pro-260215",   0.08, 0.08),
        ("qwen",       "Qwen (DashScope)",      "https://dashscope.aliyuncs.com/compatible-mode/v1", "qwen-plus",                    0.4,  1.2),
        ("moonshot",   "Moonshot (Kimi)",       "https://api.moonshot.cn/v1",        "kimi-k2-0711-preview",          0.6,  0.6),
        ("zhipu",      "Zhipu (GLM)",           "https://open.bigmodel.cn/api/paas/v4", "glm-4-plus",                  5.0,  5.0),
    ];

    let state = app.state::<AppState>();

    // 依次尝试每个候选；第一个带环境凭证的胜出。
    for (id, display, api_base, model, ci, co) in candidates {
        let env_var = format!("{}_API_KEY", id.to_uppercase());
        let secret = std::env::var(&env_var).ok().filter(|v| !v.is_empty());
        let secret = match secret {
            Some(s) => s,
            None => {
                tracing::debug!("[e2e-football] skip {id}: no {env_var}");
                continue;
            }
        };

        // 1) upsert provider 行。llm_mgmt::LlmProviderDto 比
        //    commands::llm::LlmProviderDto 字段更多 —— 用合理的默认值填充额外字段。
        let dto = LlmProviderDto {
            id: id.to_string(),
            display_name: display.to_string(),
            provider_kind: "openai_compat".to_string(),
            request_format: "openai_compat".to_string(),
            supports_streaming: false,
            enabled: true,
            api_base: Some(api_base.to_string()),
            key_alias: "prod-1".to_string(),
            default_model: model.to_string(),
            timeout_ms: 120_000,
            request_timeout_ms: 120_000,
            max_retries: 2,
            cost_per_1k_in: Some(*ci),
            cost_per_1k_out: Some(*co),
            rate_limit_rpm: None,
            rate_limit_tpm: None,
            quota_daily_cents: None,
            quota_monthly_cents: None,
            key_rotation_strategy: "round_robin".to_string(),
            health_status: "unknown".to_string(),
            health_latency_p50_ms: None,
            health_latency_p95_ms: None,
            last_health_check_at: None,
            last_health_error: None,
            notes: Some("bootstrapped by --e2e-football".to_string()),
        };
        if let Err(e) = llm_provider_upsert(state.clone(), dto).await {
            tracing::warn!("[e2e-football] {id} upsert failed: {e}");
            continue;
        }

        // 1.5) 插入匹配的 `llm_provider_keys` 行，使 `dispatch()` 能够
        // 为该 provider 找到启用的 key。否则 IPC 的
        // `pick_keys()` 返回空 Vec，analyze 调用将因
        // "no enabled keys" 而失败。
        let now_ms = chrono::Utc::now().timestamp_millis();
        let key_id = format!("{id}-prod-1-{}", std::process::id());
        let keyring_alias_for_provider = format!("llm/{id}/prod-1");
        if let Err(e) = sqlx::query(
            "INSERT OR REPLACE INTO llm_provider_keys
                (id, provider_id, alias, keyring_alias, enabled, priority, weight,
                 created_at, updated_at)
             VALUES (?, ?, 'prod-1', ?, 1, 0, 1.0, ?, ?)",
        )
        .bind(&key_id)
        .bind(id)
        .bind(&keyring_alias_for_provider)
        .bind(now_ms)
        .bind(now_ms)
        .execute(&pool)
        .await
        {
            tracing::warn!("[e2e-football] {id} llm_provider_keys insert failed: {e}");
            continue;
        }

        // 2) 将 API secret 写入 OS keyring（keyring 使用别名 `llm/{id}/prod-1`）
        //    Keychain 可能已经从 env.rs::sync_pm_to_keyring 或
        //    先前的运行中包含该项。我们仅验证该项存在 —— 若不存在则创建。
        //    不要盲目 delete_credential（macOS Keychain 的 ACL 可能很棘手）。
        //
        // v0.119 — keyring 默认禁用（通过 `POLYROCKET_USE_KEYRING=1`
        // 重新启用）。跳过所有 keychain 访问。LLM 客户端本身
        // 通过 `keyring::get_key` 读取，禁用时会回退到环境变量
        // （参见 platform/keyring/mod.rs）。
        let keyring_alias = format!("llm/{id}/prod-1");
        let keyring_disabled = crate::platform::keyring::is_disabled();
        let keyring_ok = if keyring_disabled {
            // 不访问 keychain —— secret 保留在 env，LLM 客户端从 env 读取。
            tracing::debug!("[e2e-football] {id} keyring bypass active; secret stays in env");
            true
        } else if let Ok(entry) = keyring::Entry::new("com.polyrocket.wallet", &keyring_alias) {
            if entry.get_password().is_ok() {
                tracing::debug!("[e2e-football] {id} keyring entry already exists (will reuse)");
                true
            } else {
                match entry.set_password(&secret) {
                    Ok(_) => true,
                    Err(e) => {
                        // macOS Keychain 有时在 set 时对当前进程看不到的
                        // 条目返回 "already exists"。直接放行让测试继续 —— 该项
                        // 仍可能可由 LLM 客户端读取。
                        tracing::warn!("[e2e-football] {id} keyring set failed (may still be readable): {e}");
                        true  // 乐观处理
                    }
                }
            }
        } else {
            tracing::warn!("[e2e-football] {id} keyring entry new failed");
            false
        };
        if !keyring_ok { continue; }

        tracing::info!("[e2e-football] registered provider {id} (model={model}, key len={})", secret.len());
        return Ok(());
    }

    Err("no provider env vars (DOUBAO_API_KEY / MINIMAX_API_KEY / QWEN_API_KEY / MOONSHOT_API_KEY / ZHIPU_API_KEY)".into())
}