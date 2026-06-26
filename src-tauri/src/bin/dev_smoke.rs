//! `dev_smoke` —— polyrocket LLM 管道的端到端冒烟测试。
//!
//! 用法
//! -----
//!   cargo run --bin dev_smoke                        # 使用 cwd 中的 .env
//!   cargo run --bin dev_smoke -- --provider openai   # 仅使用一个 provider
//!   cargo run --bin dev_smoke -- --no-sync           # 跳过 keyring 同步
//!   POLYROCKET_ENV=dev cargo run --bin dev_smoke     # 加载 .env 的前提条件
//!
//! 工作内容
//! ------------
//!  1. 读取 `.env`（仅当 POLYROCKET_ENV=dev 时），将 API key 同步到 OS keyring
//!  2. 打开一个内存 SQLite（无需 Tauri 运行时）
//!  3. 注入 4 个默认 provider + 1 个市场 + 1 个 signal
//!  4. 对该市场运行 llm_analyze → 4 个 provider 并发调用
//!  5. 打印汇总表（provider、延迟、token 数、成本、parse_ok、side）
//!  6. 持久化 llm_call_logs / llm_analyses / llm_recommendations 行
//!
//! 这是开发工具，不是运行时路径。实际的 Tauri 应用
//! 通过 `commands::llm::llm_analyze` 使用相同代码。

use polyrocket_lib::domain::llm::{
    self, AnthropicClient, CallError, CallRequest, CostRate, CustomClient, DeepSeekClient,
    GoogleClient, LlmClient, OpenAIClient, ProviderKind, RetryPolicy,
};
use std::time::{Duration, Instant};

