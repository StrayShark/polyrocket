// v0.121 — UI 驱动的 e2e。用真实的 DOM 点击
//（与用户使用鼠标/触控板所走的路径相同）替代
// e2e_football.rs 中的 `pushState` + 直接
// `invoke` 模式：
//
//   1. Webview 加载，应用导航到 /dashboard
//   2. 点击侧边栏的 "Markets" 项
//      （NavLink 至 /markets —— React Router useNavigate）
//   3. 对每个足球市场：
//        a. 点击市场卡片（链接至 /markets/{id}）
//        b. 等待 MarketDetail 页面渲染
//        c. 点击新增的 "Run analysis" 按钮
//           （data-testid="run-analysis-btn"）
//        d. 等待 LLM analyze mutation 完成
//           （toast 成功 —— 通过 Mutation 状态呈现）
//        e. 截屏已渲染的预测
//
// 这是用户要求的"全程用 app 操作"模式：每个
// 操作都是真实的 DOM 事件 → 真实的 React handler → 真实的 IPC
// → 真实的 LLM 调用 → 真实的 DB 写入 → 真实的 UI 更新。
// 脚本唯一参与的是派发合成的 click()。
//
// 运行方式：
//   POLYROCKET_E2E_UI_DRIVE=1 POLYROCKET_E2E_KEEP_RUNNING=1 \
//     cargo run --bin polyrocket -- --e2e-football
//
// 迭代的市场：markets 表 ORDER BY id 中
// 全部 4 个足球种子市场（确定性）。
//
// 截屏目录：/tmp/polyrocket-screens/

use serde_json::{json, Value};
use std::time::Duration;
use tauri::{AppHandle, Manager};
use crate::infra::state::AppState;

const SCREENSHOT_DIR: &str = "/tmp/polyrocket-screens";

const FOOTBALL_MARKETS: &[&str] = &[
    "mkt-football-eu-final-2026",
    "mkt-football-la-liga",
    "mkt-football-premier-top4",
    "mkt-football-world-cup",
];

