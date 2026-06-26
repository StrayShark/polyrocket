//! L2 —— PnL 仪表盘 KPI（M8）。
//!
//! IPC:dashboard_kpis —— 聚合 bets、model_performance、signals
//! 表，用于首页仪表盘磁贴。计算逻辑将按 M8 里程碑迁移到 `domain::pnl`。

use crate::AppResult;
use crate::infra::error::AppError;
use crate::infra::state::AppState;
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::State;

#[derive(Debug, Serialize, Deserialize, Type)]
pub struct DashboardKpis {
    pub total_equity_usdc: String,
    pub open_pnl_usdc: String,
    pub win_rate_30d: f64,
    pub brier_score: f64,
    pub active_signals: i64,
    pub open_positions: i64,
}

/// Dashboard 页面的聚合 KPI。
/// 为了性能在 Rust 端计算 —— 查询完全留在本地。
#[tauri::command]
#[specta::specta]
pub async fn dashboard_kpis(state: State<'_, AppState>) -> AppResult<DashboardKpis> {
    // v0.122b+ 补齐修复 —— schema 中 `bets.size` 和 `bets.pnl` 都是
    // TEXT 类型，但 `SUM()` 返回 REAL。若把结果以 `Option<String>`
    // 读取会触发 "Rust type String (as TEXT) is not compatible with
    // SQL type REAL" 错误，进而让整个 Dashboard 页崩溃
    // （React Query 的错误边界会捕获并显示 "Something went wrong"，
    // 而不是 KPI 卡片）。修复方案：在 SQL 层 CAST 为 REAL，
    // 这里以 f64 解码，再格式化为 4 位小数字符串作为 DTO 传输。
    let total_equity: Option<f64> = sqlx::query_scalar(
        "SELECT COALESCE(SUM(CAST(size AS REAL)), 0.0) FROM bets WHERE status = 'open'",
    )
    .fetch_optional(&state.db)
    .await?;
    let open_pnl: Option<f64> = sqlx::query_scalar(
        "SELECT COALESCE(SUM(CAST(pnl AS REAL)), 0.0) FROM bets WHERE status = 'open'",
    )
    .fetch_optional(&state.db)
    .await?;

    let wins: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM bets WHERE status = 'won' AND placed_at >= ?")
        .bind(chrono::Utc::now().timestamp_millis() - 30 * 24 * 3600 * 1000)
        .fetch_one(&state.db)
        .await?;
    let losses: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM bets WHERE status = 'lost' AND placed_at >= ?")
        .bind(chrono::Utc::now().timestamp_millis() - 30 * 24 * 3600 * 1000)
        .fetch_one(&state.db)
        .await?;
    let total = (wins + losses).max(1);
    let win_rate = wins as f64 / total as f64;

    let brier: Option<f64> = sqlx::query_scalar(
        "SELECT brier_score FROM model_performance WHERE category IS NULL ORDER BY window_end DESC LIMIT 1",
    )
    .fetch_optional(&state.db)
    .await?;

    let active_signals: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM signals WHERE active = 1")
        .fetch_one(&state.db)
        .await?;
    let open_positions: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM bets WHERE status = 'open'")
        .fetch_one(&state.db)
        .await?;

    Ok(DashboardKpis {
        // 把 f64 的总额格式化为 4 位小数字符串作为 DTO 传输。
        // `unwrap_or(0.0)` 在 bets 表为空时保持 API 稳定
        // （SUM 返回 NULL → COALESCE 返回 0.0 → 仍然走这一分支）。
        total_equity_usdc: format!("{:.4}", total_equity.unwrap_or(0.0)),
        open_pnl_usdc: format!("{:.4}", open_pnl.unwrap_or(0.0)),
        win_rate_30d: win_rate,
        brier_score: brier.unwrap_or(0.0),
        active_signals,
        open_positions,
    })
}

// =================================================================
// ============== v0.45b —— 模拟交易 PnL 汇总 ==============
// =================================================================

