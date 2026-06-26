//! L4 —— 数据库种子填充器（幂等）。
//!
//! 读取 `domain::seed::SeedBundle::demo()` 并把每条 row 应用到
//! SQLite pool。幂等：每条 row 都使用固定主键,因此连续调用两次
//! `apply_seed` 会产生相同的 DB 状态（INSERT OR REPLACE）。
//!
//! 触发方式：首次启动时由 `init_pool` 自动调用,也可由
//! `commands::seed::seed_demo_data(force=true)` 显式重新填充。

use crate::domain::seed::{SeedBundle, SEED_AUDIT_ACTION, SEED_NOW_MS};
use crate::infra::error::AppResult;
use sqlx::SqlitePool;

/// 若 DB 已有 demo 数据（>=1 个 wallet、>=1 个 market、
/// >=1 个 bet）则返回 true。用于在后续启动时跳过自动 seed。
pub async fn is_seeded(pool: &SqlitePool) -> AppResult<bool> {
    let wallets: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM wallets")
        .fetch_one(pool)
        .await?;
    let markets: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM markets")
        .fetch_one(pool)
        .await?;
    let bets: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM bets")
        .fetch_one(pool)
        .await?;
    Ok(wallets > 0 && markets > 0 && bets > 0)
}

/// 将标准 demo bundle 写入 DB。返回所有表中
/// 插入/替换的 row 总数。
///
/// `force=true` 总是重新写入（用于开发用的 "reset demo data" 按钮）;
/// `force=false` 时,若 DB 已被 seed 则为 no-op。
pub async fn apply_seed(pool: &SqlitePool, force: bool) -> AppResult<usize> {
    if !force && is_seeded(pool).await? {
        return Ok(0);
    }
    let bundle = SeedBundle::demo();
    let mut total = 0;

    // 1. Wallets
    for w in &bundle.wallets {
        sqlx::query(
            "INSERT OR REPLACE INTO wallets
                (id, address, label, chain_id, wallet_type, created_at, last_synced_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&w.id)
        .bind(&w.address)
        .bind(&w.label)
        .bind(w.chain_id)
        .bind(&w.wallet_type)
        .bind(w.created_at)
        .bind::<Option<i64>>(None)
        .execute(pool)
        .await?;
        total += 1;
    }

    // 2. Markets
    for m in &bundle.markets {
        sqlx::query(
            "INSERT OR REPLACE INTO markets
                (id, slug, question, description, category, tags, end_date,
                 active, resolved, outcome, liquidity, volume_24h,
                 user_interested, brief_dismissed_at, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0, NULL, ?, ?)",
        )
        .bind(&m.id)
        .bind(&m.slug)
        .bind(&m.question)
        .bind(&m.description)
        .bind(&m.category)
        .bind(&m.tags)
        .bind(m.end_date)
        .bind(if m.active { 1i64 } else { 0i64 })
        .bind(if m.resolved { 1i64 } else { 0i64 })
        .bind(&m.outcome)
        .bind(&m.liquidity)
        .bind(m.volume_24h)
        .bind(m.created_at)
        .bind(m.updated_at)
        .execute(pool)
        .await?;
        total += 1;
    }

    // 3. Signals
    // 我们需要恢复自增 ID，因为 bets 引用了它们。
    // 策略：清空并重新插入 signals，让 ID 变得确定
    //（在 demo 中这些 ID 不会出现在任何 bet.signal_id）。
    sqlx::query("DELETE FROM signals").execute(pool).await?;
    for s in &bundle.signals {
        sqlx::query(
            "INSERT INTO signals
                (market_id, computed_at, model_version, predicted_prob,
                 market_prob, edge, confidence, horizon_hours, rationale, active)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 1)",
        )
        .bind(&s.market_id)
        .bind(s.computed_at)
        .bind(&s.model_version)
        .bind(s.predicted_prob)
        .bind(s.market_prob)
        .bind(s.edge)
        .bind(s.confidence)
        .bind(s.horizon_hours)
        .bind(&s.rationale)
        .execute(pool)
        .await?;
        total += 1;
    }

    // 4. Bets
    for b in &bundle.bets {
        sqlx::query(
            "INSERT OR REPLACE INTO bets
                (id, wallet_id, market_id, signal_id, decision_id,
                 was_llm_assisted, mode, side, size, price, shares,
                 placed_at, settled_at, pnl, status, tx_hash, notes)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&b.id)
        .bind(&b.wallet_id)
        .bind(&b.market_id)
        .bind(b.signal_id)
        .bind(b.decision_id)
        .bind(if b.was_llm_assisted { 1i64 } else { 0i64 })
        .bind(&b.mode)
        .bind(&b.side)
        .bind(&b.size)
        .bind(b.price)
        .bind(&b.shares)
        .bind(b.placed_at)
        .bind(b.settled_at)
        .bind(&b.pnl)
        .bind(&b.status)
        .bind(&b.tx_hash)
        .bind(&b.notes)
        .execute(pool)
        .await?;
        total += 1;
    }

    // 5. Copy targets
    for t in &bundle.copy_targets {
        sqlx::query(
            "INSERT OR REPLACE INTO copy_targets
                (id, address, label, enabled, allocation_cap, min_edge, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&t.id)
        .bind(&t.address)
        .bind(&t.label)
        .bind(if t.enabled { 1i64 } else { 0i64 })
        .bind(&t.allocation_cap)
        .bind(t.min_edge)
        .bind(t.created_at)
        .execute(pool)
        .await?;
        total += 1;
    }

    // 6. Copy events
    for e in &bundle.copy_events {
        sqlx::query(
            "INSERT OR REPLACE INTO copy_events
                (target_id, market_id, detected_at, side, size, price, tx_hash, matched_bet_id)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&e.target_id)
        .bind(&e.market_id)
        .bind(e.detected_at)
        .bind(&e.side)
        .bind(&e.size)
        .bind(e.price)
        .bind(&e.tx_hash)
        .bind(&e.matched_bet_id)
        .execute(pool)
        .await?;
        total += 1;
    }

    // 7. 写入 audit 标记,使 seeder 行为在 Audit 页可见。
    sqlx::query(
        "INSERT INTO audit_log (at, actor, action, target, payload, result)
         VALUES (?, 'system', ?, 'demo_data', ?, 'ok')",
    )
    .bind(SEED_NOW_MS)
    .bind(SEED_AUDIT_ACTION)
    .bind(format!("{{\"rows\":{total}}}"))
    .execute(pool)
    .await?;
    total += 1;

    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn empty_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite");
        // 应用完整 schema（与 Drizzle 创建的一致）。
        // 我们内联了最关键的表；测试只会用到它们,
        // 因此其余的跳过即可。
        for stmt in TEST_SCHEMA {
            sqlx::query(stmt).execute(&pool).await.expect(stmt);
        }
        pool
    }

    const TEST_SCHEMA: &[&str] = &[
        "CREATE TABLE wallets (
            id TEXT PRIMARY KEY,
            address TEXT NOT NULL UNIQUE,
            label TEXT,
            chain_id INTEGER DEFAULT 137 NOT NULL,
            wallet_type TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            last_synced_at INTEGER
        )",
        "CREATE TABLE markets (
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
            user_interested INTEGER DEFAULT 0 NOT NULL,
            brief_dismissed_at INTEGER,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        )",
        "CREATE TABLE signals (
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
            active INTEGER DEFAULT 1 NOT NULL
        )",
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
            notes TEXT
        )",
        // v0.44 —— paper_fills 表。`bets` 的镜像,
        // 去掉 `tx_hash`（paper 模式不签名）。
        // 当 `ExecutorConfig.paper_mode = true` 时,
        // executor 会写入这里。Schema 是 bet
        // 字段的严格超集,用于审计,并显式
        // 带有 `mirror_id` 关联,方便 L1 展示
        // "该 fill 来自某条 copy_target 事件"。
        //
        // v0.45a —— 新增结算列:
        //   settled_at    — 我们对账 markets.resolved 的时间
        //   resolved_outcome — 'YES' | 'NO' | NULL
        //   won           — side 与 outcome 匹配为 1,
        //                   否则为 0,未结算时为 NULL
        //   pnl_usdc      — 结算后的 PnL（USDC;
        //                   未结算时为 NULL）
        // v0.45a 的对账器在 market 变为 resolved
        // 时更新这些字段。
        "CREATE TABLE IF NOT EXISTS paper_fills (
            id TEXT PRIMARY KEY,
            mirror_id TEXT NOT NULL,
            market_id TEXT NOT NULL,
            side TEXT NOT NULL,
            size TEXT NOT NULL,
            price REAL NOT NULL,
            placed_at INTEGER NOT NULL,
            notes TEXT,
            settled_at INTEGER,
            resolved_outcome TEXT,
            won INTEGER,
            pnl_usdc TEXT
        )",
        // v0.47a —— price_snapshots 表。每次
        // `sync_markets` 运行都记录每个 market 的
        // 当前 best-bid / best-ask（以及派生的
        // mid_price）。pre-v0.46 时,v0.46 回测引擎
        // 只能用退化的代理值（price=0.5）,因为
        // 我们没有历史价格快照。v0.47 让回测变真:
        // `list_resolved_markets_for_backtest` IPC
        // 现在能 join price_snapshots,以给出
        // market 解决前观察到的最新价格。
        //
        // 每个 (market_id, captured_at) 对应一行。
        // captured_at 是 unix 毫秒;为 "每个 market
        // 的最新快照" 查询建立索引。旧快照由
        // v0.47 retention 扫描清理（默认 30 天）。
        "CREATE TABLE IF NOT EXISTS price_snapshots (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            market_id TEXT NOT NULL,
            captured_at INTEGER NOT NULL,
            best_bid REAL NOT NULL,
            best_ask REAL NOT NULL,
            mid_price REAL NOT NULL,
            spread REAL NOT NULL
        )",
        "CREATE INDEX IF NOT EXISTS price_snapshots_market_recent_idx
         ON price_snapshots(market_id, captured_at DESC)",
        "CREATE TABLE copy_targets (
            id TEXT PRIMARY KEY,
            address TEXT NOT NULL UNIQUE,
            label TEXT,
            enabled INTEGER DEFAULT 1 NOT NULL,
            allocation_cap TEXT,
            min_edge REAL DEFAULT 0.05 NOT NULL,
            created_at INTEGER NOT NULL
        )",
        "CREATE TABLE copy_events (
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
        "CREATE TABLE audit_log (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            at INTEGER NOT NULL DEFAULT (unixepoch() * 1000),
            actor TEXT NOT NULL,
            action TEXT NOT NULL,
            target TEXT,
            payload TEXT,
            result TEXT
        )",
    ];

    #[tokio::test]
    async fn apply_seed_inserts_every_table() {
        let pool = empty_pool().await;
        let n = apply_seed(&pool, false).await.expect("seed");
        assert!(n >= 50, "expected at least 50 rows, got {n}");
        let wallet_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM wallets")
            .fetch_one(&pool).await.unwrap();
        assert!(wallet_count >= 3);
        let market_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM markets")
            .fetch_one(&pool).await.unwrap();
        assert!(market_count >= 10);
        let bet_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM bets")
            .fetch_one(&pool).await.unwrap();
        assert!(bet_count >= 15);
    }

    #[tokio::test]
    async fn apply_seed_is_idempotent() {
        let pool = empty_pool().await;
        let first = apply_seed(&pool, false).await.expect("first seed");
        let second = apply_seed(&pool, false).await.expect("second seed");
        // 第二次调用 force=false 应为 no-op。
        assert_eq!(second, 0, "second seed should be a no-op");
        assert!(first > 0, "first seed should insert rows");
        let bet_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM bets")
            .fetch_one(&pool).await.unwrap();
        // 行数必须保持不变。
        assert!(bet_count >= 15, "expected >=15 bets after idempotent re-seed, got {bet_count}");
    }

    #[tokio::test]
    async fn apply_seed_force_replaces_rows() {
        let pool = empty_pool().await;
        apply_seed(&pool, false).await.expect("first");
        // 手工清空一张表;force=true 应能重新插入。
        sqlx::query("DELETE FROM copy_events").execute(&pool).await.unwrap();
        let forced = apply_seed(&pool, true).await.expect("force seed");
        assert!(forced > 0);
        let event_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM copy_events")
            .fetch_one(&pool).await.unwrap();
        assert!(event_count >= 4);
    }

    #[tokio::test]
    async fn is_seeded_returns_false_on_empty_db() {
        let pool = empty_pool().await;
        let seeded = is_seeded(&pool).await.expect("check");
        assert!(!seeded);
    }

    #[tokio::test]
    async fn is_seeded_returns_true_after_seed() {
        let pool = empty_pool().await;
        apply_seed(&pool, false).await.expect("seed");
        let seeded = is_seeded(&pool).await.expect("check");
        assert!(seeded);
    }

    #[tokio::test]
    async fn seed_writes_audit_marker() {
        let pool = empty_pool().await;
        apply_seed(&pool, false).await.expect("seed");
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM audit_log WHERE action = ?",
        )
        .bind(SEED_AUDIT_ACTION)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(count, 1, "expected exactly 1 seed audit marker");
    }
}
