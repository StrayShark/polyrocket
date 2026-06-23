//! `e2e_football` — full end-to-end football market analysis test.
//!
//! Pipeline (uses the same code path the Tauri IPC uses):
//!   1. Init in-memory SQLite with full schema (26 tables via migrations.rs)
//!   2. Seed 1 real football market from SeedBundle.demo()
//!   3. Insert 1 enabled LLM provider (Doubao — proven to work)
//!   4. Build MarketContext + FootballMatchContext
//!   5. Build football.v1.0 prompt (Dixon-Coles + Elo + xG + CLV)
//!   6. Call the LLM via CustomClient::call() (same as dispatch internals)
//!   7. Parse response with parse_football_recommendation()
//!   8. Print the full match analysis to stdout
//!
//! Run:
//!   cd src-tauri && cargo run --bin e2e_football
//!
//! Requirements:
//!   - DOUBAO_API_KEY in env (or keyring entry llm/doubao/prod-1)

use polyrocket_lib::domain::llm::prompts::{
    build_football_match_request, parse_football_recommendation, FootballMatchContext,
    MarketContext, PROMPT_VERSION_FOOTBALL_MATCH,
};
use polyrocket_lib::domain::llm::{CustomClient, LlmClient};
use polyrocket_lib::domain::seed::SeedBundle;
use polyrocket_lib::infra::db::migrations::ensure_primary_tables;

use std::time::Instant;