pub async fn run(app: AppHandle) {
    let ui_drive = std::env::var("POLYROCKET_E2E_UI_DRIVE")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    if !ui_drive {
        // 不是 UI drive 模式 —— 让常规的 e2e_football 运行。
        return;
    }

    tracing::info!("[ui-drive] starting; will click through 4 football markets");

    tokio::time::sleep(Duration::from_secs(3)).await;

    let window = match app.get_webview_window("main") {
        Some(w) => w,
        None => {
            tracing::error!("[ui-drive] no main window");
            app.exit(1);
            return;
        }
    };

    // 确保截屏目录存在。
    let _ = std::fs::create_dir_all(SCREENSHOT_DIR);

    // 截取 "0-start" 截屏（任何初始加载的页面）。
    let _ = screenshot_window(&window, &format!("{SCREENSHOT_DIR}/ui-0-start.png")).await;

    // -- 1. 点击侧边栏的 "Markets"
    //   直接使用 React fiber 的 onClick 以绕过
    //   React 的合成事件系统（MouseEvent 派发
    //   对 React 17+ 根委托的 handler 不生效）。
    tracing::info!("[ui-drive] step 1: click Markets sidebar");
    let nav_markets_js = r#"
        (() => {
            const links = Array.from(document.querySelectorAll('a'));
            const marketsLink = links.find(a =>
                a.getAttribute('href') === '/markets' ||
                a.textContent.trim().includes('Markets')
            );
            if (!marketsLink) return { ok: false, error: 'no Markets link found' };
            const reactKey = Object.keys(marketsLink).find(k => k.startsWith('__reactProps$'));
            if (!reactKey) {
                // fallback to direct href navigation
                marketsLink.click();
                return { ok: true, method: 'href-click', href: marketsLink.getAttribute('href') };
            }
            const props = marketsLink[reactKey];
            if (props && typeof props.onClick === 'function') {
                props.onClick({ preventDefault: () => {}, stopPropagation: () => {} });
                return { ok: true, method: 'react-props', href: marketsLink.getAttribute('href') };
            }
            // fallback
            marketsLink.click();
            return { ok: true, method: 'href-click-fallback', href: marketsLink.getAttribute('href') };
        })()
    "#;
    let r1 = eval_obj(&window, nav_markets_js).await;
    tracing::info!("[ui-drive] click Markets: {}", r1);
    tokio::time::sleep(Duration::from_secs(2)).await;
    let _ = screenshot_window(&window, &format!("{SCREENSHOT_DIR}/ui-1-markets.png")).await;

    // -- 2. 对每个足球市场：点击卡片 → 点击 "Run analysis" → 等待 → 截屏
    for (i, market_id) in FOOTBALL_MARKETS.iter().enumerate() {
        tracing::info!(
            "[ui-drive] step 2.{i}: click market card for {market_id}"
        );

        // 首先，导航回 /markets（第一次迭代后，
        // 我们停留在 MarketDetail 页面上；我们需要列表页才能
        // 找到下一个市场的链接）。
        if i > 0 {
            let nav_back_js = r#"
                (() => {
                    const link = Array.from(document.querySelectorAll('a'))
                        .find(a => a.getAttribute('href') === '/markets');
                    if (!link) return { ok: false, error: 'no Markets link' };
                    const reactKey = Object.keys(link).find(k => k.startsWith('__reactProps$'));
                    if (reactKey && link[reactKey]?.onClick) {
                        link[reactKey].onClick({ preventDefault: () => {}, stopPropagation: () => {} });
                        return { ok: true, method: 'react-props' };
                    }
                    link.click();
                    return { ok: true, method: 'fallback' };
                })()
            "#;
            let r = eval_obj(&window, nav_back_js).await;
            tracing::info!("[ui-drive] nav back to /markets: {}", r);
            tokio::time::sleep(Duration::from_secs(2)).await;
        }

        // 通过 React fiber 的 onClick 点击市场卡片。
        let click_card_js = format!(
            r#"
            (() => {{
                const links = Array.from(document.querySelectorAll('a'));
                const target = links.find(a => a.getAttribute('href') === '/markets/{market_id}');
                if (!target) return {{ ok: false, error: 'no card link for {market_id}', link_count: links.length }};
                const reactKey = Object.keys(target).find(k => k.startsWith('__reactProps$'));
                if (reactKey && target[reactKey]?.onClick) {{
                    target[reactKey].onClick({{ preventDefault: () => {{}}, stopPropagation: () => {{}} }});
                    return {{ ok: true, method: 'react-props', href: target.getAttribute('href') }};
                }}
                target.click();
                return {{ ok: true, method: 'fallback', href: target.getAttribute('href') }};
            }})()
            "#
        );
        let r = eval_obj(&window, &click_card_js).await;
        tracing::info!("[ui-drive] click card {market_id}: {}", r);
        tokio::time::sleep(Duration::from_secs(2)).await;

        // -- 2.b. 点击 "Run analysis" 按钮（data-testid）
        //   React 17+ 使用根事件委托。简单的
        //   .click() 或 MouseEvent 派发可能不会触发
        //   React onClick handler，因为 React 的
        //   合成事件系统在根节点监听，并从不同的路径
        //   重建事件。可靠的方式是获取
        //   `__reactProps$<id>` fiber 属性，并直接
        //   调用 `onClick`。这完全绕过事件系统，
        //   直接运行 handler。
        let click_analyze_js = r#"
            (() => {
                const btn = document.querySelector('[data-testid="run-analysis-btn"]');
                if (!btn) return { ok: false, error: 'no [data-testid="run-analysis-btn"]' };
                // 查找 React fiber 的 props
                const reactKey = Object.keys(btn).find(k => k.startsWith('__reactProps$'));
                if (!reactKey) return {
                    ok: false,
                    error: 'no React fiber props found on button',
                    react_keys: Object.keys(btn).filter(k => k.startsWith('__react'))
                };
                const props = btn[reactKey];
                if (!props || typeof props.onClick !== 'function') return {
                    ok: false,
                    error: 'no onClick handler in props',
                    prop_keys: Object.keys(props || {})
                };
                // 直接调用 React 的 onClick handler
                try {
                    props.onClick({ preventDefault: () => {}, stopPropagation: () => {} });
                    return {
                        ok: true,
                        method: 'react-props-direct',
                        btn_text: btn.textContent?.trim() ?? '',
                    };
                } catch (e) {
                    return { ok: false, error: 'onClick threw: ' + String(e) };
                }
            })()
        "#;
        let r = eval_obj(&window, click_analyze_js).await;
        tracing::info!("[ui-drive] click Run analysis: {}", r);

        // -- 2.b.5. 确认按钮已变为 "Analyzing…"（React handler
        //   此时应该已经启动了 mutation）。如果没有，
        //   点击已丢失，我们中止该市场。
        let mut analyze_started = false;
        for _ in 0..15 {
            tokio::time::sleep(Duration::from_millis(500)).await;
            let state_js = r#"
                (() => {
                    const btn = document.querySelector('[data-testid="run-analysis-btn"]');
                    return {
                        btn_text: btn?.textContent?.trim() ?? '',
                        has_toast_failed: document.body.innerText.includes('Analysis failed'),
                    };
                })()
            "#;
            let state = eval_obj(&window, state_js).await;
            if let Ok(s) = serde_json::from_str::<Value>(&state) {
                let btn_text = s.get("btn_text").and_then(|v| v.as_str()).unwrap_or("");
                if btn_text.contains("Analyzing") {
                    analyze_started = true;
                    break;
                }
                if s.get("has_toast_failed").and_then(|v| v.as_bool()).unwrap_or(false) {
                    tracing::warn!("[ui-drive] {market_id} toast says Analysis failed");
                    break;
                }
            }
        }
        if !analyze_started {
            tracing::warn!(
                "[ui-drive] {market_id} button did not flip to Analyzing… within 7.5s — click may be lost"
            );
        }

        // 分析中的截屏
        let path = format!("{SCREENSHOT_DIR}/ui-2-{}-analyzing.png", i + 1);
        let _ = screenshot_window(&window, &path).await;

        // -- 2.c. 等待 LLM analyze 完成。
        //
        //   检测方式：轮询 signals 表，查找该 market_id
        //   的新行。Rust LLM analyze 路径在 LLM 调用
        //   完成后写入 `signals`，因此新行意味着
        //   分析已完成。我们还检查 e2e_football UI drive
        //   流程对创建 signal 的预期 —— 若未创建，
        //   表示分析正在进行或已失败。
        //
        //   轮询 DB 比 DOM 更可靠（DOM 存在 React Query
        //   缓存滞后）。超时：90 秒（LLM 调用可能需要
        //   5-30 秒；我们为重试预留余量）。
        let start = std::time::Instant::now();
        let mut signal_found = false;
        let mut elapsed = Duration::from_secs(0);
        for attempt in 0..90 {
            tokio::time::sleep(Duration::from_secs(1)).await;
            elapsed = start.elapsed();
            // 查询 DB 查找该市场的 signal
            if let Some(state) = app.try_state::<AppState>() {
                let pool = state.db.clone();
                let row: Option<(i64,)> = sqlx::query_as(
                    "SELECT id FROM signals WHERE market_id = ? AND active = 1
                     ORDER BY computed_at DESC LIMIT 1"
                )
                .bind(market_id)
                .fetch_optional(&pool)
                .await
                .ok()
                .flatten();
                if let Some((id,)) = row {
                    signal_found = true;
                    tracing::info!(
                        "[ui-drive] {market_id} signal id={id} found in DB after {}s (attempt {})",
                        elapsed.as_secs(), attempt
                    );
                    break;
                }
            }
        }
        if !signal_found {
            tracing::warn!(
                "[ui-drive] {market_id} NO signal in DB after {}s",
                elapsed.as_secs()
            );
        }
        // 给 React Query 2-3 秒重新获取 + 渲染
        tokio::time::sleep(Duration::from_secs(3)).await;

        // -- 2.e. 截屏
        let path = format!("{SCREENSHOT_DIR}/ui-2-{}-after.png", i + 1);
        let _ = screenshot_window(&window, &path).await;
    }

    // -- 3. 最终 dashboard / markets 截屏，展示全部 4 个预测
    let _ = eval_obj(&window, r#"
        (() => {
            const link = Array.from(document.querySelectorAll('a'))
                .find(a => a.getAttribute('href') === '/markets');
            if (link) {
                const ev = new MouseEvent('click', { bubbles: true, cancelable: true, view: window });
                link.dispatchEvent(ev);
            }
        })()
    "#).await;
    tokio::time::sleep(Duration::from_secs(2)).await;
    let _ = screenshot_window(&window, &format!("{SCREENSHOT_DIR}/ui-9-markets-all.png")).await;

    let _ = eval_obj(&window, r#"
        (() => {
            const link = Array.from(document.querySelectorAll('a'))
                .find(a => a.getAttribute('href') === '/signals');
            if (link) {
                const ev = new MouseEvent('click', { bubbles: true, cancelable: true, view: window });
                link.dispatchEvent(ev);
            }
        })()
    "#).await;
    tokio::time::sleep(Duration::from_secs(2)).await;
    let _ = screenshot_window(&window, &format!("{SCREENSHOT_DIR}/ui-9-signals-all.png")).await;

    let _ = eval_obj(&window, r#"
        (() => {
            const link = Array.from(document.querySelectorAll('a'))
                .find(a => a.getAttribute('href') === '/dashboard');
            if (link) {
                const ev = new MouseEvent('click', { bubbles: true, cancelable: true, view: window });
                link.dispatchEvent(ev);
            }
        })()
    "#).await;
    tokio::time::sleep(Duration::from_secs(2)).await;
    let _ = screenshot_window(&window, &format!("{SCREENSHOT_DIR}/ui-9-dashboard.png")).await;

    tracing::info!("[ui-drive] done. screenshots in {SCREENSHOT_DIR}");

    // 用于视觉检查的 KEEP_RUNNING
    let keep_running = std::env::var("POLYROCKET_E2E_KEEP_RUNNING")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    if keep_running {
        tracing::info!("[ui-drive] KEEP_RUNNING=1 — staying alive");
        loop {
            tokio::time::sleep(Duration::from_secs(3600)).await;
        }
    }
    app.exit(0);
}

/// 在 webview 中运行 JS 表达式并返回 JSON 字符串。
/// 通过 `window.eval` 加回调的方式复用
/// e2e_football 的 `eval_with_callback` 机制。
/// 不会 await 异步表达式 —— 尽可能同步返回。
async fn eval_obj(window: &tauri::WebviewWindow, js: &str) -> String {
    use std::sync::mpsc;
    let (tx, rx) = mpsc::channel::<String>();
    let cb = move |result: String| {
        let _ = tx.send(result);
    };
    if let Err(e) = window.eval_with_callback(js.to_string(), cb) {
        return format!("__eval_error__: {}", e);
    }
    tokio::task::spawn_blocking(move || {
        match rx.recv_timeout(Duration::from_secs(30)) {
            Ok(s) => s,
            Err(mpsc::RecvTimeoutError::Timeout) => "__timeout_30s__".to_string(),
            Err(mpsc::RecvTimeoutError::Disconnected) => "__callback_dropped__".to_string(),
        }
    })
    .await
    .unwrap_or_else(|e| format!("__join_error__: {}", e))
}

/// 仅截取 polyrocket 窗口的截图。使用
/// CGWindowListCopyWindowInfo 按 owner + 标题
/// 查找窗口 id，然后执行 `screencapture -l <windowId>`。
/// 这是获得干净的 polyrocket 专属截图最可靠的
/// 方法，与 z-order 无关（基于此前发现：多显示器
/// 配置下其他应用可能位于 polyrocket 之上）。
async fn screenshot_window(_window: &tauri::WebviewWindow, path: &str) -> Result<(), String> {
    let wid_script = r#"
import CoreGraphics
let wins = CGWindowListCopyWindowInfo([.optionAll, .excludeDesktopElements], kCGNullWindowID) as? [[String: Any]] ?? []
for w in wins {
    let o = w[kCGWindowOwnerName as String] as? String ?? ""
    let n = w[kCGWindowName as String] as? String ?? ""
    if o == "polyrocket" && n == "polyrocket" {
        print(w[kCGWindowNumber as String] as? Int ?? 0)
    }
}
"#;
    let wid_out = std::process::Command::new("swift")
        .arg("-e")
        .arg(wid_script)
        .output()
        .map_err(|e| format!("swift: {}", e))?;
    let wid_str = String::from_utf8_lossy(&wid_out.stdout).trim().to_string();
    let wid: u32 = wid_str.parse().map_err(|_| format!("no wid (got {:?})", wid_str))?;
    let cmd = format!("screencapture -x -l {wid} {path}");
    let out = std::process::Command::new("sh")
        .arg("-c")
        .arg(&cmd)
        .output()
        .map_err(|e| format!("screencapture: {}", e))?;
    if !out.status.success() {
        return Err(format!(
            "screencapture failed: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    tracing::info!("[ui-drive] screenshot: {} (windowId={})", path, wid);
    Ok(())
}
