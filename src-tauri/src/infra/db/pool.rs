//! L4 — SQLite pool + PRAGMAs.
//!
//! Mirrors the Drizzle schema declared in `src/db/schema/index.ts`.
//! Path resolution goes through `crate::platform::paths` (see
//! `db_path`, `sqlite_url`) so the layout is owned in one place.

use crate::infra::error::AppResult;
use crate::platform::paths::{db_path, sqlite_url};
use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use tauri::{AppHandle, Manager};

/// Initialize SQLite pool in the app data dir.
///
/// Applies three PRAGMAs that match the Drizzle runtime expectations:
/// - `journal_mode = WAL`     — better concurrent reads during writes
/// - `synchronous = NORMAL`   — fsync once per commit, not per write
/// - `foreign_keys = ON`      — enforce FK constraints (off by default in SQLite)
///
/// Also creates the `_polyrocket_settings` side-table (see [`super::settings`]).
/// v0.53a — 构造 SQLite pool + 跑 PRAGMA + 调所有 `ensure_*_columns` / `ensure_*_table`。
///
/// **业务流程**：
///   1. `db_path(app)` 解析实际 DB 路径（用户可在 Settings 自定义）
///   2. `SqlitePoolOptions::new().max_connections(8).connect(url)`
///   3. 跑 `PRAGMA journal_mode=WAL`、`foreign_keys=ON`、`busy_timeout=5000`
///   4. 调所有 `ensure_*` 创建表 + 索引
///   5. 调 `apply_seed` 检查是否是首次启动
///
/// **调用方**：`lib.rs::run()` 在 `setup` hook 里调一次。结果塞进 `AppState.db`。
///
/// **错误**：DB 文件权限错 / 磁盘满 / schema 创建失败 → 返回 `AppError::Db`，
/// Tauri setup 阶段会 panic 阻止 app 启动（用户看到错误对话框）。
pub async fn init_pool(app: &AppHandle) -> AppResult<SqlitePool> {
    // v0.53a — resolve_db_path checks
    // `storage_path.json` first. If absent or invalid,
    // fall back to the OS default. The JSON file is
    // written by commands::storage::set_storage_path
    // AFTER the user picks a custom path; it takes
    // effect on the NEXT launch (we can't migrate
    // an already-open pool mid-flight).
    let db_path = crate::platform::paths::resolve_db_path(app)?;
    let db_url = sqlite_url(&db_path);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&db_url)
        .await?;

    sqlx::query("PRAGMA journal_mode = WAL").execute(&pool).await?;
    sqlx::query("PRAGMA synchronous = NORMAL").execute(&pool).await?;
    sqlx::query("PRAGMA foreign_keys = ON").execute(&pool).await?;

    super::settings::ensure_table(&pool).await?;
    // v0.119 — primary table migrations. Must run FIRST so that
    // is_seeded() can SELECT COUNT(*) on the primary tables below.
    super::migrations::ensure_primary_tables(&pool).await?;
    ensure_copy_mirror_queue(&pool).await?;
    ensure_price_snapshots(&pool).await?;
    // v0.78 — bankroll allocation tables (M11)
    super::bankroll::ensure_tables(&pool).await?;
    // v0.51a — clob_snapshots table (real order
    // book per market per timestamp). Idempotent.
    ensure_clob_snapshots(&pool).await?;
    // v0.119 — paper_fills TABLE itself (was only created
    // via first-run seed before; pre-v0.45 databases had no
    // table and crashed on boot). Must run BEFORE the
    // column-ensure step below, otherwise the column check
    // sees zero columns (PRAGMA returns empty) and would
    // skip the ALTER TABLE calls. Idempotent.
    super::paper_fills::ensure_paper_fills_table(&pool).await?;
    // v0.45a — paper_fills settlement columns. Idempotent:
    // ALTER TABLE ADD COLUMN is a no-op if the column
    // already exists when wrapped in the IF NOT EXISTS
    // guard, BUT sqlite doesn't support IF NOT EXISTS on
    // ADD COLUMN. We use the `PRAGMA table_info` check
    // pattern instead.
    super::paper_fills::ensure_paper_fills_columns(&pool).await?;
    // v0.50a — bets order-type columns (order_type,
    // limit_price, stop_price, post_only). Same
    // idempotent pattern.
    super::bets_columns::ensure_bets_columns(&pool).await?;

    // v0.8a — first-run demo data seeder.
    // Idempotent: if the DB is already populated (e.g. user has used
    // the app before), this is a no-op. Otherwise it inserts the
    // canonical demo bundle so the UI shows a populated dashboard
    // out of the box.
    if !super::seed::is_seeded(&pool).await? {
        let inserted = super::seed::apply_seed(&pool, false).await?;
        tracing::info!(
            rows = inserted,
            "first-run seeder populated demo data"
        );
    }

    Ok(pool)
}

