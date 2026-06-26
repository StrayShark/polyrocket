//! L4 —— 主表结构迁移。
//!
//! v0.119 —— 将主表（wallets、markets、signals、bets）
//! 从 `seed.rs::apply_seed()` 抽离到独立的迁移中，
//! 每次启动都会执行。此前这些表只在首次运行的 seed 阶段
//! 创建，当其中任一表已存在时会被跳过。这导致一种"先有鸡
//! 还是先有蛋"的崩溃：先前已被 seed 过，但表被损坏或
//! 清空的数据库（例如 WAL 截断、手工 fs 手术）。
//!
//! 具有幂等性 —— 每条语句都是 `CREATE TABLE IF NOT EXISTS`。
//! 每次启动都可安全运行。

use sqlx::SqlitePool;

/// v0.119 —— 按依赖顺序运行所有主表迁移。
///
/// 顺序很关键：被外键引用的表必须先创建。我们故意不在
/// schema 中声明外键（SQLite 默认关闭外键强制检查；
/// 启用会令用户困惑），但逻辑上：
///   1. wallets  —— 独立
///   2. markets  —— 独立
///   3. signals  —— 引用 markets
///   4. bets     —— 引用 wallets + markets + signals
pub async fn ensure_primary_tables(pool: &SqlitePool) -> sqlx::Result<()> {
    // 1) wallets
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS wallets (
            id TEXT PRIMARY KEY,
            address TEXT NOT NULL UNIQUE,
            label TEXT,
            chain_id INTEGER DEFAULT 137 NOT NULL,
            wallet_type TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            last_synced_at INTEGER
        )",
    )
    .execute(pool)
    .await?;

    // 2) markets
    //
    // v0.119 —— 新增 `yes_price REAL`、`no_price REAL`、`closes_at INTEGER`、
    // `status TEXT`、`resolution_source TEXT`。`llm_analyze` IPC 的
    // `build_market_context`（commands/llm.rs）会 SELECT 这些列 —— 缺失
    // 时，第一次 analyze 调用会以 `no such column: resolution_source`
    // （以及其它列）崩溃。pre-v0.119 时该 SELECT 静默失败，
    // 在刚 seed 过的库上 llm_analyze 一次都没成功过。
    //
    // 所有新列都可空或带 DEFAULT，因此 seed.rs / sync_markets 写入的
    // 现有行依然有效。生产环境 sync_markets 仍需填充 yes/no_price
    //（目前只有 orderbook_snapshots 拥有这些字段）；现阶段
    // build_market_context 在列为空时回退为 NULL，LLM 仍能
    // 拿到一个有效（虽然稍稀疏）的市场上下文。
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS markets (
            id TEXT PRIMARY KEY,
            slug TEXT NOT NULL UNIQUE,
            question TEXT NOT NULL,
            description TEXT,
            category TEXT NOT NULL,
            tags TEXT,
            end_date INTEGER NOT NULL,
            active INTEGER DEFAULT 1 NOT NULL,
            resolved INTEGER DEFAULT 0 NOT NULL,
            outcome TEXT,
            liquidity TEXT,
            volume_24h REAL,
            yes_price REAL,
            no_price REAL,
            closes_at INTEGER,
            status TEXT,
            resolution_source TEXT,
            user_interested INTEGER DEFAULT 0 NOT NULL,
            brief_dismissed_at INTEGER,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        )",
    )
    .execute(pool)
    .await?;

    // 3) signals
    //
    // v0.119 —— 新增 `kind TEXT`、`summary TEXT`、`polarity TEXT DEFAULT 'neutral'`。
    // `llm_analyze` IPC 的 `build_market_context`（commands/llm.rs）会 SELECT
    // 这 3 列以填充 `SignalSummary { name, value, polarity }`
    // （prompts.rs）。pre-v0.119 时，每次 analyze 调用都会因
    // `no such column: s.kind` 崩溃。三列都是 TEXT/可空，
    // 现有 seed 行依旧有效。
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS signals (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            market_id TEXT NOT NULL,
            computed_at INTEGER NOT NULL,
            model_version TEXT NOT NULL,
            predicted_prob REAL NOT NULL,
            market_prob REAL NOT NULL,
            edge REAL NOT NULL,
            confidence REAL NOT NULL,
            horizon_hours INTEGER NOT NULL,
            rationale TEXT,
            kind TEXT,
            summary TEXT,
            polarity TEXT DEFAULT 'neutral',
            active INTEGER DEFAULT 1 NOT NULL
        )",
    )
    .execute(pool)
    .await?;

    // 4) bets
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS bets (
            id TEXT PRIMARY KEY,
            wallet_id TEXT NOT NULL,
            market_id TEXT NOT NULL,
            signal_id INTEGER,
            decision_id INTEGER,
            was_llm_assisted INTEGER DEFAULT 0 NOT NULL,
            mode TEXT NOT NULL,
            side TEXT NOT NULL,
            size TEXT NOT NULL,
            price REAL NOT NULL,
            shares TEXT NOT NULL,
            placed_at INTEGER NOT NULL,
            settled_at INTEGER,
            pnl TEXT,
            status TEXT NOT NULL,
            tx_hash TEXT,
            notes TEXT,
            order_type TEXT NOT NULL DEFAULT 'market',
            limit_price REAL,
            stop_price REAL,
            post_only INTEGER NOT NULL DEFAULT 0
        )",
    )
    .execute(pool)
    .await?;

    // 5) copy_targets（与 seed.rs 原始 schema 一致 —— 必须与
    // `commands::copy::add_copy_target` IPC 的 INSERT 语句匹配）
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS copy_targets (
            id TEXT PRIMARY KEY,
            address TEXT NOT NULL UNIQUE,
            label TEXT,
            enabled INTEGER DEFAULT 1 NOT NULL,
            allocation_cap TEXT,
            min_edge REAL DEFAULT 0.05 NOT NULL,
            created_at INTEGER NOT NULL
        )",
    )
    .execute(pool)
    .await?;

    // 6) copy_events（与 seed.rs 原始 schema 一致）
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS copy_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            target_id TEXT NOT NULL,
            market_id TEXT NOT NULL,
            detected_at INTEGER NOT NULL,
            side TEXT NOT NULL,
            size TEXT NOT NULL,
            price REAL NOT NULL,
            tx_hash TEXT NOT NULL UNIQUE,
            matched_bet_id TEXT
        )",
    )
    .execute(pool)
    .await?;

    // 7) audit_log（与 seed.rs 原始 schema 一致 —— 必须与
    // commands/{audit,llm,brief,bet,storage_migrate}.rs
    // 以及 infra::scheduler/mod.rs 中的 INSERT 语句匹配）
    //
    // v0.119 —— `at` 现在带有 DEFAULT，因此省略该列的
    // INSERT 语句（commands/ 中绝大多数）依然能成功。
    // 修复前，所有 audit_log 写入方都必须记得 `.bind(now_ms)`，
    // 否则会以 `NOT NULL constraint failed: audit_log.at` 崩溃。
    // DEFAULT 使用 SQLite 3.38+ 的 `unixepoch() * 1000` 获得
    // 毫秒级 unix 时间戳。
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS audit_log (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            at INTEGER NOT NULL DEFAULT (unixepoch() * 1000),
            actor TEXT NOT NULL,
            action TEXT NOT NULL,
            target TEXT,
            payload TEXT,
            result TEXT
        )",
    )
    .execute(pool)
    .await?;

    // 8) daily_briefs（在 fresh-init 流程中曾缺失）
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS daily_briefs (
            market_id TEXT PRIMARY KEY,
            rank INTEGER NOT NULL,
            match_score REAL NOT NULL,
            score_breakdown TEXT,
            computed_at INTEGER NOT NULL,
            expires_at INTEGER NOT NULL
        )",
    )
    .execute(pool)
    .await?;

    // 9) llm_analyses + llm_recommendations
    //
    // v0.119 —— 新增 `signal_id` + `triggered_by` 列。`llm_analyze` IPC
    // （commands/llm.rs:23-31）会同时写入这两列 —— 缺失时，首次 fresh init
    // 之后的 analyze 调用会因 `no column named signal_id` 崩溃。
    //
    // 同时为 `prompt_template` 和 `market_snapshot` 添加 DEFAULT。
    // IPC 中 pending 行的 INSERT（commands/llm.rs:1130 附近）两者都未设置，
    // 但列为 NOT NULL —— pre-v0.119 时每次 analyze 调用都会崩溃。
    // 添加 DEFAULT 后，行以 'pending' 状态落盘，template + snapshot
    // 为空字符串；后续 UPDATE 可填入（当前 IPC 暂未实现，但行至少有效）。
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS llm_analyses (
            id TEXT PRIMARY KEY,
            market_id TEXT NOT NULL,
            signal_id INTEGER,
            triggered_by TEXT NOT NULL DEFAULT '',
            prompt_version TEXT NOT NULL,
            prompt_template TEXT NOT NULL DEFAULT '',
            market_snapshot TEXT NOT NULL DEFAULT '{}',
            requested_at INTEGER NOT NULL,
            completed_at INTEGER,
            status TEXT NOT NULL,
            n_providers INTEGER NOT NULL DEFAULT 0,
            n_success INTEGER NOT NULL DEFAULT 0,
            n_failed INTEGER NOT NULL DEFAULT 0,
            consensus_predicted REAL,
            consensus_side TEXT,
            consensus_conf REAL,
            total_latency_ms INTEGER,
            cost_cents REAL
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS llm_recommendations (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            analysis_id TEXT NOT NULL,
            provider_id TEXT NOT NULL,
            model TEXT NOT NULL DEFAULT '',
            side TEXT,
            confidence REAL,
            predicted_prob REAL,
            rationale TEXT,
            reasoning TEXT,
            raw_response TEXT,
            latency_ms INTEGER NOT NULL,
            tokens_in INTEGER,
            tokens_out INTEGER,
            cost_cents REAL,
            error_kind TEXT,
            error_message TEXT,
            parse_ok INTEGER NOT NULL DEFAULT 1,
            parse_error TEXT,
            created_at INTEGER NOT NULL
        )",
    )
    .execute(pool)
    .await?;

    // 10) llm_providers（被 llm_provider_keys + llm_health_checks 引用）。
    // Schema 必须与 commands::llm::upsert_llm_provider 和
    // commands::llm_mgmt 中的 INSERT 语句一致。包含 llm_mgmt UPSERT
    // 路径的全部字段，使 fresh DB 能托管完整的 provider 配置。
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS llm_providers (
            id TEXT PRIMARY KEY,
            display_name TEXT NOT NULL,
            provider_kind TEXT,
            request_format TEXT,
            supports_streaming INTEGER DEFAULT 0,
            enabled INTEGER DEFAULT 1 NOT NULL,
            api_base TEXT,
            key_alias TEXT,
            default_model TEXT,
            timeout_ms INTEGER DEFAULT 30000,
            request_timeout_ms INTEGER DEFAULT 30000,
            max_retries INTEGER DEFAULT 3,
            cost_per_1k_in REAL DEFAULT 0,
            cost_per_1k_out REAL DEFAULT 0,
            rate_limit_rpm INTEGER,
            rate_limit_tpm INTEGER,
            quota_daily_cents INTEGER,
            quota_monthly_cents INTEGER,
            key_rotation_strategy TEXT,
            health_status TEXT,
            health_latency_p50_ms INTEGER,
            health_latency_p95_ms INTEGER,
            last_health_check_at INTEGER,
            last_health_error TEXT,
            notes TEXT,
            updated_at INTEGER
        )",
    )
    .execute(pool)
    .await?;

    // 11) llm_provider_keys（被 llm_health_checks + keyring 引用）
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS llm_provider_keys (
            id TEXT PRIMARY KEY,
            provider_id TEXT NOT NULL,
            alias TEXT NOT NULL,
            keyring_alias TEXT NOT NULL,
            enabled INTEGER DEFAULT 1 NOT NULL,
            priority INTEGER DEFAULT 0,
            weight REAL DEFAULT 1.0,
            last_used_at INTEGER,
            last_error TEXT,
            last_error_at INTEGER,
            total_calls INTEGER DEFAULT 0,
            total_errors INTEGER DEFAULT 0,
            notes TEXT,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        )",
    )
    .execute(pool)
    .await?;

    // 12) llm_health_checks（后台监控 + scheduler 读取）
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS llm_health_checks (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            provider_id TEXT NOT NULL,
            key_id TEXT,
            checked_at INTEGER NOT NULL,
            trigger TEXT NOT NULL,
            success INTEGER NOT NULL,
            latency_ms INTEGER,
            http_status INTEGER,
            error_code TEXT,
            error_message TEXT,
            model_used TEXT
        )",
    )
    .execute(pool)
    .await?;

    // 13) model_performance（被 pnl.rs 的 brier_score 查询使用）
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS model_performance (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            model_version TEXT NOT NULL,
            category TEXT,
            window_start INTEGER NOT NULL,
            window_end INTEGER NOT NULL,
            brier_score REAL,
            log_loss REAL,
            n_predictions INTEGER NOT NULL DEFAULT 0,
            n_resolved INTEGER NOT NULL DEFAULT 0,
            computed_at INTEGER NOT NULL
        )",
    )
    .execute(pool)
    .await?;

    // 14) llm_call_logs（按调用记录：延迟、费用、成功 —— 为
    // scheduler 成本异常检测和 LLM 性能仪表盘供数）
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS llm_call_logs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            analysis_id TEXT,
            provider_id TEXT NOT NULL,
            key_id TEXT,
            called_at INTEGER NOT NULL,
            latency_ms INTEGER NOT NULL,
            tokens_in INTEGER,
            tokens_out INTEGER,
            cost_cents REAL,
            http_status INTEGER,
            success INTEGER NOT NULL,
            error_code TEXT,
            error_message TEXT,
            prompt_version TEXT,
            predicted_prob REAL,
            recommended_side TEXT,
            caller TEXT,
            retry_count INTEGER DEFAULT 0
        )",
    )
    .execute(pool)
    .await?;

    // 15) llm_decisions（审计轨迹：用户是否遵循了 LLM 的建议？
    // 为 commands/llm.rs 中的"LLM 是否正确？"评分供数）
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS llm_decisions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            analysis_id TEXT NOT NULL,
            bet_id TEXT,
            user_decision TEXT NOT NULL,
            user_decided_side TEXT,
            followed_llm_id INTEGER,
            decided_at INTEGER NOT NULL,
            context_snapshot TEXT
        )",
    )
    .execute(pool)
    .await?;

    // 16) user_brief_prefs（按用户的 daily-brief 偏好）
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS user_brief_prefs (
            user_id TEXT PRIMARY KEY,
            weights_json TEXT NOT NULL,
            max_items INTEGER NOT NULL DEFAULT 5,
            min_liquidity TEXT NOT NULL DEFAULT '0',
            categories TEXT,
            updated_at INTEGER NOT NULL
        )",
    )
    .execute(pool)
    .await?;

    // 17) orderbook_snapshots（完整的 bid/ask 深度 —— 为 LLM 市场
    // 上下文供数。区别于 price_snapshots，后者只保存
    // best bid/ask 的中间价。）
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS orderbook_snapshots (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            market_id TEXT NOT NULL,
            captured_at INTEGER NOT NULL,
            best_bid REAL NOT NULL,
            best_ask REAL NOT NULL,
            bid_depth REAL,
            ask_depth REAL
        )",
    )
    .execute(pool)
    .await?;

    // P1-1 —— news_items（新闻关联器）
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS news_items (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT NOT NULL,
            source TEXT NOT NULL,
            url TEXT NOT NULL,
            published_at INTEGER NOT NULL,
            market_id TEXT,
            relevance_score REAL,
            impact_direction TEXT,
            summary TEXT,
            created_at INTEGER NOT NULL
        )",
    )
    .execute(pool)
    .await?;

    // P1-3 —— arb_opportunities（套利扫描器 + 跨平台）
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS arb_opportunities (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            market_id TEXT NOT NULL,
            question TEXT NOT NULL,
            yes_cost REAL NOT NULL,
            no_cost REAL NOT NULL,
            total_cost REAL NOT NULL,
            profit_margin REAL NOT NULL,
            platform TEXT DEFAULT 'polymarket' NOT NULL,
            detected_at INTEGER NOT NULL,
            expires_at INTEGER
        )",
    )
    .execute(pool)
    .await?;

    // v0.119 —— 针对已经 seed 过的数据库的加性列迁移。
    // `CREATE TABLE IF NOT EXISTS` 在表已存在且 schema 不同时
    // 是 no-op，因此我们还需要对 v0.119+ 新增的列显式执行
    // `ALTER TABLE ADD COLUMN`。如果尝试添加已存在的列，
    // SQLite 会抛出"duplicate column"错误，所以每条 ALTER
    // 都被包装在 sqlite 层的 try 中（用 `changes()` 函数返回 0
    // 来吞掉 duplicate-column 错误并非标准做法，因此我们
    // 通过子查询守卫接受错误）。
    //
    // audit_log：pre-v0.119 创建时没有 `at` DEFAULT。我们防御性
    //   地添加（若已存在 DEFAULT 则 no-op）。
    // llm_analyses：pre-v0.119 创建时没有 `signal_id` /
    //   `triggered_by`。两列均可空，对已有行安全。
    let _ = sqlx::query("ALTER TABLE llm_analyses ADD COLUMN signal_id INTEGER")
        .execute(pool).await;  // 忽略 "duplicate column" 错误
    let _ = sqlx::query("ALTER TABLE llm_analyses ADD COLUMN triggered_by TEXT NOT NULL DEFAULT ''")
        .execute(pool).await;

    // markets：v0.119 新增 yes_price / no_price / closes_at / status /
    // resolution_source 以便 llm_analyze 可以 SELECT 它们。
    // 对已 seed 的数据库防御性添加（上方进程内的 CREATE TABLE IF NOT
    // EXISTS 在 markets 已存在时是 no-op）。
    let _ = sqlx::query("ALTER TABLE markets ADD COLUMN yes_price REAL")
        .execute(pool).await;
    let _ = sqlx::query("ALTER TABLE markets ADD COLUMN no_price REAL")
        .execute(pool).await;
    let _ = sqlx::query("ALTER TABLE markets ADD COLUMN closes_at INTEGER")
        .execute(pool).await;
    let _ = sqlx::query("ALTER TABLE markets ADD COLUMN status TEXT")
        .execute(pool).await;
    let _ = sqlx::query("ALTER TABLE markets ADD COLUMN resolution_source TEXT")
        .execute(pool).await;

    // signals：v0.119 新增 kind / summary / polarity，以便 llm_analyze
    // 可以在 build_market_context 中 SELECT 它们。对已 seed 的
    // 数据库防御性添加。
    let _ = sqlx::query("ALTER TABLE signals ADD COLUMN kind TEXT")
        .execute(pool).await;
    let _ = sqlx::query("ALTER TABLE signals ADD COLUMN summary TEXT")
        .execute(pool).await;
    let _ = sqlx::query("ALTER TABLE signals ADD COLUMN polarity TEXT DEFAULT 'neutral'")
        .execute(pool).await;

    // llm_recommendations：v0.119 新增 `reasoning` 列。`llm_analyze`
    // 会按条建议写入 reasoning 文本（commands/llm.rs:715-735）。
    // pre-v0.119 时每次 analyze 调用都因 `no such column: reasoning`
    // 崩溃。可空列，因此已有行依旧有效。
    let _ = sqlx::query("ALTER TABLE llm_recommendations ADD COLUMN reasoning TEXT")
        .execute(pool).await;
    let _ = sqlx::query("ALTER TABLE llm_recommendations ADD COLUMN parse_ok INTEGER NOT NULL DEFAULT 1")
        .execute(pool).await;
    let _ = sqlx::query("ALTER TABLE llm_recommendations ADD COLUMN parse_error TEXT")
        .execute(pool).await;

    // 18) spike_alerts（P0-3 —— 尖峰检测）。存储由
    // `commands::spike::run_spike_scan` 写入的检测到的价格尖峰。
    // `notified` 跟踪前端是否已为该 alert 展示通知
    //（0 = 待通知，1 = 已通知）。
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS spike_alerts (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            market_id TEXT NOT NULL,
            old_price REAL NOT NULL,
            new_price REAL NOT NULL,
            change_pct REAL NOT NULL,
            detected_at INTEGER NOT NULL,
            notified INTEGER DEFAULT 0 NOT NULL
        )",
    )
    .execute(pool)
    .await?;

    // Phase 1.7 —— 用于 scan_market_anomalies scheduler 循环的
    // market_anomalies 表。存储周期性后台扫描器检测到的价格
    // 尖峰、成交量激增与均值回归信号。
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS market_anomalies (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            market_id TEXT NOT NULL,
            kind TEXT NOT NULL,           -- 'price_spike' | 'volume_surge' | 'reversion'
            severity TEXT NOT NULL,       -- 'low' | 'medium' | 'high'
            payload TEXT,                 -- JSON details
            detected_at INTEGER NOT NULL,
            notified INTEGER DEFAULT 0 NOT NULL
        )",
    )
    .execute(pool)
    .await?;

    // v0.127 —— market_signal_cache 表：用于 signals_refresh 调度循环
    // 缓存 smart_money_score + crowd_opinion 的计算结果。
    // 避免 L1 每次进入 MarketDetail 都触发一次重计算(涉及 bets + wallets JOIN)。
    // scheduler 每 30 分钟 (POLYROCKET_SIGNALS_REFRESH_MIN) 重新计算所有
    // 活跃 market 的两个分数并 UPSERT,MarketDetail 优先读 cache
    // (由 IPC 内部判断 cache 是否新鲜,< 30 min 走 cache,
    // 否则现场重算并写回)。
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS market_signal_cache (
            market_id TEXT PRIMARY KEY,
            smart_money_yes REAL NOT NULL,
            smart_money_no REAL NOT NULL,
            crowd_yes REAL NOT NULL,
            crowd_no REAL NOT NULL,
            computed_at INTEGER NOT NULL,
            bet_count INTEGER NOT NULL DEFAULT 0
        )",
    )
    .execute(pool)
    .await?;

    Ok(())
}
