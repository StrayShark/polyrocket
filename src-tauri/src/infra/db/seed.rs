//! L4 — Database seeder (idempotent).
//!
//! Reads `domain::seed::SeedBundle::demo()` and applies each row to the
//! SQLite pool. Idempotent: each row uses a fixed primary key, so calling
//! `apply_seed` twice produces the same DB state (INSERT OR REPLACE).
//!
//! Trigger: called from `init_pool` on first launch (auto), and from
//! `commands::seed::seed_demo_data(force=true)` for explicit re-seed.

use crate::domain::seed::{SeedBundle, SEED_AUDIT_ACTION, SEED_NOW_MS};
use crate::infra::error::AppResult;
use sqlx::SqlitePool;

/// Returns true if the DB already has demo data (>=1 wallet, >=1 market,
/// >=1 bet). Used to skip the auto-seed on subsequent boots.
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

/// Apply the canonical demo bundle to the DB. Returns the number of rows
/// inserted/replaced across all tables.
///
/// `force=true` always re-applies (useful for the dev "reset demo data"
/// button); `force=false` is a no-op if the DB is already seeded.
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
        .bind(&m.volume_24h)
        .bind(m.created_at)
        .bind(m.updated_at)
        .execute(pool)
        .await?;
        total += 1;
    }

    // 3. Signals
    // We need to recover the auto-increment IDs since bets reference them.
    // Strategy: wipe and re-insert signals so the IDs are deterministic
    // (they don't appear in any bet.signal_id in the demo).
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

    // 7. Audit marker so the seeder is observable in the Audit page.
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
        // Apply the full schema (the same one Drizzle would create).
        // We inline the most important tables; tests will only touch
        // these, so it's fine to skip the rest.
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
            volume_24h TEXT,
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
        // v0.44 — paper_fills table. Mirror of `bets` minus
        // the `tx_hash` (paper doesn't sign anything). The
        // executor writes here when
        // `ExecutorConfig.paper_mode = true`. The schema is
        // a strict superset of the bet fields used for
        // audit, with an explicit `mirror_id` link so the
        // L1 can show "this fill would have come from this
        // copy_target event".
        //
        // v0.45a — added settlement columns:
        //   settled_at    — when we reconciled against
        //                   markets.resolved
        //   resolved_outcome — 'YES' | 'NO' | NULL
        //   won           — 1 if side matched outcome, 0
        //                   otherwise, NULL if not settled
        //   pnl_usdc      — settled PnL in USDC (NULL if
        //                   not settled)
        // The reconciler (v0.45a) updates these fields when
        // a market becomes resolved.
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
        // v0.47a — price_snapshots table. Records the
        // current best-bid / best-ask (and derived
        // mid_price) for each market every time
        // `sync_markets` runs. Pre-v0.46, the v0.46
        // backtest engine had to fall back to a
        // degenerate proxy (price=0.5) because we
        // had no historical price snapshots. v0.47
        // makes the backtest real: the
        // `list_resolved_markets_for_backtest` IPC
        // now joins price_snapshots to surface the
        // most recent price observed before the
        // market's resolution.
        //
        // One row per (market_id, captured_at).
        // captured_at is unix-ms; we index it for
        // the "most recent snapshot per market"
        // query. Old snapshots are pruned by the
        // v0.47 retention sweep (default 30 days).
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
            at INTEGER NOT NULL,
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
        // Second call with force=false should be a no-op.
        assert_eq!(second, 0, "second seed should be a no-op");
        assert!(first > 0, "first seed should insert rows");
        let bet_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM bets")
            .fetch_one(&pool).await.unwrap();
        // Row count must be unchanged.
        assert!(bet_count >= 15, "expected >=15 bets after idempotent re-seed, got {bet_count}");
    }

    #[tokio::test]
    async fn apply_seed_force_replaces_rows() {
        let pool = empty_pool().await;
        apply_seed(&pool, false).await.expect("first");
        // Wipe one table by hand; force=true should re-insert.
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