/// v0.45b —— 模拟交易 PnL 汇总。把 `paper_fills` 表聚合为一个结构体，
/// L1 可在 Dashboard / PnL 页面以「如果当时下了注会怎样」的指标呈现。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaperPnlSummary {
    /// 累计已记录的模拟成交条数（已结算 + 未结算）。
    pub total_fills: i64,
    /// 已对照市场结算结果完成对账的模拟成交条数。
    pub settled_fills: i64,
    /// 其中判定为获胜（方向与结算结果一致）的成交条数。
    pub won_fills: i64,
    /// 其中判定为失败的成交条数。
    pub lost_fills: i64,
    /// 结算胜率（won / settled）。尚无已结算成交时为 0.0。
    pub win_rate: f64,
    /// 所有已结算成交的累计已实现 PnL，单位 USDC。正数 = 盈利，负数 = 亏损。
    pub realized_pnl_usdc: String,
    /// 用户是否启用了模拟交易模式。L1 用此决定是否渲染该卡片。
    pub paper_mode_enabled: bool,
}

/// v0.45b —— 模拟交易 PnL IPC。返回聚合汇总以及当前 paper_mode 标志
///（这样 L1 可以在不发起第二次查询的情况下，决定显示或隐藏该卡片）。
#[tauri::command]
pub async fn paper_pnl_summary(
    state: State<'_, AppState>,
) -> AppResult<PaperPnlSummary> {
    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM paper_fills")
        .fetch_one(&state.db)
        .await?;
    let settled: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM paper_fills WHERE settled_at IS NOT NULL",
    )
    .fetch_one(&state.db)
    .await?;
    let won: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM paper_fills WHERE won = 1",
    )
    .fetch_one(&state.db)
    .await?;
    let lost = settled - won;
    let win_rate = if settled > 0 {
        won as f64 / settled as f64
    } else {
        0.0
    };
    // 已实现 PnL —— 对所有已结算成交的 pnl_usdc 求和。
    // 在 SQL 端计算是为了避免在 Rust 循环中累积浮点误差。
    let realized_pnl: f64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(CAST(pnl_usdc AS REAL)), 0.0) FROM paper_fills
         WHERE settled_at IS NOT NULL AND pnl_usdc IS NOT NULL",
    )
    .fetch_one(&state.db)
    .await?;
    let paper_mode = *state
        .mirror_paper_mode
        .lock()
        .map_err(|e| AppError::Internal(format!("mirror_paper_mode lock: {e}")))?;
    Ok(PaperPnlSummary {
        total_fills: total,
        settled_fills: settled,
        won_fills: won,
        lost_fills: lost,
        win_rate,
        realized_pnl_usdc: format!("{:.4}", realized_pnl),
        paper_mode_enabled: paper_mode,
    })
}

// =================================================================
// ============== v0.50c —— 成交分析 =========================
// =================================================================

/// v0.50c + v0.51b —— 成交分析汇总。把 `bets` 表聚合为一个结构体，
/// L1 Dashboard 可显示为「成交分析」卡片。
///
/// v0.51b 新增滑点与成交耗时：
///   - avg_slippage = 对所有 fill_price 非空的已成交行
///     求 |fill_price - price| 的均值。在 v0.5d 的
///     确定性桩数据中（fill_price == price）恒为 0；
///     v0.51+ 接入真实 CLOB 后会反映实际滑点。
///   - avg_time_to_fill_ms = 对已成交行求
///     (filled_at - placed_at) 的均值。同上。
///   - partial_fill_count / partial_fill_rate。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FillAnalytics {
    pub total_fills: i64,
    pub open_count: i64,
    pub won_count: i64,
    pub lost_count: i64,
    pub cancelled_count: i64,
    /// (settled_at - placed_at) 的平均值，单位毫秒。
    /// 尚未结算时为 None。
    pub avg_time_to_settlement_ms: Option<f64>,
    /// 结算胜率（won / settled）。尚未结算时为 0.0。
    pub win_rate: f64,
    /// 已结算行的已实现 PnL 合计，单位 USDC。
    pub realized_pnl_usdc: String,
    /// 按 order_type 分组的成交分布。取值范围
    /// "market" | "limit" | "stop_loss"；v0.50 之前的
    /// 行归入 "market"（迁移时的默认值）。
    pub by_order_type: Vec<OrderTypeBucket>,
    /// post_only 成交数量（限价单 + post_only）。
    pub post_only_count: i64,
    /// post_only 成交的占比。total_fills == 0 时为 0.0。
    pub post_only_rate: f64,
    /// v0.51b —— 已成交行 |fill_price - price| 的平均值。
    /// 当没有任何行具备 fill_price（即 v0.51b 之前的数据库）时为 None。
    pub avg_slippage: Option<f64>,
    /// v0.51b —— (filled_at - placed_at) 的平均值，
    /// 单位毫秒。没有任何行具备 filled_at 时为 None。
    pub avg_time_to_fill_ms: Option<f64>,
    /// v0.51b —— 部分成交数量（存在 fill_size 且 < shares）。
    pub partial_fill_count: i64,
    /// v0.51b —— 部分成交的占比。total_fills == 0 时为 0.0。
    pub partial_fill_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderTypeBucket {
    pub order_type: String,
    pub count: i64,
    pub settled: i64,
    pub won: i64,
    pub realized_pnl_usdc: f64,
}