/// Migration helper for the mirror queue (v0.6a M5 auto-execution).
/// Called by `init_pool` so first launch after upgrade creates the table.
/// v0.20a — 创建 `copy_mirror_queue` 表（copy-trading mirror 队列）。**幂等**。
///
/// **表用途**：每条 row 代表一次 mirror executor 要处理的 mirror 事件。
/// v0.20a 起 mirror 不再走内存队列 —— 写 DB 后由 `run_mirror_executor_loop` 消费，
/// 保证重启后不丢。
pub async fn ensure_copy_mirror_queue(pool: &SqlitePool) -> sqlx::Result<()> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS copy_mirror_queue (
            id TEXT PRIMARY KEY,
            event_id INTEGER NOT NULL,
            target_id TEXT NOT NULL,
            market_id TEXT NOT NULL,
            side TEXT NOT NULL,
            size TEXT NOT NULL,
            flipped INTEGER NOT NULL DEFAULT 0,
            status TEXT NOT NULL DEFAULT 'pending',
            created_at INTEGER NOT NULL,
            submitted_at INTEGER,
            filled_at INTEGER,
            bet_id TEXT,
            reject_reason TEXT
        )",
    )
    .execute(pool)
    .await?;
    sqlx::query(
        "CREATE INDEX IF NOT EXISTS mirror_queue_status_idx
         ON copy_mirror_queue(status, created_at)",
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// v0.47a — ensure the price_snapshots table +
/// its market/recent index exist. Idempotent.
/// v0.47a — 创建 `price_snapshots` 表（每个 market 每次 sweep 一行）。**幂等**。
///
/// **表用途**：存最近一次 sweep 的 (best_bid, best_ask, mid_price, spread)，
/// 给 L1 Dashboard + backtest join 用。**不**存 order book depth（那是
/// `clob_snapshots` 的活）。
pub async fn ensure_price_snapshots(pool: &SqlitePool) -> sqlx::Result<()> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS price_snapshots (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            market_id TEXT NOT NULL,
            captured_at INTEGER NOT NULL,
            best_bid REAL NOT NULL,
            best_ask REAL NOT NULL,
            mid_price REAL NOT NULL,
            spread REAL NOT NULL
        )",
    )
    .execute(pool)
    .await?;
    sqlx::query(
        "CREATE INDEX IF NOT EXISTS price_snapshots_market_recent_idx
         ON price_snapshots(market_id, captured_at DESC)",
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// v0.51a — ensure the `clob_snapshots` table +
/// its market/recent index exist. Idempotent.
///
/// `clob_snapshots` is the FULL order-book record
/// (one row per price level per side per timestamp),
/// as opposed to `price_snapshots` (one row per
/// market per timestamp with a single bid/ask pair).
/// v0.51a — 创建 `clob_snapshots` 表（每个 market 每次 snapshot 多行）。**幂等**。
///
/// **表用途**：存 full order book ladder。v0.51a+ 给需要 depth 推理的功能用
///（post-only 检查、partial fill 模拟、book 演变可视化）。
pub async fn ensure_clob_snapshots(pool: &SqlitePool) -> sqlx::Result<()> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS clob_snapshots (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            market_id TEXT NOT NULL,
            captured_at INTEGER NOT NULL,
            side TEXT NOT NULL,
            price REAL NOT NULL,
            size REAL NOT NULL
        )",
    )
    .execute(pool)
    .await?;
    sqlx::query(
        "CREATE INDEX IF NOT EXISTS clob_snapshots_market_recent_idx
         ON clob_snapshots(market_id, captured_at DESC)",
    )
    .execute(pool)
    .await?;
    Ok(())
}
