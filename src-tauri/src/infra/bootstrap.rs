//! L4 —— v0.123 开发态 bootstrap。
//!
//! 启动时,当 `POLYROCKET_ENV=dev`(且 keyring-only 模式
//! 关闭 —— v0.119 的默认)时,检查加载的 `.env` 并
//! 自注册 LLM provider 行 + PM wallet 行,让 L1 有
//! 内容可渲染。这消除了 v0.121 UI E2E 阻塞的
//! "全新 DB 没什么可显示" 问题
//!（之前这些行只在 Welcome 流程中创建;
//! 跳过 Welcome 就会留下空 dashboard）。
//!
//! 幂等:每次操作都是基于确定 id 的 `INSERT OR IGNORE`
//! 或 `INSERT OR REPLACE`,所以每次启动都调用
//! `bootstrap()` 都是安全的,且收敛到规范状态。
//!
//! 分层规则:本模块位于 L4(基础设施),使用
//! L4 `infra::db::pool` + L4 `platform::env`(.env 加载器
//! 已填充 process env)。它**不**调
//! L2/L3 IPC handler —— LLM provider upsert
//! 直接走 SQL 路径(逐字复制
//! `commands::llm_mgmt::llm_provider_upsert`,去掉
//! specta 包装)。

use crate::infra::error::AppResult;
use crate::infra::state::AppState;
use serde_json::json;
use sqlx::SqlitePool;
use tauri::AppHandle;
use tauri::Manager;

/// 运行 bootstrap。可在 `lib.rs::run()` 的
/// setup hook 中每次启动都安全调用。错误会
/// 记录但**不**让启动失败 —— 即使 .env 缺失
/// 或 DB 只读,app 仍可使用。
pub fn bootstrap(app: &AppHandle) {
    // 门控:与 .env loader 同规则。缺少 dev-mode
    // 标志时我们不会自动注册(真实生产用户
    // 期望自己粘贴密钥)。
    let env_name = std::env::var("POLYROCKET_ENV").unwrap_or_default();
    let keyring_only = std::env::var("POLYROCKET_KEYRING_ONLY")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    if env_name != "dev" || keyring_only {
        tracing::info!(
            "v0.123 bootstrap: skipped (env={env_name:?}, keyring_only={keyring_only})"
        );
        return;
    }

    let pool = app.state::<AppState>().db.clone();

    // 我们已经在 Tauri async runtime 中
    //（setup hook 跑在它上面）。直接 spawn
    // task —— **不**要在这里用 `block_on`,
    // 否则会 panic "Cannot start a runtime
    // from within a runtime"。
    tauri::async_runtime::spawn(async move {
        match do_bootstrap(&pool).await {
            Ok(n) => tracing::info!("v0.123 bootstrap: {n} action(s) applied"),
            Err(e) => tracing::warn!("v0.123 bootstrap: failed: {e}"),
        }
    });
}

async fn do_bootstrap(pool: &SqlitePool) -> AppResult<usize> {
    let mut actions = 0usize;
    actions += register_llm_providers(pool).await?;
    actions += register_pm_wallet(pool).await?;
    actions += expire_stale_markets(pool).await?;
    Ok(actions)
}

/// 遍历候选 LLM provider 列表。对每个设置了
/// `*_API_KEY` 环境变量的,在 `llm_providers` +
/// `llm_provider_keys`(LLM dispatch IPC 读取的
/// 两张表)里 `INSERT OR IGNORE` 一行。
async fn register_llm_providers(pool: &SqlitePool) -> AppResult<usize> {
    // 候选镜像自 `e2e_football::bootstrap_providers`。
    // 这里的 id 是 SHORT 形式(如 `MiniMax`);
    // `llm_providers.id` 列存的就是这种 short 形式,
    // `pick_keys` 也按它匹配。环境变量名为
    // `{id_uppercase}_API_KEY`(如 `MINIMAX_API_KEY`)。
    let candidates: &[(&str, &str, &str, &str, f64, f64)] = &[
        ("MiniMax", "MiniMax (M2.7)",         "https://api.minimax.chat/v1",                "MiniMax-M2.7",                 0.4,  1.2),
        ("doubao",  "Doubao (火山方舟)",        "https://ark.cn-beijing.volces.com/api/coding/v3",   "doubao-seed-2-0-pro-260215",   0.08, 0.08),
        ("qwen",    "Qwen (DashScope)",        "https://dashscope.aliyuncs.com/compatible-mode/v1","qwen-plus",                    0.4,  1.2),
        ("moonshot","Moonshot (Kimi)",         "https://api.moonshot.cn/v1",                 "kimi-k2-0711-preview",          0.6,  0.6),
        ("zhipu",   "Zhipu (GLM)",             "https://open.bigmodel.cn/api/paas/v4",       "glm-4-plus",                  5.0,  5.0),
    ];

    let now_ms = chrono::Utc::now().timestamp_millis();
    let mut count = 0usize;

    for (id, display, api_base, model, ci, co) in candidates {
        let env_var = format!("{}_API_KEY", id.to_uppercase());
        let secret = match std::env::var(&env_var).ok().filter(|v| !v.is_empty()) {
            Some(s) => s,
            None => continue,
        };

        // 1) upsert 到 llm_providers。id 列存的是
        //    SHORT 形式(如 `MiniMax`)—— 与
        //    `llm_provider_keys.provider_id` 形式一致。
        let upsert_ok = sqlx::query(
            "INSERT INTO llm_providers
                (id, display_name, provider_kind, request_format, supports_streaming,
                 enabled, api_base, key_alias, default_model, timeout_ms,
                 request_timeout_ms, max_retries, cost_per_1k_in, cost_per_1k_out,
                 key_rotation_strategy, health_status, updated_at, notes)
             VALUES (?, ?, 'openai_compat', 'openai_compat', 0, 1, ?, 'prod-1',
                     ?, 120000, 120000, 2, ?, ?, 'round_robin', 'unknown', ?, 'bootstrapped by v0.123 dev-env sync')
             ON CONFLICT(id) DO UPDATE SET
                api_base = excluded.api_base,
                default_model = excluded.default_model,
                enabled = 1,
                updated_at = excluded.updated_at",
        )
        .bind(id)
        .bind(display)
        .bind(api_base)
        .bind(model)
        .bind(*ci)
        .bind(*co)
        .bind(now_ms)
        .execute(pool)
        .await;

        if let Err(e) = upsert_ok {
            tracing::warn!("bootstrap: llm_providers upsert for {id} failed: {e}");
            continue;
        }

        // 2) 插入对应的 llm_provider_keys 行(dispatch IPC
        //    读这张表 —— 没有行就 "no enabled keys")。
        let key_id = format!("{id}-prod-1-bootstrap");
        let keyring_alias = format!("llm/{id}/prod-1");
        let ins = sqlx::query(
            "INSERT OR IGNORE INTO llm_provider_keys
                (id, provider_id, alias, keyring_alias, enabled, priority, weight,
                 created_at, updated_at, notes)
             VALUES (?, ?, 'prod-1', ?, 1, 0, 1.0, ?, ?, 'bootstrapped by v0.123 dev-env sync')",
        )
        .bind(&key_id)
        .bind(id)
        .bind(&keyring_alias)
        .bind(now_ms)
        .bind(now_ms)
        .execute(pool)
        .await;

        match ins {
            Ok(r) if r.rows_affected() > 0 => {
                count += 1;
                tracing::info!("bootstrap: registered {id} (key_len={})", secret.len());
            }
            Ok(_) => {
                // 已存在 —— 幂等跳过
            }
            Err(e) => {
                tracing::warn!("bootstrap: llm_provider_keys insert for {id} failed: {e}");
            }
        }
    }

    Ok(count)
}

