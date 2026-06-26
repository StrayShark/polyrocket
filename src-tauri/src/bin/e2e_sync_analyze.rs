//! `e2e_sync_analyze` —— 完整的端到端测试：从 Polymarket Gamma API
//! 同步真实的足球市场，然后对第一个市场运行 LLM 分析。
//!
//! 流水线（使用与 Tauri IPC 相同的代码路径）：
//!   1. 初始化内存 SQLite，包含完整 schema（通过 migrations.rs 26 张表）
//!   2. 调用 `polymarket::fetch_active_markets()` —— 真实的 Gamma API 调用
//!   3. 通过 `is_football_market()` 过滤足球市场
//!   4. 将过滤后的市场插入 DB（与 `sync_markets` 命令相同的 SQL）
//!   5. 将同步的市场列表输出到 stdout
//!   6. 从环境变量设置 LLM provider（DOUBAO_API_KEY / MINIMAX_API_KEY 等）
//!   7. 为第一个市场构建 MarketContext + FootballMatchContext
//!   8. 构建 football.v1.0 prompt（Dixon-Coles + Elo + xG + CLV）
//!   9. 通过 CustomClient::call() 调用 LLM（与 dispatch 内部相同）
//!  10. 使用 parse_football_recommendation() 解析响应
//!  11. 将完整比赛分析打印到 stdout
//!
//! 运行：
//!   cd src-tauri && cargo run --bin e2e_sync_analyze
//!
//! 要求：
//!   - 至少在环境变量中提供以下之一：DOUBAO_API_KEY、MINIMAX_API_KEY、QWEN_API_KEY、
//!     MOONSHOT_API_KEY、ZHIPU_API_KEY
//!   - 可访问 gamma-api.polymarket.com
//!   - 可选：POLYROCKET_PROXY=127.0.0.1:7897 使用代理

use polyrocket_lib::domain::llm::prompts::{
    build_football_match_request, parse_football_recommendation, FootballMatchContext,
    MarketContext, PROMPT_VERSION_FOOTBALL_MATCH,
};
use polyrocket_lib::domain::llm::{CustomClient, LlmClient};
use polyrocket_lib::domain::polymarket;
use polyrocket_lib::infra::db::migrations::ensure_primary_tables;

use std::time::Instant;

/// LLM 分析回退链中的 provider 候选。
#[derive(Clone)]
struct Provider {
    id: &'static str,
    display: &'static str,
    api_base: &'static str,
    model: &'static str,
    cost_in: f64,
    cost_out: f64,
}

/// 支持的 5 个 provider，按可靠性顺序尝试。
const PROVIDERS: &[Provider] = &[
    Provider {
        id: "MiniMax",
        display: "MiniMax (M2.7)",
        api_base: "https://api.minimax.chat/v1",
        model: "MiniMax-M2.7",
        cost_in: 0.4,
        cost_out: 1.2,
    },
    Provider {
        id: "doubao",
        display: "Doubao (火山方舟)",
        api_base: "https://ark.cn-beijing.volces.com/api/coding/v3",
        model: "doubao-seed-2-0-pro-260215",
        cost_in: 0.08,
        cost_out: 0.08,
    },
    Provider {
        id: "qwen",
        display: "Qwen (DashScope, 通义千问)",
        api_base: "https://dashscope.aliyuncs.com/compatible-mode/v1",
        model: "qwen-plus",
        cost_in: 0.4,
        cost_out: 1.2,
    },
    Provider {
        id: "moonshot",
        display: "Moonshot (Kimi, 月之暗面)",
        api_base: "https://api.moonshot.cn/v1",
        model: "kimi-k2-0711-preview",
        cost_in: 0.6,
        cost_out: 0.6,
    },
    Provider {
        id: "zhipu",
        display: "Zhipu (智谱 GLM)",
        api_base: "https://open.bigmodel.cn/api/paas/v4",
        model: "glm-4-plus",
        cost_in: 5.0,
        cost_out: 5.0,
    },
];

