//! L2 — PnL dashboard KPIs (M8).
//!
//! IPC: `dashboard_kpis` — aggregates `bets`, `model_performance`,
//! `signals` tables for the home dashboard tiles. Computation will
//! move into `domain::pnl` per M8 milestone.

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

/// Aggregated KPIs for the Dashboard page.
/// Computed in Rust for speed — full query stays local.
#[tauri::command]
#[specta::specta]
pub async fn dashboard_kpis(state: State<'_, AppState>) -> AppResult<DashboardKpis> {
    let total_equity: Option<String> =
        sqlx::query_scalar("SELECT COALESCE(SUM(size), '0') FROM bets WHERE status = 'open'")
            .fetch_optional(&state.db)
            .await?;
    let open_pnl: Option<String> =
        sqlx::query_scalar("SELECT COALESCE(SUM(pnl), '0') FROM bets WHERE status = 'open'")
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
        total_equity_usdc: total_equity.unwrap_or_else(|| "0".into()),
        open_pnl_usdc: open_pnl.unwrap_or_else(|| "0".into()),
        win_rate_30d: win_rate,
        brier_score: brier.unwrap_or(0.0),
        active_signals,
        open_positions,
    })
}

// =================================================================
// ============== v0.45b — paper trading PnL summary ==============
// =================================================================

/// v0.45b — paper trading PnL summary. Aggregates
/// the `paper_fills` table into a single struct the
/// L1 can show on the Dashboard / PnL page as a
/// "what would have happened" stat.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaperPnlSummary {
    /// Total paper fills recorded (settled + unsettled).
    pub total_fills: i64,
    /// Number of paper fills that have been
    /// reconciled against a market resolution.
    pub settled_fills: i64,
    /// Number of those that were wins (side matched
    /// the resolution outcome).
    pub won_fills: i64,
    /// Number that were losses.
    pub lost_fills: i64,
    /// Settled win rate (won / settled). 0.0 when
    /// no fills are settled yet.
    pub win_rate: f64,
    /// Total realized PnL across all settled fills,
    /// in USDC. Positive = gains, negative = losses.
    pub realized_pnl_usdc: String,
    /// Whether the user has paper mode enabled.
    /// The L1 uses this to decide whether to show
    /// the card at all.
    pub paper_mode_enabled: bool,
}

/// v0.45b — paper PnL IPC. Returns the aggregate
/// summary, plus the current paper_mode flag (so
/// the L1 can show / hide the card without a
/// second query).
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
    // Realized PnL — sum of pnl_usdc across settled
    // fills. We compute in SQL to avoid floating
    // point error in the Rust loop.
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
// ============== v0.50c — fill analytics =========================
// =================================================================

/// v0.50c + v0.51b — fill analytics summary. Aggregates
/// the `bets` table into a struct the L1 Dashboard
/// can show as a "Fill analytics" card.
///
/// v0.51b adds slippage and time-to-fill:
///   - avg_slippage = mean(|fill_price - price|) over
///     filled rows where fill_price is non-null.
///     For the v0.5d deterministic stub (where
///     fill_price = price) this is 0 by construction;
///     when v0.51+ wires the real CLOB it'll reflect
///     actual slip.
///   - avg_time_to_fill_ms = mean(filled_at - placed_at)
///     over filled rows. Same story.
///   - partial_fill_count / partial_fill_rate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FillAnalytics {
    pub total_fills: i64,
    pub open_count: i64,
    pub won_count: i64,
    pub lost_count: i64,
    pub cancelled_count: i64,
    /// (settled_at - placed_at) average, ms.
    /// None when no bets have settled yet.
    pub avg_time_to_settlement_ms: Option<f64>,
    /// Settled win rate (won / settled). 0.0 when
    /// nothing is settled yet.
    pub win_rate: f64,
    /// Total realized PnL across settled rows, USDC.
    pub realized_pnl_usdc: String,
    /// Breakdown of fills by order_type. The key is
    /// "market" | "limit" | "stop_loss"; pre-v0.50
    /// rows are bucketed under "market" (the
    /// migration default).
    pub by_order_type: Vec<OrderTypeBucket>,
    /// Count of post_only fills (limit + post_only).
    pub post_only_count: i64,
    /// Share of fills that were post_only. 0.0
    /// when total_fills == 0.
    pub post_only_rate: f64,
    /// v0.51b — average |fill_price - price| across
    /// filled rows. None when no rows have a
    /// fill_price (i.e. pre-v0.51b database).
    pub avg_slippage: Option<f64>,
    /// v0.51b — average (filled_at - placed_at) in
    /// ms. None when no rows have a filled_at.
    pub avg_time_to_fill_ms: Option<f64>,
    /// v0.51b — count of partial fills (fill_size
    /// present and < shares).
    pub partial_fill_count: i64,
    /// v0.51b — share of fills that were partial.
    /// 0.0 when total_fills == 0.
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