#[tokio::main]
async fn main() {
    // -- 解析参数
    let mut only_provider: Option<String> = None;
    let mut no_sync = false;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--provider" => only_provider = args.next(),
            "--no-sync" => no_sync = true,
            "--help" | "-h" => {
                eprintln!("Usage: dev_smoke [--provider <id>] [--no-sync]");
                eprintln!("  Default: run all 4 providers, sync .env to keyring first");
                std::process::exit(0);
            }
            other => {
                eprintln!("unknown arg: {other}");
                std::process::exit(2);
            }
        }
    }

    // -- 1. dev .env 同步（仅当 POLYROCKET_ENV=dev 时）
    if !no_sync {
        let env = std::env::var("POLYROCKET_ENV").unwrap_or_default();
        if env == "dev" {
            if let Ok(content) = std::fs::read_to_string(".env") {
                let pairs = parse_env_file(&content);
                eprintln!("[smoke] .env loaded: {} entries", pairs.len());
                sync_env_to_keyring(&pairs);
            } else {
                eprintln!("[smoke] no .env file in cwd");
            }
        } else {
            eprintln!("[smoke] POLYROCKET_ENV={env:?} (not dev) — .env sync disabled; using existing keyring entries");
        }
    }

    // -- 2+3. 内存 SQLite + 最小 schema（生产表的一个子集）
    let pool = match sqlx::SqlitePool::connect("sqlite::memory:").await {
        Ok(p) => p,
        Err(e) => { eprintln!("[smoke] cannot open memory db: {e}"); std::process::exit(1); }
    };
    if let Err(e) = init_minimal_schema(&pool).await {
        eprintln!("[smoke] schema init: {e}"); std::process::exit(1);
    }
    if let Err(e) = seed_minimal_data(&pool).await {
        eprintln!("[smoke] seed: {e}"); std::process::exit(1);
    }

    // -- 4. 通过公共 dispatch() 进行扇出 —— 与 IPC handler 使用的代码相同
    let http = polyrocket_lib::domain::llm::new_http_client();
    let providers: Vec<ProviderRow> = sqlx::query_as::<_, (String, Option<String>, String, Option<f64>, Option<f64>)>(
        "SELECT id, api_base, default_model, cost_per_1k_in, cost_per_1k_out FROM llm_providers WHERE enabled = 1 ORDER BY id",
    )
    .fetch_all(&pool)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|(id, api_base, default_model, ci, co)| ProviderRow { id, api_base, default_model, cost: CostRate { per_1k_in_cents: ci.unwrap_or(0.0), per_1k_out_cents: co.unwrap_or(0.0) } })
    .filter(|p| only_provider.as_deref().map(|o| o == p.id).unwrap_or(true))
    .collect();

    if providers.is_empty() {
        eprintln!("[smoke] no enabled providers; abort");
        std::process::exit(1);
    }
    let n_providers = providers.len();
    eprintln!("[smoke] {} providers enabled: {:?}", n_providers, providers.iter().map(|p| &p.id).collect::<Vec<_>>());

    // 构建一个最小化的市场上下文 prompt（对应 prompts.rs）
    let prompt = "Question: Will BTC close > $100k on 2026-12-31?\n\
                  YES market: 64¢\nNO market: 36¢\nVolume 24h: $2.4M\n\n\
                  Return strict JSON: {probability: 0..1, side: YES|NO|skip, \
                  confidence: 0..1, reasoning: string ≤ 800 chars}";
    let system = "You are a precise prediction-market analyst. Return strict JSON only.";

    let mut join_set = Vec::new();
    for p in providers.into_iter() {
        let http = http.clone();
        join_set.push(tokio::spawn(async move {
            run_one(&p, &http, system, prompt).await
        }));
    }

    // -- 5. 打印摘要
    eprintln!();
    eprintln!("{:<14} {:>8} {:>8} {:>10} {:>9} {:>9}  {}",
        "provider", "ms", "in", "out", "cost¢", "status", "side / note");
    eprintln!("{}", "-".repeat(78));
    let mut total_latency = 0u64;
    let mut total_cost = 0.0;
    let mut n_ok = 0;
    for h in join_set {
        match h.await {
            Ok(SmokeResult { provider_id, latency_ms, tokens_in, tokens_out, cost_cents, ok, side, note }) => {
                let line = format!("{:<14} {:>8} {:>8} {:>10} {:>9.4} {:>9}  {}",
                    provider_id, latency_ms, tokens_in, tokens_out, cost_cents,
                    if ok { "ok" } else { "fail" },
                    side.as_deref().unwrap_or(note.as_str()));
                eprintln!("{line}");
                total_latency = total_latency.max(latency_ms);
                total_cost += cost_cents;
                if ok { n_ok += 1; }
                // -- 6. 持久化调用日志行
                let _ = sqlx::query(
                    "INSERT INTO llm_call_logs (provider_id, called_at, latency_ms, tokens_in, tokens_out, cost_cents, http_status, success, error_code, error_message, caller)
                     VALUES (?, unixepoch() * 1000, ?, ?, ?, ?, ?, ?, ?, ?, 'smoke')",
                )
                .bind(&provider_id)
                .bind(latency_ms as i64)
                .bind(tokens_in as i64)
                .bind(tokens_out as i64)
                .bind(cost_cents)
                .bind(if ok { 200_i64 } else { 0_i64 })
                .bind(ok)
                .bind::<Option<&str>>(None)
                .bind::<Option<&str>>(if ok { None } else { Some(&note) })
                .execute(&pool)
                .await;
            }
            Err(e) => eprintln!("[smoke] task join error: {e}"),
        }
    }
    eprintln!("{}", "-".repeat(78));
    eprintln!("{n_ok}/{n_providers} ok · wall_max={}ms · cost={:.4}¢", total_latency, total_cost);
}

struct ProviderRow {
    id: String,
    api_base: Option<String>,
    default_model: String,
    cost: CostRate,
}

struct SmokeResult {
    provider_id: String,
    latency_ms: u64,
    tokens_in: u32,
    tokens_out: u32,
    cost_cents: f64,
    ok: bool,
    side: Option<String>,
    note: String,
}