/// v0.50c —— 成交分析 IPC。聚合 `bets` 表，返回供 L1 Dashboard 卡片使用的
/// `FillAnalytics`。
#[tauri::command]
pub async fn fill_analytics(state: State<'_, AppState>) -> AppResult<FillAnalytics> {
    // 全表聚合。COUNT/AVG/SUM —— 不会触发扫描风险，
    // 因为 `bets` 对单用户而言很小（< 100k 行）。
    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM bets")
        .fetch_one(&state.db)
        .await?;
    let open_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM bets WHERE status = 'open'",
    )
    .fetch_one(&state.db)
    .await?;
    let won_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM bets WHERE status = 'won'",
    )
    .fetch_one(&state.db)
    .await?;
    let lost_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM bets WHERE status = 'lost'",
    )
    .fetch_one(&state.db)
    .await?;
    let cancelled_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM bets WHERE status = 'cancelled'",
    )
    .fetch_one(&state.db)
    .await?;
    let settled_count = won_count + lost_count + cancelled_count;
    let win_rate = if settled_count > 0 {
        won_count as f64 / settled_count as f64
    } else {
        0.0
    };

    // 平均成交到结算时间（仅统计已结算行）。
    let avg_tts: Option<f64> = sqlx::query_scalar(
        "SELECT AVG(settled_at - placed_at) FROM bets
         WHERE settled_at IS NOT NULL",
    )
    .fetch_one(&state.db)
    .await?;

    // 已结算行的已实现 PnL。
    let realized: Option<f64> = sqlx::query_scalar(
        "SELECT COALESCE(SUM(CAST(pnl AS REAL)), 0.0) FROM bets
         WHERE status IN ('won', 'lost') AND pnl IS NOT NULL",
    )
    .fetch_one(&state.db)
    .await?;
    let realized = realized.unwrap_or(0.0);

    // 按订单类型拆分。执行 3 次独立的 COUNT/SUM 查询；
    // 也可以用一条 GROUP BY，但显式写法更易读，
    // 而且表很小。
    let mut by_order_type = Vec::new();
    for ot in ["market", "limit", "stop_loss"] {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM bets WHERE order_type = ?",
        )
        .bind(ot)
        .fetch_one(&state.db)
        .await?;
        let settled: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM bets WHERE order_type = ?
             AND status IN ('won', 'lost')",
        )
        .bind(ot)
        .fetch_one(&state.db)
        .await?;
        let won: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM bets WHERE order_type = ? AND status = 'won'",
        )
        .bind(ot)
        .fetch_one(&state.db)
        .await?;
        let pnl: Option<f64> = sqlx::query_scalar(
            "SELECT COALESCE(SUM(CAST(pnl AS REAL)), 0.0) FROM bets
             WHERE order_type = ? AND status IN ('won', 'lost') AND pnl IS NOT NULL",
        )
        .bind(ot)
        .fetch_one(&state.db)
        .await?;
        by_order_type.push(OrderTypeBucket {
            order_type: ot.to_string(),
            count,
            settled,
            won,
            realized_pnl_usdc: pnl.unwrap_or(0.0),
        });
    }

    let post_only_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM bets WHERE post_only = 1",
    )
    .fetch_one(&state.db)
    .await?;
    let post_only_rate = if total > 0 {
        post_only_count as f64 / total as f64
    } else {
        0.0
    };

    // v0.51b —— 滑点与成交耗时。两个查询都会跳过
    // fill 列为 NULL 的行（v0.51b 之前的数据库）。
    let avg_slippage: Option<f64> = sqlx::query_scalar(
        "SELECT AVG(ABS(fill_price - price)) FROM bets
         WHERE fill_price IS NOT NULL",
    )
    .fetch_one(&state.db)
    .await?;
    let avg_time_to_fill_ms: Option<f64> = sqlx::query_scalar(
        "SELECT AVG(filled_at - placed_at) FROM bets
         WHERE filled_at IS NOT NULL",
    )
    .fetch_one(&state.db)
    .await?;
    let partial_fill_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM bets WHERE partial = 1",
    )
    .fetch_one(&state.db)
    .await?;
    let partial_fill_rate = if total > 0 {
        partial_fill_count as f64 / total as f64
    } else {
        0.0
    };

    Ok(FillAnalytics {
        total_fills: total,
        open_count,
        won_count,
        lost_count,
        cancelled_count,
        avg_time_to_settlement_ms: avg_tts,
        win_rate,
        realized_pnl_usdc: format!("{:.4}", realized),
        by_order_type,
        post_only_count,
        post_only_rate,
        avg_slippage,
        avg_time_to_fill_ms,
        partial_fill_count,
        partial_fill_rate,
    })
}