#[tokio::main]
async fn main() {
    println!("=== e2e_football: end-to-end football market analysis ===\n");
    let started = Instant::now();

    // -- 1. in-memory SQLite + full schema (26 tables)
    let pool = match sqlx::SqlitePool::connect("sqlite::memory:").await {
        Ok(p) => p,
        Err(e) => {
            eprintln!("[e2e] open memory db: {e}");
            std::process::exit(1);
        }
    };
    if let Err(e) = ensure_primary_tables(&pool).await {
        eprintln!("[e2e] ensure_primary_tables: {e}");
        std::process::exit(1);
    }
    println!("[e2e] ✓ schema initialised (26 tables via migrations.rs)");

    // -- 2. seed a real football market via SeedBundle.demo()
    let bundle = SeedBundle::demo();
    let football_markets: Vec<_> = bundle.markets.iter().filter(|m| m.category == "football").collect();
    println!("[e2e] ✓ found {} football markets in seed bundle", football_markets.len());
    if football_markets.is_empty() {
        eprintln!("[e2e] no football markets in seed bundle");
        std::process::exit(1);
    }
    let target = football_markets[0];
    println!("[e2e] target market:");
    println!("         id       : {}", target.id);
    println!("         question : {}", target.question);
    println!("         slug     : {}", target.slug);
    println!("         volume24h: {}", target.volume_24h.unwrap_or(0.0));

    // Insert the market into the DB (idempotent via INSERT OR REPLACE)
    sqlx::query(
        "INSERT OR REPLACE INTO markets
            (id, slug, question, description, category, tags, end_date, active, resolved, outcome,
             liquidity, volume_24h, user_interested, brief_dismissed_at, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0, NULL, ?, ?)",
    )
    .bind(&target.id)
    .bind(&target.slug)
    .bind(&target.question)
    .bind(&target.description)
    .bind(&target.category)
    .bind(&target.tags)
    .bind(target.end_date)
    .bind(if target.active { 1_i64 } else { 0 })
    .bind(if target.resolved { 1_i64 } else { 0 })
    .bind(&target.outcome)
    .bind(&target.liquidity)
    .bind(&target.volume_24h)
    .bind(target.created_at)
    .bind(target.updated_at)
    .execute(&pool)
    .await
    .expect("insert market");
    println!("[e2e] ✓ market inserted into DB");

    // -- 3. set up provider list with fallback (Doubao first; fall back if rate-limited)
    #[derive(Clone)]
    struct Provider {
        id: &'static str,
        display: &'static str,
        api_base: &'static str,
        model: &'static str,
        cost_in: f64,
        cost_out: f64,
    }
    let providers = vec![
        Provider {
            id: "MiniMax", display: "MiniMax (M2.7)",
            api_base: "https://api.minimax.chat/v1",
            model: "MiniMax-M2.7",
            cost_in: 0.4, cost_out: 1.2,
        },
        Provider {
            id: "doubao", display: "Doubao (火山方舟)",
            api_base: "https://ark.cn-beijing.volces.com/api/coding/v3",
            model: "doubao-seed-2-0-pro-260215",
            cost_in: 0.08, cost_out: 0.08,
        },
        Provider {
            id: "qwen", display: "Qwen (DashScope, 通义千问)",
            api_base: "https://dashscope.aliyuncs.com/compatible-mode/v1",
            model: "qwen-plus",
            cost_in: 0.4, cost_out: 1.2,
        },
        Provider {
            id: "moonshot", display: "Moonshot (Kimi, 月之暗面)",
            api_base: "https://api.moonshot.cn/v1",
            model: "kimi-k2-0711-preview",
            cost_in: 0.6, cost_out: 0.6,
        },
        Provider {
            id: "zhipu", display: "Zhipu (智谱 GLM)",
            api_base: "https://open.bigmodel.cn/api/paas/v4",
            model: "glm-4-plus",
            cost_in: 5.0, cost_out: 5.0,
        },
    ];

    // Insert all 4 into DB as enabled (so any is selectable)
    for p in &providers {
        sqlx::query(
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
        .bind(p.id).bind(p.display).bind(p.api_base).bind(p.model)
        .bind(p.cost_in).bind(p.cost_out)
        .execute(&pool).await.expect("insert provider");
    }
    println!("[e2e] ✓ {} providers inserted (doubao, qwen, moonshot, zhipu)", providers.len());

    // -- 5. build MarketContext from the seeded market
    let yes_price: u32 = 64; // from Polymarket: Real Madrid around 64¢ YES
    let no_price: u32 = 36;
    let vol = target.volume_24h.unwrap_or(0.0);
    let liq = target.liquidity.unwrap_or_default().parse::<f64>().unwrap_or(0.0);
    let market_ctx = MarketContext {
        market_id: target.id.clone(),
        question: target.question.clone(),
        category: target.category.clone(),
        yes_price_cents: yes_price,
        no_price_cents: no_price,
        volume_24h_usdc: vol,
        liquidity_usdc: liq,
        closes_at_unix_ms: target.end_date,
        resolution_source: "Polymarket (UMA)".into(),
        recent_signals: vec![],
        orderbook_top: None,
    };
    println!("[e2e] ✓ MarketContext built (yes={yes_price}¢ no={no_price}¢ vol=${vol:.0})");

    // -- 6. route to football.v1.0 (auto-derives home/away + market_type)
    let football_ctx = FootballMatchContext::from_market_context(market_ctx.clone());
    println!("[e2e] ✓ FootballMatchContext:");
    println!("         market_type = {:?}", football_ctx.market_type);
    println!("         home_team   = {:?}", football_ctx.home_team);
    println!("         away_team   = {:?}", football_ctx.away_team);
    println!("         handicap    = {:?}", football_ctx.handicap_line);
    println!("         o/u line    = {:?}", football_ctx.over_under_line);
    println!("         prompt_ver  = {PROMPT_VERSION_FOOTBALL_MATCH}");

let req = build_football_match_request(providers[0].model, &football_ctx);
    // Get user content (last message in the messages vec)
    let user_content = req.messages.iter()
        .rev()
        .find(|m| m.role == "user")
        .map(|m| m.content.as_str())
        .unwrap_or("(no user msg)");
    println!("[e2e] ✓ football.v1.0 prompt built ({} bytes user content)", user_content.len());
    println!();
    println!("=== Prompt preview (first 500 chars) ===");
    let preview: String = user_content.chars().take(500).collect();
    println!("{}", preview);
    println!("...\n");

    // -- 7. try each provider in order; use first that returns 2xx + parseable JSON
    let http = polyrocket_lib::domain::llm::new_http_client();
    let analysis_id_base = format!("e2e-{}", std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());

    let mut selected: Option<(Provider, polyrocket_lib::domain::llm::CallOutcome)> = None;
    for p in &providers {
        // Build per-provider req (model name differs)
        let req_p = build_football_match_request(p.model, &football_ctx);
        let env_var = format!("{}_API_KEY", p.id.to_uppercase());
        let keyring_alias = format!("llm/{}/prod-1", p.id);
        let api_key = std::env::var(&env_var).ok()
            .filter(|v| !v.is_empty())
            .or_else(|| {
                keyring::Entry::new("com.polyrocket.wallet", &keyring_alias)
                    .and_then(|e| e.get_password())
                    .ok()
            })
            .unwrap_or_default();
        if api_key.is_empty() {
            println!("[e2e] ⊘ skip {}: no {} env var / keyring empty", p.id, env_var);
            continue;
        }
        let client = CustomClient::new_openai_compat(p.api_base, p.model);
        let cost = polyrocket_lib::domain::llm::CostRate {
            per_1k_in_cents: p.cost_in,
            per_1k_out_cents: p.cost_out,
        };
        println!("[e2e] ▶ trying {} (model={}, analysis_id={}-{})", p.id, p.model, analysis_id_base, p.id);
        let t = Instant::now();
        let r = client.call(&http, &api_key, &req_p, cost).await;
        let elapsed = t.elapsed();
        match r {
            Ok(out) => {
                println!("[e2e]   ✓ {} responded in {}ms — http={} tokens={}in/{}out",
                    p.id, elapsed.as_millis(), out.http_status, out.tokens_in, out.tokens_out);
                selected = Some((p.clone(), out));
                break;
            }
            Err(e) => {
                println!("[e2e]   ✗ {} failed ({}ms): code={} http={:?}",
                    p.id, elapsed.as_millis(), e.code, e.http_status);
                // If auth / quota / not retryable, try next
                if matches!(e.code, "auth" | "quota" | "model_not_found") || e.http_status == Some(401) || e.http_status == Some(403) || e.http_status == Some(404) {
                    continue;
                }
                // Transient errors: also continue but the message above shows
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

    // -- 8. print the analysis result
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
    println!("{}", raw_preview);
    println!("...\n");

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
                println!("  │ {}", line);
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
                let f = |v: &Option<f64>| v.map(|x| format!("{:.3}", x)).unwrap_or_else(|| "n/a".into());
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
                if let Some(ah) = &fb.asian_handicap_recommendation {
                    println!("  asian_handicap_rec    : {ah}");
                }
            } else {
                println!();
                println!("(no framework_breakdown — model didn't return one)");
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
    println!("=== e2e_football completed in {}ms ===", total.as_millis());
}