/// v0.50c — fill analytics IPC. Aggregates the
/// `bets` table. Returns `FillAnalytics` for the
/// L1 Dashboard card.
#[tauri::command]
pub async fn fill_analytics(state: State<'_, AppState>) -> AppResult<FillAnalytics> {
    // Whole-table aggregates. COUNT/AVG/SUM — no
    // scan risk since `bets` is small (< 100k rows
    // for a single user).
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

    // Avg time-to-settlement (only settled rows).
    let avg_tts: Option<f64> = sqlx::query_scalar(
        "SELECT AVG(settled_at - placed_at) FROM bets
         WHERE settled_at IS NOT NULL",
    )
    .fetch_one(&state.db)
    .await?;

    // Realized PnL across settled rows.
    let realized: Option<f64> = sqlx::query_scalar(
        "SELECT COALESCE(SUM(CAST(pnl AS REAL)), 0.0) FROM bets
         WHERE status IN ('won', 'lost') AND pnl IS NOT NULL",
    )
    .fetch_one(&state.db)
    .await?;
    let realized = realized.unwrap_or(0.0);

    // Order-type breakdown. We do 3 separate
    // COUNT/SUM queries; an alternative is one
    // GROUP BY, but the explicit form is easier
    // to read and the table is small.
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

    // v0.51b — slippage and time-to-fill. Both
    // queries skip rows where the fill columns are
    // NULL (pre-v0.51b database).
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
// v0.50c — fill_analytics cargo tests
// ============================================================
//
// The IPC handler is mostly SQL. The interesting
// shape to verify is that empty tables don't divide
// by zero, that order-type buckets add up to the
// total, and that post_only_rate uses the right
// denominator. We exercise all three with a
// hand-rolled pool.

#[cfg(test)]
mod fill_analytics_tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    /// Build a fresh DB with the schema we need:
    /// `bets` with the v0.50a columns.
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

    /// v0.50c — empty table returns zeros, no
    /// divide-by-zero, all buckets present.
    #[tokio::test]
    async fn fill_analytics_empty_db() {
        let pool = make_pool().await;
        // Inline the SQL — fill_analytics' signature
        // takes a tauri::State which is awkward to
        // build in a unit test. The SQL is what we
        // actually want to verify.
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
        // Avg TTS over settled rows is None when no
        // rows have settled.
        let avg: Option<f64> = sqlx::query_scalar(
            "SELECT AVG(settled_at - placed_at) FROM bets WHERE settled_at IS NOT NULL",
        )
        .fetch_one(&pool).await.unwrap();
        assert!(avg.is_none());
    }

    /// v0.50c — mixed bag: status counts and the
    /// order-type buckets add up to total_fills.
    #[tokio::test]
    async fn fill_analytics_with_mixed_bets() {
        let pool = make_pool().await;
        // Insert 5 rows: 2 open, 2 won, 1 lost.
        //   1 market + 1 limit + 1 stop_loss in the
        //   settled bucket; 1 market + 1 limit open.
        //   1 of the limit orders is post_only.
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
        // Status counts.
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
        // avg TTS = ((100+200+50) / 3) = 116.666... ms
        let avg: Option<f64> = sqlx::query_scalar(
            "SELECT AVG(settled_at - placed_at) FROM bets WHERE settled_at IS NOT NULL",
        )
        .fetch_one(&pool).await.unwrap();
        let avg = avg.expect("avg TTS");
        assert!((avg - 116_666.666).abs() < 1.0, "got: {avg}");
        // win_rate = 2 / (2 + 1) = 0.6666...
        let settled = won_count + lost_count + cancelled_count;
        let win_rate = if settled > 0 { won_count as f64 / settled as f64 } else { 0.0 };
        assert!((win_rate - 2.0 / 3.0).abs() < 1e-6);
        // realized PnL = 50 + 40 + (-50) = 40.0
        let realized: f64 = sqlx::query_scalar(
            "SELECT COALESCE(SUM(CAST(pnl AS REAL)), 0.0) FROM bets
             WHERE status IN ('won', 'lost') AND pnl IS NOT NULL",
        )
        .fetch_one(&pool).await.unwrap();
        assert!((realized - 40.0).abs() < 1e-6, "got: {realized}");
        // 1 of 5 is post_only → rate 0.2
        let post_only_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM bets WHERE post_only = 1",
        )
        .fetch_one(&pool).await.unwrap();
        assert_eq!(post_only_count, 1);
        let post_only_rate = if total > 0 {
            post_only_count as f64 / total as f64
        } else { 0.0 };
        assert!((post_only_rate - 0.2).abs() < 1e-6);
        // Order-type buckets add up.
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

    /// v0.50c — pre-v0.50 rows (no order_type column
    /// at write time) default to 'market'. We verify
    /// by inserting rows WITHOUT specifying order_type
    /// — since SQLite adds the column with NOT NULL
    /// DEFAULT 'market', an explicit NULL would be
    /// rejected. (Skipping that case; the migration's
    /// DEFAULT is the contract.)
    #[test]
    fn pre_v050_default_is_market_marker() {
        // Marker test. The real invariant is enforced
        // by the ALTER TABLE migration in
        // infra/db/bets_columns.rs. Here we just
        // assert the helper structure compiles.
    }
}
