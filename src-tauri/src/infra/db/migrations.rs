//! L4 — Primary table schema migrations.
//!
//! v0.119 — moved primary tables (wallets, markets, signals, bets)
//! out of `seed.rs::apply_seed()` into a dedicated migration that
//! runs on EVERY boot. Previously these tables were only created
//! during first-run seed, which is skipped if any of these tables
//! already exists. This caused a chicken-and-egg crash on databases
//! that previously had been seeded but had their tables corrupted
//! or wiped (e.g. WAL truncation, manual fs surgery).
//!
//! Idempotent — every statement is `CREATE TABLE IF NOT EXISTS`.
//! Safe to run on every boot.

use sqlx::SqlitePool;

/// v0.119 — run all primary table migrations in dependency order.
///
/// Order matters: tables referenced by foreign keys must be created
/// first. We intentionally don't declare FKs in the schema (sqlite
/// FK enforcement is OFF by default; would surprise users) but
/// logically:
///   1. wallets  — independent
///   2. markets  — independent
///   3. signals  — references markets
///   4. bets     — references wallets + markets + signals
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
    // v0.119 — added `yes_price REAL`, `no_price REAL`, `closes_at INTEGER`,
    // `status TEXT`, `resolution_source TEXT`. The `llm_analyze` IPC's
    // `build_market_context` (commands/llm.rs) SELECTs all of these — without
    // them, the very first analyze call crashed with `no such column:
    // resolution_source` (and the others). Pre-v0.119 the SELECT silently
    // broke and llm_analyze had never succeeded against a freshly-seeded DB.
    //
    // All new columns are nullable or have DEFAULTs so existing rows from
    // seed.rs / sync_markets keep working. Production sync_markets still
    // needs to populate yes/no_price (currently only orderbook_snapshots
    // has them); for now the build_market_context falls back to NULL when
    // the columns are empty and the LLM still gets a valid (if slightly
    // sparse) market context.
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
    // v0.119 — added `kind TEXT`, `summary TEXT`, `polarity TEXT DEFAULT 'neutral'`.
    // The `llm_analyze` IPC's `build_market_context` (commands/llm.rs) selects
    // these 3 columns to populate `SignalSummary { name, value, polarity }`
    // (prompts.rs). Pre-v0.119 this crashed with `no such column: s.kind`
    // on every analyze call. All 3 are TEXT/nullable so existing seed
    // rows keep working.
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

    // 5) copy_targets (matches seed.rs original schema — must match
    // `commands::copy::add_copy_target` IPC's INSERT statement)
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

    // 6) copy_events (matches seed.rs original schema)
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

    // 7) audit_log (matches seed.rs original schema — must match
    // INSERT statements in commands/{audit,llm,brief,bet,storage_migrate}.rs
    // and infra::scheduler/mod.rs)
    //
    // v0.119 — `at` now has a DEFAULT so INSERT statements that omit it
    // (most of them in commands/) still succeed. Pre-fix, every audit_log
    // writer had to remember `.bind(now_ms)` or it crashed with
    // `NOT NULL constraint failed: audit_log.at`. The DEFAULT uses
    // SQLite 3.38+'s `unixepoch()` * 1000 for unix epoch milliseconds.
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

    // 8) daily_briefs (was missing in fresh-init flow)
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
    // v0.119 — added `signal_id` + `triggered_by` columns. `llm_analyze` IPC
    // (commands/llm.rs:23-31) writes both — without them the very first
    // analyze call after a fresh init would crash with `no column named signal_id`.
    //
    // Also added DEFAULTs to `prompt_template` and `market_snapshot`. The IPC's
    // pending-row INSERT (commands/llm.rs:1130 area) sets neither, but they are
    // NOT NULL — pre-v0.119 this crashed on every analyze call. With DEFAULTs,
    // the row lands as 'pending' with empty template + snapshot; a later UPDATE
    // can fill them in (current IPC doesn't, but the row is at least valid).
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

    // 10) llm_providers (referenced by llm_provider_keys + llm_health_checks).
    // Schema must match INSERT statements in commands::llm::upsert_llm_provider
    // and commands::llm_mgmt. Includes all fields from the wider
    // llm_mgmt UPSERT path so a fresh DB can host the full provider config.
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

    // 11) llm_provider_keys (referenced by llm_health_checks + keyring)
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

    // 12) llm_health_checks (background monitor + scheduler reads)
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

    // 13) model_performance (used by pnl.rs brier_score query)
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

    // 14) llm_call_logs (per-call log: latency, cost, success — feeds
    // scheduler cost-anomaly + LLM perf dashboards)
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

    // 15) llm_decisions (audit trail: did user follow the LLM recommendation?
    // feeds the "was the LLM right?" scoring in commands/llm.rs)
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

    // 16) user_brief_prefs (per-user daily-brief preferences)
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

    // 17) orderbook_snapshots (full bid/ask depth — feeds LLM market
    // context. Distinct from price_snapshots which only stores
    // best bid/ask mid.)
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

    // v0.119 — additive column migrations for already-seeded databases.
    // `CREATE TABLE IF NOT EXISTS` is a no-op when the table exists with
    // a different schema, so we also need explicit `ALTER TABLE ADD COLUMN`
    // for fresh columns added in v0.119+. SQLite throws "duplicate column"
    // if we try to add a column that already exists, so each ALTER is wrapped
    // in a sqlite-level try (using a CASE that swallows the duplicate-column
    // error via the `changes()` function returning 0 isn't standard, so we
    // accept the error via a subquery guard).
    //
    // audit_log: was created pre-v0.119 without `at` DEFAULT. We add it
    //   defensively (no-op if already defaulted).
    // llm_analyses: was created pre-v0.119 without `signal_id` /
    //   `triggered_by`. Both are nullable so safe to add to existing rows.
    let _ = sqlx::query("ALTER TABLE llm_analyses ADD COLUMN signal_id INTEGER")
        .execute(pool).await;  // ignore "duplicate column" error
    let _ = sqlx::query("ALTER TABLE llm_analyses ADD COLUMN triggered_by TEXT NOT NULL DEFAULT ''")
        .execute(pool).await;

    // markets: v0.119 added yes_price / no_price / closes_at / status /
    // resolution_source so llm_analyze can SELECT them. Add defensively
    // for already-seeded databases (the in-process CREATE TABLE IF NOT
    // EXISTS above is a no-op when markets already exists).
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

    // signals: v0.119 added kind / summary / polarity so llm_analyze can
    // SELECT them in build_market_context. Add defensively for
    // already-seeded databases.
    let _ = sqlx::query("ALTER TABLE signals ADD COLUMN kind TEXT")
        .execute(pool).await;
    let _ = sqlx::query("ALTER TABLE signals ADD COLUMN summary TEXT")
        .execute(pool).await;
    let _ = sqlx::query("ALTER TABLE signals ADD COLUMN polarity TEXT DEFAULT 'neutral'")
        .execute(pool).await;

    // llm_recommendations: v0.119 added `reasoning` column. `llm_analyze`
    // writes per-recommendation reasoning text (commands/llm.rs:715-735).
    // Pre-v0.119 this crashed with `no such column: reasoning` on every
    // analyze call. Nullable so existing rows keep working.
    let _ = sqlx::query("ALTER TABLE llm_recommendations ADD COLUMN reasoning TEXT")
        .execute(pool).await;
    let _ = sqlx::query("ALTER TABLE llm_recommendations ADD COLUMN parse_ok INTEGER NOT NULL DEFAULT 1")
        .execute(pool).await;
    let _ = sqlx::query("ALTER TABLE llm_recommendations ADD COLUMN parse_error TEXT")
        .execute(pool).await;

    Ok(())
}