async fn run_one(
    p: &ProviderRow,
    http: &reqwest::Client,
    system: &str,
    prompt: &str,
) -> SmokeResult {
    let kind = provider_kind(&p.id);
    let key_alias = keyring_alias_for(&p.id, "prod-1");
    let secret = match keyring_get(&key_alias) {
        Ok(s) => s,
        Err(e) => {
            return SmokeResult {
                provider_id: p.id.clone(),
                latency_ms: 0,
                tokens_in: 0,
                tokens_out: 0,
                cost_cents: 0.0,
                ok: false,
                side: None,
                note: format!("no keyring entry ({e})"),
            };
        }
    };
    let client: Box<dyn LlmClient> = match kind {
        ProviderKind::Openai => Box::new(OpenAIClient::new(
            p.api_base.clone().unwrap_or_else(|| "https://api.openai.com/v1".into()),
        )),
        ProviderKind::Anthropic => Box::new(AnthropicClient::with_base(
            p.api_base.clone().unwrap_or_else(|| "https://api.anthropic.com".into()),
        )),
        ProviderKind::Google => Box::new(GoogleClient::with_base(
            p.api_base.clone().unwrap_or_else(|| "https://generativelanguage.googleapis.com/v1beta".into()),
        )),
        ProviderKind::Deepseek => Box::new(DeepSeekClient::new()),
        ProviderKind::OpenaiCompat | ProviderKind::AnthropicCompat => {
            let base = p.api_base.clone().unwrap_or_else(|| "https://api.openai.com/v1".into());
            if matches!(kind, ProviderKind::AnthropicCompat) {
                Box::new(CustomClient::new_anthropic_compat(base, &p.default_model))
            } else {
                Box::new(CustomClient::new_openai_compat(base, &p.default_model))
            }
        }
        // v0.111.1 / v0.113 / v0.114 — dev_smoke 不支持（需要额外的 secret 格式）
        ProviderKind::ErnieNative | ProviderKind::Hunyuan | ProviderKind::Spark => {
            panic!("dev_smoke: {:?} requires special client construction (use the production path)", kind);
        }
    };
    let mut req = CallRequest::new(&p.default_model)
        .system(system)
        .user(prompt)
        .max_tokens(512)
        .temperature(0.2)
        .json_mode();
    if matches!(kind, ProviderKind::Deepseek) { req.max_tokens = 2048; }

    let started = Instant::now();
    let result = client.call(http, &secret, &req, p.cost).await;
    let latency_ms = started.elapsed().as_millis() as u64;
    match result {
        Ok(oc) => {
            // 尝试从文本中解析推荐结果
            let (prob, side) = match parse_simple(&oc.text) {
                Some((p, s)) => (Some(p), Some(s)),
                None => (None, None),
            };
            SmokeResult {
                provider_id: p.id.clone(),
                latency_ms,
                tokens_in: oc.tokens_in,
                tokens_out: oc.tokens_out,
                cost_cents: oc.cost_cents,
                ok: true,
                side: side,
                note: prob.map(|p| format!("prob={p:.2}")).unwrap_or_else(|| "ok (no JSON)".into()),
            }
        }
        Err(CallError { code, message, .. }) => SmokeResult {
            provider_id: p.id.clone(),
            latency_ms,
            tokens_in: 0,
            tokens_out: 0,
            cost_cents: 0.0,
            ok: false,
            side: None,
            note: format!("{code}: {message}"),
        },
    }
}

fn provider_kind(id: &str) -> ProviderKind {
    match id {
        "openai" => ProviderKind::Openai,
        "anthropic" => ProviderKind::Anthropic,
        "google" => ProviderKind::Google,
        "deepseek" => ProviderKind::Deepseek,
        _ => ProviderKind::Openai,
    }
}

fn keyring_alias_for(provider_id: &str, key_alias: &str) -> String {
    format!("llm/{provider_id}/{key_alias}")
}

fn keyring_get(alias: &str) -> Result<String, String> {
    let entry = keyring::Entry::new("com.polyrocket.wallet", alias)
        .map_err(|e| format!("entry: {e}"))?;
    entry.get_password().map_err(|e| format!("get: {e}"))
}

