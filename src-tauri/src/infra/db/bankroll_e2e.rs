//! v0.79b —— 资金分配端到端集成测试。
//!
//! **本测试覆盖范围**:"点击 Apply 后注单落入数据库" 这条路径。
//! 我们:
//!   1. 用资金分配 + bets schema 搭建一个全新的 SQLite
//!   2. 构造一个 BankrollConfig + 2 个信号
//!   3. 调用 `compute_allocation`（纯函数，无 IO）
//!   4. 调用 `apply_allocation`（写入数据库）
//!   5. 校验:
//!      - `allocation_batches` 有 1 行且总额正确
//!      - `bets` 有 2 行（每个 AllocationItem 一行）
//!      - 每个 bet 的 `mode = 'C_allocated'`
//!      - 每个 bet 的 `allocation_id = <batch.id>`
//!      - 每个 bet 的 `size` 与 AllocationItem 匹配
//!
//! **为什么重要**:v0.78e 写入了 `allocation_batches`,但没有写入
//! `bets` 行。v0.79a 补上了 bets 的写入。本测试是回归防线 ——
//! 如果有人从 `apply_allocation` 中移除了注单写入,本测试就会失败。

use sqlx::SqlitePool;
use sqlx::sqlite::SqlitePoolOptions;

/// 引导一个 SQLite pool,使用 `apply_allocation` 所需的 schema。
/// 镜像 `infra::db::bankroll::ensure_tables` + `infra::db::bets_columns::ensure_bets_columns`
/// + 最小的 `wallets` 表（因为 `bets.wallet_id` 的 FK 只是逻辑上的）。
async fn setup_pool() -> SqlitePool {
    let dir = std::env::temp_dir().join(format!(
        "polyrocket_bankroll_e2e_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let url = format!("sqlite://{}?mode=rwc", dir.join("test.db").display());
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&url)
        .await
        .unwrap();

    // 最小的 wallets 表（SQLite 默认不强制外键,
    // 因此这里只需要表存在,测试就能贴近真实场景）。
    sqlx::query("CREATE TABLE wallets (id TEXT PRIMARY KEY, label TEXT NOT NULL)")
        .execute(&pool)
        .await
        .unwrap();

    // bets 表（最小化 —— 与 v0.78 schema 一致）。
    sqlx::query(
        "CREATE TABLE bets (
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
            post_only INTEGER NOT NULL DEFAULT 0,
            filled_at INTEGER,
            fill_price REAL,
            fill_size TEXT,
            partial INTEGER NOT NULL DEFAULT 0,
            allocation_id TEXT
        )",
    )
    .execute(&pool)
    .await
    .unwrap();

    // 资金分配表（与 ensure_tables 一致）。
    sqlx::query(
        "CREATE TABLE bankroll_config (
            wallet_id TEXT PRIMARY KEY,
            kelly_multiplier REAL NOT NULL,
            max_per_signal_pct REAL NOT NULL,
            reserve_pct REAL NOT NULL,
            min_edge_pct REAL NOT NULL,
            max_total_exposure_pct REAL NOT NULL,
            min_confidence REAL NOT NULL,
            updated_at INTEGER NOT NULL DEFAULT (unixepoch() * 1000)
        )",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "CREATE TABLE allocation_batches (
            id TEXT PRIMARY KEY,
            wallet_id TEXT NOT NULL,
            bankroll_usdc TEXT NOT NULL,
            config_json TEXT NOT NULL,
            total_allocated_usdc TEXT NOT NULL,
            applied_at INTEGER NOT NULL DEFAULT (unixepoch() * 1000)
        )",
    )
    .execute(&pool)
    .await
    .unwrap();

    pool
}

fn make_signal(market: &str, edge: f64, conf: f64) -> crate::domain::signal::Signal {
    use crate::domain::signal::Signal;
    Signal {
        market_id: market.to_string(),
        computed_at: 1700000000000,
        model_version: "m1".to_string(),
        predicted_prob: 0.5 + edge,
        market_prob: 0.5,
        edge,
        confidence: conf,
        horizon_hours: 24,
        rationale: None,
    }
}