/// 为 L2 PM 地址插入占位 `wallets` 行,让 L1
/// 有 wallet 可显示。我们**不**从 .env
/// 拿地址(.env 只有 L2 API key/secret/passphrase,
/// 没有 L1 polygon 地址);我们把 address 留空,
/// 让 Welcome 流程在用户想用 Mode A 时填进去。
///
/// 对 v0.123 简化的 UI 来说,这一行只是
/// 让 `list_wallets` 至少返回一条
///（让侧栏 footer 能显示 "1 wallet"）。
/// 它不带余额 —— 余额 IPC 直连 CLOB API。
async fn register_pm_wallet(pool: &SqlitePool) -> AppResult<usize> {
    // 若已有 wallet 则跳过。
    let existing: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM wallets")
        .fetch_one(pool)
        .await?;
    if existing > 0 {
        return Ok(0);
    }
    let now_ms = chrono::Utc::now().timestamp_millis();
    let id = "wallet-bootstrap-pm".to_string();
    let label = "PM (auto-bootstrapped)".to_string();
    sqlx::query(
        "INSERT INTO wallets (id, address, label, chain_id, wallet_type, created_at)
         VALUES (?, '', ?, 137, 'pm-l2', ?)",
    )
    .bind(&id)
    .bind(&label)
    .bind(now_ms)
    .execute(pool)
    .await?;
    tracing::info!("bootstrap: registered placeholder PM wallet (L2 creds only, no L1 address)");
    Ok(1)
}

// 抑制 `json!` 宏 import 的 unused 警告
//（保留供将来扩展写更丰富的 provider metadata 时使用）。
#[allow(dead_code)]
fn _force_json_use() -> serde_json::Value {
    json!({})
}

/// v0.123 —— 每次启动时过期陈旧 market。
///
/// seed bundle(以及任何未来的 fixture)硬编码了 `end_date`
/// 值。随着真实时间前进,`end_date` 已过的 market 不应再
/// 出现在 active 列表中。没有这次扫描,L1 会一直
/// 在已关闭数月的比赛上显示 "active" 徽标
///（例如 1 月的 UCL 决赛、5 月的 La Liga —— 在
/// 2026-06-23 仍然可见,虽然两项赛事都已结束）。
///
/// 修复办法:每次 bootstrap 时,把满足
/// `end_date < now_ms AND active=1` 的 market 翻成
/// `active=0`。幂等且轻量(一次 UPDATE,通常
/// 第一次之后影响 0 行)。
///
/// **范围**:本函数只翻 `active` 标志 —— **不**碰
/// `resolved` / `outcome`（那两项由 PM sync 在
/// 真实结果到达时设置）。对没有 PM 链接的 seed
/// 数据,`resolved` 保持 0,但 `active=0` 足以让
/// 这行从 active-only 列表中隐藏。
async fn expire_stale_markets(pool: &SqlitePool) -> AppResult<usize> {
    let now_ms = chrono::Utc::now().timestamp_millis();
    let result = sqlx::query(
        "UPDATE markets SET active = 0
         WHERE active = 1
           AND end_date IS NOT NULL
           AND end_date < ?",
    )
    .bind(now_ms)
    .execute(pool)
    .await?;
    let n = result.rows_affected() as usize;
    if n > 0 {
        tracing::info!("bootstrap: expired {n} stale market(s) (end_date < now)");
    }
    Ok(n)
}
