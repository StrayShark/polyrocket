//! L4 —— SQLite 连接池与 PRAGMA。
//!
//! 与 `src/db/schema/index.ts` 中声明的 Drizzle schema 保持一致。
//! 路径解析通过 `crate::platform::paths`（参见 `db_path`、
//! `sqlite_url`），布局由单一来源管理。

use crate::infra::error::AppResult;
use crate::platform::paths::{db_path, sqlite_url};
use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use tauri::{AppHandle, Manager};

/// 在 app data 目录初始化 SQLite 连接池。
///
/// 应用与 Drizzle 运行时一致的三条 PRAGMA：
/// - `journal_mode = WAL`     —— 写入期间并发读更友好
/// - `synchronous = NORMAL`   —— 每次 commit 一次 fsync，而非每次写
/// - `foreign_keys = ON`      —— 启用外键约束（SQLite 默认关闭）
///
/// 同时创建 `_polyrocket_settings` 副表（参见 [`super::settings`]）。
/// v0.53a —— 构造 SQLite pool + 跑 PRAGMA + 调所有 `ensure_*_columns` / `ensure_*_table`。
///
/// **业务流程**:
///   1. `db_path(app)` 解析实际 DB 路径(用户可在 Settings 自定义)
///   2. `SqlitePoolOptions::new().max_connections(8).connect(url)`
///   3. 跑 `PRAGMA journal_mode=WAL`、`foreign_keys=ON`、`busy_timeout=5000`
///   4. 调所有 `ensure_*` 创建表 + 索引
///   5. 调 `apply_seed` 检查是否是首次启动
///
/// **调用方**:`lib.rs::run()` 在 `setup` hook 里调一次。结果塞进 `AppState.db`。
///
/// **错误**:DB 文件权限错 / 磁盘满 / schema 创建失败 → 返回 `AppError::Db`,
/// Tauri setup 阶段会 panic 阻止 app 启动(用户看到错误对话框)。
pub async fn init_pool(app: &AppHandle) -> AppResult<SqlitePool> {
    // v0.53a —— resolve_db_path 检查
    // 优先读 `storage_path.json`。若缺失或无效,
    // 回退到操作系统默认路径。JSON 文件由
    // commands::storage::set_storage_path 在用户
    // 选定自定义路径后写入;它在**下一次**启动时
    // 生效(无法在已经打开的 pool 运行中迁移)。
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
    // v0.119 —— 主表迁移。必须先于下面所有 `is_seeded()`
    // 的 SELECT COUNT(*) 之前跑。
    super::migrations::ensure_primary_tables(&pool).await?;
    ensure_copy_mirror_queue(&pool).await?;
    ensure_price_snapshots(&pool).await?;
    // v0.78 —— 资金分配表(M11)
    super::bankroll::ensure_tables(&pool).await?;
    // v0.51a —— clob_snapshots 表(每个 market 每个
    // 时间戳的真实订单簿)。幂等。
    ensure_clob_snapshots(&pool).await?;
    // v0.119 —— paper_fills 表本身(此前仅在首次
    // 启动的 seed 中创建;pre-v0.45 数据库没有该表,
    // 启动时崩溃)。必须先于下面的列-ensure 步骤跑,
    // 否则列检查会看到 0 个列(PRAGMA 返回空),
    // 跳过 ALTER TABLE 调用。幂等。
    super::paper_fills::ensure_paper_fills_table(&pool).await?;
    // v0.45a —— paper_fills 结算列。幂等:
    // ALTER TABLE ADD COLUMN 在用 IF NOT EXISTS 守卫
    // 包裹时,若列已存在则为 no-op,但 SQLite
    // 不支持 ADD COLUMN 上的 IF NOT EXISTS。
    // 我们改用 `PRAGMA table_info` 检查模式。
    super::paper_fills::ensure_paper_fills_columns(&pool).await?;
    // v0.50a —— bets 订单类型列(order_type、
    // limit_price、stop_price、post_only)。
    // 同样的幂等模式。
    super::bets_columns::ensure_bets_columns(&pool).await?;

    // v0.8a —— 首次运行 demo 数据填充器。
    // 幂等:若 DB 已被填充(例如用户此前用过 app),
    // 本调用是 no-op。否则会插入标准的 demo 数据集,
    // 使 UI 开箱即有数据展示。
    if !super::seed::is_seeded(&pool).await? {
        let inserted = super::seed::apply_seed(&pool, false).await?;
        tracing::info!(
            rows = inserted,
            "first-run seeder populated demo data"
        );
    }

    Ok(pool)
}

/// v0.6a M5 —— mirror 队列的迁移辅助。
/// 由 `init_pool` 调用,使得升级后首次启动能创建该表。
/// v0.20a —— 创建 `copy_mirror_queue` 表（copy-trading mirror 队列）。**幂等**。
///
/// **表用途**:每条 row 代表一次 mirror executor 要处理的 mirror 事件。
/// v0.20a 起 mirror 不再走内存队列 —— 写 DB 后由 `run_mirror_executor_loop` 消费,
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

/// v0.47a —— 确保 `price_snapshots` 表 + 它的
/// market/recent 索引存在。幂等。
/// v0.47a —— 创建 `price_snapshots` 表（每个 market 每次 sweep 一行）。**幂等**。
///
/// **表用途**:存最近一次 sweep 的 (best_bid, best_ask, mid_price, spread),
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

/// v0.51a —— 确保 `clob_snapshots` 表 + 它的
/// market/recent 索引存在。幂等。
///
/// `clob_snapshots` 是完整的订单簿记录(同一时间戳下
/// 每个价格档位、每个方向对应一行),与 `price_snapshots`
/// (每个 market 每个时间戳一行,只含一对 bid/ask)不同。
/// v0.51a —— 创建 `clob_snapshots` 表（每个 market 每次 snapshot 多行）。**幂等**。
///
/// **表用途**:存 full order book ladder。v0.51a+ 给需要 depth 推理的功能用
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