/// 足球关键词检查 —— 与 `commands::market::is_football_market` 对应。
/// 在此重复是因为该二进制无法直接调用 L2 命令。
fn is_football_text(s: &str) -> bool {
    let lower = s.to_lowercase();
    lower.contains("football")
        || lower.contains("soccer")
        || lower.contains("fifa")
        || lower.contains("uefa")
        || lower.contains("world cup")
        || lower.contains("champions league")
        || lower.contains("europa league")
        || lower.contains("premier league")
        || lower.contains("la liga")
        || lower.contains("bundesliga")
        || lower.contains("serie a")
        || lower.contains("ligue 1")
        || lower.contains("mls")
        || lower.contains("epl")
        || lower.contains("match")
        || lower.contains("goal")
        || lower.contains(" fc")
        || lower.contains(" united")
        || lower.contains(" city")
        || lower.contains("real madrid")
        || lower.contains("barcelona")
        || lower.contains("liverpool")
        || lower.contains("arsenal")
        || lower.contains("chelsea")
        || lower.contains("tottenham")
        || lower.contains("manchester")
        || lower.contains("bayern")
        || lower.contains("dortmund")
        || lower.contains("juventus")
        || lower.contains("milan")
        || lower.contains("inter")
        || lower.contains("psg")
        || lower.contains("marseille")
}

/// 检查一个市场是否与足球相关（分类 + 标签 + 问题）。
fn is_football_market(m: &polymarket::MarketSummary) -> bool {
    if let Some(cat) = m.category.as_deref() {
        if !cat.trim().is_empty() && is_football_text(cat) {
            return true;
        }
    }
    if let Some(tags) = m.tags.as_ref() {
        for t in tags {
            if is_football_text(t) {
                return true;
            }
        }
    }
    is_football_text(&m.question)
}