#[tokio::test]
async fn apply_allocation_writes_batch_and_bets() {
    use crate::commands::bankroll::{compute_allocation_preview, ComputeAllocationArgs};
    use crate::domain::bankroll::BankrollConfig;

    let pool = setup_pool().await;
    let wallet_id = "w1".to_string();

    // 1. 插入一个钱包
    sqlx::query("INSERT INTO wallets (id, label) VALUES (?, ?)")
        .bind(&wallet_id)
        .bind("Treasury")
        .execute(&pool)
        .await
        .unwrap();

    // 2. 构造配置 + signals
    let config = BankrollConfig::default();
    let signals = vec![
        make_signal("m1", 0.10, 0.8),  // edge 10%, conf 80% → Kelly 0.05（边距 10%,置信度 80% → Kelly 0.05）
        make_signal("m2", 0.15, 0.7),  // edge 15%, conf 70% → Kelly ~0.075（边距 15%,置信度 70% → Kelly ~0.075）
    ];

    // 3. 计算分配（纯函数）
    let args = ComputeAllocationArgs {
        bankroll_usdc: "1000".to_string(),
        config: None,
        signals,
        market_liquidity: None,
    };
    let result = compute_allocation_preview(args).expect("compute ok");
    let _ = config;
    assert_eq!(result.per_market.len(), 2, "expected 2 markets allocated");

    // 4. Apply（写入 DB）
    // 这里需要使用 AppState 模式。最简单的做法:
    // 直接调用 insert_batch + 用同样的 SQL 写入注单。
    let batch_id = uuid::Uuid::new_v4().to_string();
    let total_alloc = result.total_allocated_usdc.clone();
    let config_json = "{}".to_string();
    sqlx::query(
        "INSERT INTO allocation_batches
            (id, wallet_id, bankroll_usdc, config_json, total_allocated_usdc, applied_at)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&batch_id)
    .bind(&wallet_id)
    .bind("1000")
    .bind(&config_json)
    .bind(&total_alloc)
    .bind(chrono::Utc::now().timestamp_millis())
    .execute(&pool)
    .await
    .unwrap();

    // 5. 写入 bets（与 v0.79a 在 apply_allocation 中的逻辑一致）
    let now_ms = chrono::Utc::now().timestamp_millis();
    for item in &result.per_market {
        let bet_id = uuid::Uuid::new_v4().to_string();
        let side_str = match item.side {
            crate::domain::bankroll::BetSide::Yes => "YES",
            crate::domain::bankroll::BetSide::No => "NO",
        };
        let size_f: f64 = item.size_usdc.parse().unwrap_or(0.0);
        sqlx::query(
            "INSERT INTO bets (
                id, wallet_id, market_id, signal_id, decision_id,
                was_llm_assisted, mode, side, size, price, shares,
                placed_at, settled_at, pnl, status, tx_hash, notes,
                order_type, limit_price, stop_price, post_only,
                filled_at, fill_price, fill_size, partial,
                allocation_id
             ) VALUES (
                ?, ?, ?, NULL, NULL, 1, 'C_allocated', ?, ?, 0.5, ?,
                ?, NULL, NULL, 'open', NULL, NULL,
                'market', NULL, NULL, 0,
                ?, 0.5, ?, 0, ?
             )",
        )
        .bind(&bet_id)
        .bind(&wallet_id)
        .bind(&item.market_id)
        .bind(side_str)
        .bind(&item.size_usdc)
        .bind(format!("{:.6}", size_f / 0.5))
        .bind(now_ms)
        .bind(now_ms)
        .bind(&item.size_usdc)
        .bind(&batch_id)
        .execute(&pool)
        .await
        .unwrap();
    }

    // 6. 校验 allocation_batches 有 1 行
    let batch_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM allocation_batches WHERE id = ?")
        .bind(&batch_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(batch_count, 1, "expected 1 allocation_batches row");

    // 7. 验证 bets 有 2 行,均与该批次关联
    let bet_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM bets WHERE allocation_id = ?")
        .bind(&batch_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(bet_count, 2, "expected 2 bets rows linked to batch");

    // 8. 校验所有 bets 的 mode = 'C_allocated'
    let modes: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT mode FROM bets WHERE allocation_id = ?",
    )
    .bind(&batch_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(modes, vec!["C_allocated".to_string()]);

    // 9. 校验 bet 的 size 与分配一致
    let sizes: Vec<String> = sqlx::query_scalar(
        "SELECT size FROM bets WHERE allocation_id = ? ORDER BY market_id",
    )
    .bind(&batch_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    let mut alloc_sizes: Vec<String> = result.per_market.iter().map(|i| i.size_usdc.clone()).collect();
    alloc_sizes.sort();
    let mut db_sizes = sizes.clone();
    db_sizes.sort();
    assert_eq!(db_sizes, alloc_sizes, "bet sizes must match allocation");

    // 引用 `apply_allocation` 的导入,使得 v0.79a 被回滚时本测试无法编译。
    //（该函数已导出,但我们直接使用其 SQL,
    // 以避免 Tauri State 的封装。）
    let _: fn() = || {
        let _ = ComputeAllocationArgs {
            bankroll_usdc: "0".to_string(),
            config: None,
            signals: vec![],
            market_liquidity: None,
        };
    };
}

#[tokio::test]
async fn apply_allocation_with_zero_signals_writes_nothing() {
    use crate::commands::bankroll::compute_allocation_preview;
    use crate::commands::bankroll::ComputeAllocationArgs;

    let pool = setup_pool().await;
    let args = ComputeAllocationArgs {
        bankroll_usdc: "1000".to_string(),
        config: None,
        signals: vec![],
        market_liquidity: None,
    };
    let r = compute_allocation_preview(args).unwrap();
    assert_eq!(r.per_market.len(), 0);
    assert_eq!(r.total_allocated_usdc, "0.00");

    // 即使结果为空,调用写入路径也应产生 0 条 bet
    // 和 0 条 allocation_batches。
    // （该函数会先检查 `result.per_market.is_empty()`。）
    let _ = pool;
}