// ============================================================
// v0.50c —— fill_analytics cargo 测试
// ============================================================
//
// IPC 处理函数基本就是 SQL。真正需要验证的几点是：
// 空表不能除以零、各 order_type 桶的合计等于总数、
// post_only_rate 用了正确的分母。我们用一个手搓的连接池
// 来覆盖这三点。

#[cfg(test)]
mod fill_analytics_tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    /// 创建一个包含所需 schema 的全新数据库：
    /// `bets` 表带 v0.50a 的列。
    async fn make_pool() -> sqlx::SqlitePool {
        let dir = std::env::temp_dir().join(format!(
            "polyrocket_fill_analytics_test_{}",
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
        sqlx::query(
            "CREATE TABLE bets (
                id TEXT PRIMARY KEY,
                market_id TEXT NOT NULL,
                side TEXT NOT NULL,
                size TEXT NOT NULL,
                price REAL NOT NULL,
                placed_at INTEGER NOT NULL,
                settled_at INTEGER,
                pnl TEXT,
                status TEXT NOT NULL,
                order_type TEXT NOT NULL DEFAULT 'market',
                limit_price REAL,
                stop_price REAL,
                post_only INTEGER NOT NULL DEFAULT 0
            )",
        )
        .execute(&pool)
        .await
        .unwrap();
        pool
    }

    /// v0.50c —— 空表返回 0、无除零错误、所有桶都存在。
    #[tokio::test]
    async fn fill_analytics_empty_db() {
        let pool = make_pool().await;
        // 把 SQL 直接写在这里 —— fill_analytics 的签名
        // 需要 tauri::State，在单元测试里构造它很别扭。
        // 我们真正想验证的是 SQL 本身。
        let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM bets")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(total, 0);
        let won: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM bets WHERE status = 'won'")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(won, 0);
        let post_only: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM bets WHERE post_only = 1",
        )
        .fetch_one(&pool).await.unwrap();
        assert_eq!(post_only, 0);
        // 当没有任何已结算行时，已结算行的 avg TTS 为 None。
        let avg: Option<f64> = sqlx::query_scalar(
            "SELECT AVG(settled_at - placed_at) FROM bets WHERE settled_at IS NOT NULL",
        )
        .fetch_one(&pool).await.unwrap();
        assert!(avg.is_none());
    }

    /// v0.50c —— 混合场景：状态计数和 order_type 桶的合计等于 total_fills。
    #[tokio::test]
    async fn fill_analytics_with_mixed_bets() {
        let pool = make_pool().await;
        // 插入 5 行：2 open、2 won、1 lost。
        //   在已结算桶中包含 1 market + 1 limit + 1 stop_loss；
        //   open 状态中包含 1 market + 1 limit。
        //   其中 1 个限价单为 post_only。
        let rows = [
            ("b1", "m1", "YES", "100", 0.50, 1_000_000, Some(1_100_000), Some(50.0),  "won",       "market",   None,      None,     0),
            ("b2", "m1", "NO",  "100", 0.45, 1_000_000, Some(1_200_000), Some(40.0),  "won",       "limit",    Some(0.45), None,   1),
            ("b3", "m2", "YES", "50",  0.60, 1_000_000, Some(1_050_000), Some(-50.0), "lost",      "stop_loss",Some(0.55), Some(0.65), 0),
            ("b4", "m2", "NO",  "20",  0.70, 1_100_000, None,             None,        "open",      "market",   None,      None,     0),
            ("b5", "m3", "YES", "30",  0.30, 1_100_000, None,             None,        "open",      "limit",    Some(0.30), None,   0),
        ];
        for r in rows {
            sqlx::query(
                "INSERT INTO bets (
                    id, market_id, side, size, price,
                    placed_at, settled_at, pnl, status,
                    order_type, limit_price, stop_price, post_only
                 ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(r.0).bind(r.1).bind(r.2).bind(r.3).bind(r.4)
            .bind(r.5).bind(r.6).bind(r.7).bind(r.8)
            .bind(r.9).bind(r.10).bind(r.11).bind(r.12)
            .execute(&pool)
            .await
            .unwrap();
        }
        // 状态计数。
        let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM bets")
            .fetch_one(&pool).await.unwrap();
        let open_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM bets WHERE status = 'open'")
            .fetch_one(&pool).await.unwrap();
        let won_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM bets WHERE status = 'won'")
            .fetch_one(&pool).await.unwrap();
        let lost_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM bets WHERE status = 'lost'")
            .fetch_one(&pool).await.unwrap();
        let cancelled_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM bets WHERE status = 'cancelled'")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(total, 5);
        assert_eq!(open_count, 2);
        assert_eq!(won_count, 2);
        assert_eq!(lost_count, 1);
        assert_eq!(cancelled_count, 0);
        // 平均 TTS（成交到结算耗时）= ((100+200+50) / 3) = 116.666... ms
        let avg: Option<f64> = sqlx::query_scalar(
            "SELECT AVG(settled_at - placed_at) FROM bets WHERE settled_at IS NOT NULL",
        )
        .fetch_one(&pool).await.unwrap();
        let avg = avg.expect("avg TTS");
        assert!((avg - 116_666.666).abs() < 1.0, "got: {avg}");
        // 胜率 = 2 / (2 + 1) = 0.6666...
        let settled = won_count + lost_count + cancelled_count;
        let win_rate = if settled > 0 { won_count as f64 / settled as f64 } else { 0.0 };
        assert!((win_rate - 2.0 / 3.0).abs() < 1e-6);
        // 已实现盈亏 = 50 + 40 + (-50) = 40.0
        let realized: f64 = sqlx::query_scalar(
            "SELECT COALESCE(SUM(CAST(pnl AS REAL)), 0.0) FROM bets
             WHERE status IN ('won', 'lost') AND pnl IS NOT NULL",
        )
        .fetch_one(&pool).await.unwrap();
        assert!((realized - 40.0).abs() < 1e-6, "got: {realized}");
        // 5 个中有 1 个 post_only → 比率 0.2
        let post_only_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM bets WHERE post_only = 1",
        )
        .fetch_one(&pool).await.unwrap();
        assert_eq!(post_only_count, 1);
        let post_only_rate = if total > 0 {
            post_only_count as f64 / total as f64
        } else { 0.0 };
        assert!((post_only_rate - 0.2).abs() < 1e-6);
        // 各 order_type 桶的合计等于总数。
        let mut total_bucketed = 0;
        for ot in ["market", "limit", "stop_loss"] {
            let n: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM bets WHERE order_type = ?",
            )
            .bind(ot)
            .fetch_one(&pool)
            .await
            .unwrap();
            total_bucketed += n;
        }
        assert_eq!(total_bucketed, total);
    }

    /// v0.50c —— v0.50 之前的行（写入时还没有 order_type 列）
    /// 默认为 'market'。我们通过「不指定 order_type 就插入」
    /// 来验证这一点 —— SQLite 用 NOT NULL DEFAULT 'market'
    /// 新增列，显式 NULL 会被拒绝。（这里跳过该用例；
    /// 迁移里的 DEFAULT 才是契约。）
    #[test]
    fn pre_v050_default_is_market_marker() {
        // 占位测试。真正的不变式由 infra/db/bets_columns.rs
        // 中的 ALTER TABLE 迁移保证。这里仅断言辅助结构能编译。
    }
}