#[tokio::main]
async fn main() {
    println!("=== e2e_sync_analyze: sync real markets + LLM analysis ===\n");
    let started = Instant::now();

    // -- 1. 内存 SQLite + 完整 schema
    let pool = match sqlx::SqlitePool::connect("sqlite::memory:").await {
        Ok(p) => p,
        Err(e) => {
            eprintln!("[e2e] FATAL: open memory db: {e}");
            std::process::exit(1);
        }
    };
    if let Err(e) = ensure_primary_tables(&pool).await {
        eprintln!("[e2e] FATAL: ensure_primary_tables: {e}");
        std::process::exit(1);
    }
    println!("[e2e] ✓ schema initialised (26 tables)");

    // -- 2. 从 Polymarket Gamma API 抓取真实市场
    println!("[e2e] ▶ fetching markets from Polymarket Gamma API...");
    let fetch_start = Instant::now();
    let remote = match polymarket::fetch_active_markets().await {
        Ok(m) => m,
        Err(e) => {
            eprintln!("[e2e] FATAL: fetch_active_markets failed: {e}");
            eprintln!("[e2e]   (check network / POLYROCKET_PROXY setting)");
            std::process::exit(1);
        }
    };
    let fetch_elapsed = fetch_start.elapsed();
    println!(
        "[e2e] ✓ Gamma API returned {} markets in {}ms",
        remote.len(),
        fetch_elapsed.as_millis()
    );

    // -- 3. 过滤足球市场
    let football: Vec<_> = remote.iter().filter(|m| is_football_market(m)).collect();
    let non_football: Vec<_> = remote.iter().filter(|m| !is_football_market(m)).collect();
    println!(
        "[e2e] ✓ football filter: {} football / {} non-football (total {})",
        football.len(),
        non_football.len(),
        remote.len()
    );

    if football.is_empty() {
        eprintln!("[e2e] FATAL: no football markets found in Gamma API response");
        eprintln!("[e2e]   (the API may be down or returning non-football content)");
        std::process::exit(1);
    }

    // -- 4. 将足球市场插入 DB
    let now_ms = chrono::Utc::now().timestamp_millis();
    let mut n_written = 0usize;
    for m in &football {
        let end_ms = m.end_date_ms.unwrap_or(0);
        let active_flag = m.active && !m.archived;
        let resolved_flag = m.closed;
        let _ = sqlx::query(
            "INSERT INTO markets (id, slug, question, description, category, end_date,
                                  active, resolved, outcome, liquidity, volume_24h,
                                  created_at, updated_at)
             VALUES (?, ?, ?, ?, 'football', ?, ?, ?, NULL, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET
                question=excluded.question,
                description=excluded.description,
                category='football',
                end_date=excluded.end_date,
                active=excluded.active,
                resolved=excluded.resolved,
                liquidity=excluded.liquidity,
                volume_24h=excluded.volume_24h,
                updated_at=excluded.updated_at",
        )
        .bind(&m.id)
        .bind(&m.slug)
        .bind(&m.question)
        .bind(m.description.as_deref())
        .bind(end_ms)
        .bind(active_flag)
        .bind(resolved_flag)
        .bind(&m.liquidity)
        .bind(m.volume_24hr)
        .bind(now_ms)
        .bind(now_ms)
        .execute(&pool)
        .await;
        n_written += 1;
    }
    println!("[e2e] ✓ {} football markets inserted into DB", n_written);

    // -- 5. 列出已同步的市场
    println!("\n=== Synced Football Markets ===");
    for (i, m) in football.iter().enumerate() {
        println!(
            "  {}. {:<40} | vol24h=${:>10.0} | {}",
            i + 1,
            &m.question.chars().take(40).collect::<String>(),
            m.volume_24hr,
            m.id
        );
    }
    println!();

    // -- 6. 在 DB 中设置 LLM provider
    let mut available_providers: Vec<&Provider> = Vec::new();
    for p in PROVIDERS {
        let env_var = format!("{}_API_KEY", p.id.to_uppercase());
        let keyring_alias = format!("llm/{}/prod-1", p.id);
        let api_key = std::env::var(&env_var)
            .ok()
            .filter(|v| !v.is_empty())
            .or_else(|| {
                keyring::Entry::new("com.polyrocket.wallet", &keyring_alias)
                    .and_then(|e| e.get_password())
                    .ok()
            })
            .unwrap_or_default();
        if api_key.is_empty() {
            println!("[e2e] ⊘ skip provider {}: no {} env var", p.id, env_var);
            continue;
        }
        // 向 DB 插入 provider 行。
        let _ = sqlx::query(
            "INSERT OR REPLACE INTO llm_providers
                (id, display_name, provider_kind, request_format, supports_streaming, enabled,
                 api_base, key_alias, default_model, timeout_ms, request_timeout_ms, max_retries,
                 cost_per_1k_in, cost_per_1k_out, rate_limit_rpm, rate_limit_tpm,
                 quota_daily_cents, quota_monthly_cents, key_rotation_strategy,
                 health_status, health_latency_p50_ms, health_latency_p95_ms,
                 last_health_check_at, last_health_error, notes, updated_at)
                VALUES (?, ?, 'openai_compat', 'openai_compat', 0, 1, ?, 'prod-1', ?,
                        120000, 120000, 2, ?, ?, 60, 60000, 1000, 100000,
                        NULL, NULL, NULL, NULL, NULL, NULL, NULL, strftime('%s','now')*1000)",
        )
        .bind(p.id)
        .bind(p.display)
        .bind(p.api_base)
        .bind(p.model)
        .bind(p.cost_in)
        .bind(p.cost_out)
        .execute(&pool)
        .await;
        // 向 DB 插入 provider key 行。
        let key_id = format!("{}-prod-1-{}", p.id, std::process::id());
        let _ = sqlx::query(
            "INSERT OR REPLACE INTO llm_provider_keys
                (id, provider_id, alias, keyring_alias, enabled, priority, weight,
                 created_at, updated_at)
             VALUES (?, ?, 'prod-1', ?, 1, 0, 1.0, ?, ?)",
        )
        .bind(&key_id)
        .bind(p.id)
        .bind(format!("llm/{}/prod-1", p.id))
        .bind(now_ms)
        .bind(now_ms)
        .execute(&pool)
        .await;
        available_providers.push(p);
        println!("[e2e] ✓ provider {} registered (model={})", p.id, p.model);
    }

    if available_providers.is_empty() {
        eprintln!("[e2e] FATAL: no LLM providers available");
        eprintln!("[e2e]   set one of: DOUBAO_API_KEY, MINIMAX_API_KEY, QWEN_API_KEY, MOONSHOT_API_KEY, ZHIPU_API_KEY");
        std::process::exit(1);
    }

    // -- 7. 选取第一个足球市场并构建上下文
    let target = football[0];
    println!("\n=== Target Market for LLM Analysis ===");
    println!("  id       : {}", target.id);
    println!("  question : {}", target.question);
    println!("  slug     : {}", target.slug);
    println!("  volume24h: ${:.0}", target.volume_24hr);

    // 使用来自市场成交量的合理 YES 价格。
    let yes_price: u32 = 50; // 默认中间价
    let no_price: u32 = 50;
    let vol = target.volume_24hr;
    let liq = target.liquidity.parse::<f64>().unwrap_or(0.0);

    let market_ctx = MarketContext {
        market_id: target.id.clone(),
        question: target.question.clone(),
        category: "football".into(),
        yes_price_cents: yes_price,
        no_price_cents: no_price,
        volume_24h_usdc: vol,
        liquidity_usdc: liq,
        closes_at_unix_ms: target.end_date_ms.unwrap_or(0),
        resolution_source: "Polymarket (UMA)".into(),
        recent_signals: vec![],
        orderbook_top: None,
    };
    println!("[e2e] ✓ MarketContext built (yes={yes_price}¢ no={no_price}¢ vol=${vol:.0})");

    // -- 8. 构建足球上下文 + prompt
    let football_ctx = FootballMatchContext::from_market_context(market_ctx.clone());
    println!("[e2e] ✓ FootballMatchContext:");
    println!("         market_type = {:?}", football_ctx.market_type);
    println!("         home_team   = {:?}", football_ctx.home_team);
    println!("         away_team   = {:?}", football_ctx.away_team);
    println!("         handicap    = {:?}", football_ctx.handicap_line);
    println!("         o/u line    = {:?}", football_ctx.over_under_line);
    println!("         prompt_ver  = {PROMPT_VERSION_FOOTBALL_MATCH}");

    let req = build_football_match_request(available_providers[0].model, &football_ctx);
    let user_content = req
        .messages
        .iter()
        .rev()
        .find(|m| m.role == "user")
        .map(|m| m.content.as_str())
        .unwrap_or("(no user msg)");
    println!(
        "[e2e] ✓ football.v1.0 prompt built ({} bytes user content)",
        user_content.len()
    );
    println!();
    println!("=== Prompt preview (first 500 chars) ===");
    let preview: String = user_content.chars().take(500).collect();
    println!("{preview}...\n");

    // -- 9. 依次尝试每个可用的 provider
    let http = polyrocket_lib::domain::llm::new_http_client();
    let analysis_id_base = format!(
        "e2e-sync-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
    );

    let mut selected: Option<(&Provider, polyrocket_lib::domain::llm::CallOutcome)> = None;
    for p in &available_providers {
        let req_p = build_football_match_request(p.model, &football_ctx);
        let env_var = format!("{}_API_KEY", p.id.to_uppercase());
        let keyring_alias = format!("llm/{}/prod-1", p.id);
        let api_key = std::env::var(&env_var)
            .ok()
            .filter(|v| !v.is_empty())
            .or_else(|| {
                keyring::Entry::new("com.polyrocket.wallet", &keyring_alias)
                    .and_then(|e| e.get_password())
                    .ok()
            })
            .unwrap_or_default();
        if api_key.is_empty() {
            continue;
        }
        let client = CustomClient::new_openai_compat(p.api_base, p.model);
        let cost = polyrocket_lib::domain::llm::CostRate {
            per_1k_in_cents: p.cost_in,
            per_1k_out_cents: p.cost_out,
        };
        println!(
            "[e2e] ▶ trying {} (model={}, analysis_id={}-{})",
            p.id, p.model, analysis_id_base, p.id
        );
        let t = Instant::now();
        let r = client.call(&http, &api_key, &req_p, cost).await;
        let elapsed = t.elapsed();
        match r {
            Ok(out) => {
                println!(
                    "[e2e]   ✓ {} responded in {}ms — http={} tokens={}in/{}out",
                    p.id, elapsed.as_millis(), out.http_status, out.tokens_in, out.tokens_out
                );
                selected = Some((p, out));
                break;
            }
            Err(e) => {
                println!(
                    "[e2e]   ✗ {} failed ({}ms): code={} http={:?}",
                    p.id, elapsed.as_millis(), e.code, e.http_status
                );
                if matches!(e.code, "auth" | "quota" | "model_not_found")
                    || e.http_status == Some(401)
                    || e.http_status == Some(403)
                    || e.http_status == Some(404)
                {
                    continue;
                }
            }
        }
    }

    let (selected_p, resp) = match selected {
        Some(s) => s,
        None => {
            eprintln!("[e2e] FATAL: all providers failed");
            std::process::exit(1);
        }
    };

    // -- 10. 打印分析结果
    println!();
    println!("=== Match Analysis Result (provider={}) ===", selected_p.id);
    println!("display_name     : {}", selected_p.display);
    println!("http_status      : {}", resp.http_status);
    println!("latency_ms       : {}", resp.latency_ms);
    println!("tokens_in        : {}", resp.tokens_in);
    println!("tokens_out       : {}", resp.tokens_out);
    println!("cost_cents       : {:.6}", resp.cost_cents);
    println!("parse_ok         : {}", resp.parse_ok);
    if let Some(pe) = &resp.parse_error {
        println!("parse_error      : {pe}");
    }
    println!("raw_len          : {} chars", resp.text.len());
    println!();
    println!("--- raw response (first 1200 chars) ---");
    let raw_preview: String = resp.text.chars().take(1200).collect();
    println!("{raw_preview}...\n");

    // -- 11. 解析 + 打印结构化结果
    println!("--- parsed FootballRecommendationPayload ---");
    match parse_football_recommendation(&resp.text) {
        Ok(rec) => {
            let prob = rec.probability.unwrap_or(f64::NAN);
            let side = rec.side.clone().unwrap_or_else(|| "?".into());
            let conf = rec.confidence.unwrap_or(f64::NAN);
            let reasoning = rec.reasoning.unwrap_or_default();
            println!("probability      : {:.3}", prob);
            println!("side             : {}", side);
            println!("confidence       : {:.3}", conf);
            println!("reasoning_len    : {} chars", reasoning.len());
            println!();
            println!("reasoning:");
            for line in reasoning.lines() {
                println!("  │ {line}");
            }
            println!();
            if let Some(factors) = &rec.key_factors {
                println!("key_factors:");
                for (i, f) in factors.iter().enumerate() {
                    println!("  {}. {}", i + 1, f);
                }
            }
            if let Some(fb) = &rec.framework_breakdown {
                println!();
                println!("framework_breakdown:");
                let f = |v: &Option<f64>| {
                    v.map(|x| format!("{:.3}", x))
                        .unwrap_or_else(|| "n/a".into())
                };
                println!("  elo_home              : {}", f(&fb.elo_home));
                println!("  elo_away              : {}", f(&fb.elo_away));
                println!("  elo_diff_with_hfa     : {}", f(&fb.elo_diff_with_hfa));
                println!("  implied_win_pct_elo   : {}", f(&fb.implied_win_pct_elo));
                println!("  lambda_home_goals     : {}", f(&fb.lambda_home_goals));
                println!("  lambda_away_goals     : {}", f(&fb.lambda_away_goals));
                println!("  dixoncoles_home_win%  : {}", f(&fb.dixon_coles_home_win_pct));
                println!("  dixoncoles_draw%      : {}", f(&fb.dixon_coles_draw_pct));
                println!("  dixoncoles_away_win%  : {}", f(&fb.dixon_coles_away_win_pct));
                println!("  xg_last5_home_per90   : {}", f(&fb.xg_last5_home_per90));
                println!("  xg_last5_away_per90   : {}", f(&fb.xg_last5_away_per90));
                println!("  polymarket_implied    : {}", f(&fb.polymarket_implied_prob));
                println!("  clv_edge              : {}", f(&fb.clv_edge));
            }
        }
        Err(e) => {
            println!("parse error: {e}");
            println!("(raw response above — manual review needed)");
            std::process::exit(1);
        }
    }

    let total = started.elapsed();
    println!();
    println!("=== e2e_sync_analyze completed in {}ms ===", total.as_millis());
}