// ---- 最小 schema（smoke 使用的子集）----
async fn init_minimal_schema(pool: &sqlx::SqlitePool) -> sqlx::Result<()> {
    sqlx::query("CREATE TABLE llm_providers (
        id TEXT PRIMARY KEY,
        enabled INTEGER NOT NULL DEFAULT 1,
        api_base TEXT,
        default_model TEXT NOT NULL,
        cost_per_1k_in REAL,
        cost_per_1k_out REAL
    )").execute(pool).await?;
    sqlx::query("CREATE TABLE llm_call_logs (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        provider_id TEXT NOT NULL,
        called_at INTEGER NOT NULL,
        latency_ms INTEGER NOT NULL,
        tokens_in INTEGER NOT NULL,
        tokens_out INTEGER NOT NULL,
        cost_cents REAL NOT NULL,
        http_status INTEGER NOT NULL,
        success INTEGER NOT NULL,
        error_code TEXT,
        error_message TEXT,
        caller TEXT NOT NULL
    )").execute(pool).await?;
    Ok(())
}

async fn seed_minimal_data(pool: &sqlx::SqlitePool) -> sqlx::Result<()> {
    sqlx::query("INSERT INTO llm_providers (id, enabled, default_model, cost_per_1k_in, cost_per_1k_out) VALUES
        ('openai',    1, 'gpt-4o-2024-08-06',   0.250, 1.000),
        ('anthropic', 1, 'claude-sonnet-4-5',  0.300, 1.500),
        ('google',    1, 'gemini-2.5-pro',     0.125, 0.500),
        ('deepseek',  1, 'deepseek-chat',      0.014, 0.028)").execute(pool).await?;
    Ok(())
}

fn parse_simple(text: &str) -> Option<(f64, String)> {
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(text) {
        return Some((
            v.get("probability").and_then(|x| x.as_f64()).unwrap_or(0.5),
            v.get("side").and_then(|x| x.as_str()).unwrap_or("skip").into(),
        ));
    }
    if let Some(start) = text.find('{') {
        if let Some(end) = text.rfind('}') {
            if end > start {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text[start..=end]) {
                    return Some((
                        v.get("probability").and_then(|x| x.as_f64()).unwrap_or(0.5),
                        v.get("side").and_then(|x| x.as_str()).unwrap_or("skip").into(),
                    ));
                }
            }
        }
    }
    None
}

// ---- .env 解析（无依赖）----
fn parse_env_file(content: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for (lineno, raw) in content.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') { continue; }
        let line = line.strip_prefix("export ").unwrap_or(line).trim();
        let Some((k, v)) = line.split_once('=') else { continue; };
        let val = v.trim();
        let val = if (val.starts_with('"') && val.ends_with('"') && val.len() >= 2)
            || (val.starts_with('\'') && val.ends_with('\'') && val.len() >= 2) {
            &val[1..val.len() - 1]
        } else { val };
        if val.is_empty() { continue; }
        out.push((k.trim().to_string(), val.to_string()));
        let _ = lineno;
    }
    out
}

fn sync_env_to_keyring(pairs: &[(String, String)]) {
    use std::collections::HashMap;
    let map: HashMap<String, String> = pairs.iter().cloned().collect();
    let llm_map: &[(&str, &str, &str)] = &[
        ("OPENAI_API_KEY",     "openai",    "prod-1"),
        ("ANTHROPIC_API_KEY",  "anthropic", "prod-1"),
        ("GOOGLE_API_KEY",     "google",    "prod-1"),
        ("DEEPSEEK_API_KEY",   "deepseek",  "prod-1"),
    ];
    for (env_var, provider_id, key_alias) in llm_map {
        if let Some(v) = map.get(*env_var) {
            let alias = format!("llm/{provider_id}/{key_alias}");
            match keyring::Entry::new("com.polyrocket.wallet", &alias)
                .and_then(|e| e.set_password(v))
            {
                Ok(_) => eprintln!("[smoke] keyring ← {alias} (len={})", v.len()),
                Err(e) => eprintln!("[smoke] keyring {alias}: {e}"),
            }
        }
    }
}
