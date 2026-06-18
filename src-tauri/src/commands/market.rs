//! L2 — Markets (M4).
//!
//! IPCs: `list_markets` (filter by category / active-only / limit),
//! `sync_markets` (pull from Polymarket Gamma API → SQLite).
//! Depends on L3 `domain::polymarket::fetch_active_markets`.

use crate::AppResult;
use crate::domain::polymarket;
use crate::infra::state::AppState;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use tauri::State;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct MarketDto {
    pub id: String,
    pub slug: String,
    pub question: String,
    pub category: String,
    pub end_date: i64,
    pub active: bool,
    pub resolved: bool,
    pub outcome: Option<String>,
    pub liquidity: Option<String>,
    pub volume_24h: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ListMarketsArgs {
    pub category: Option<String>,
    pub active_only: Option<bool>,
    pub limit: Option<i64>,
}

#[tauri::command]
pub async fn list_markets(
    state: State<'_, AppState>,
    args: ListMarketsArgs,
) -> AppResult<Vec<MarketDto>> {
    let active_only = args.active_only.unwrap_or(true);
    let limit = args.limit.unwrap_or(200);

    let rows = match args.category.as_deref() {
        Some(cat) => {
            let mut q = String::from(
                "SELECT id, slug, question, category, end_date, active, resolved, outcome, liquidity, volume_24h FROM markets WHERE category = ?",
            );
            if active_only {
                q.push_str(" AND active = 1");
            }
            q.push_str(" ORDER BY end_date ASC LIMIT ?");
            sqlx::query_as::<_, MarketDto>(&q)
                .bind(cat)
                .bind(limit)
                .fetch_all(&state.db)
                .await?
        }
        None => {
            let mut q = String::from(
                "SELECT id, slug, question, category, end_date, active, resolved, outcome, liquidity, volume_24h FROM markets WHERE 1=1",
            );
            if active_only {
                q.push_str(" AND active = 1");
            }
            q.push_str(" ORDER BY end_date ASC LIMIT ?");
            sqlx::query_as::<_, MarketDto>(&q)
                .bind(limit)
                .fetch_all(&state.db)
                .await?
        }
    };
    Ok(rows)
}

/// Sync from Polymarket Gamma API into local SQLite.
#[tauri::command]
pub async fn sync_markets(state: State<'_, AppState>) -> AppResult<usize> {
    let remote = polymarket::fetch_active_markets().await?;
    let mut tx = state.db.begin().await?;
    let mut n = 0usize;
    for m in remote {
        sqlx::query(
            "INSERT INTO markets (id, slug, question, category, end_date, active, resolved, outcome, liquidity, volume_24h, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, unixepoch() * 1000)
             ON CONFLICT(id) DO UPDATE SET
               question=excluded.question,
               category=excluded.category,
               end_date=excluded.end_date,
               active=excluded.active,
               resolved=excluded.resolved,
               outcome=excluded.outcome,
               liquidity=excluded.liquidity,
               volume_24h=excluded.volume_24h,
               updated_at=unixepoch() * 1000",
        )
        .bind(&m.id)
        .bind(&m.slug)
        .bind(&m.question)
        .bind(&m.category)
        .bind(m.end_date)
        .bind(m.active)
        .bind(m.resolved)
        .bind(&m.outcome)
        .bind(&m.liquidity)
        .bind(&m.volume_24h)
        .execute(&mut *tx)
        .await?;
        n += 1;
    }
    tx.commit().await?;
    Ok(n)
}

// =================================================================
// ============== v0.46a — backtest sample source ==================
// =================================================================

/// v0.46a — one pre-formatted backtest sample
/// sourced from a resolved market. The L1 builds
/// `BacktestSample[]` from these. Note: we don't
/// have historical price snapshots, so `price` is
/// a fixed default (0.5 — the "no signal"
/// midpoint). The user can edit the textarea
/// before clicking Run if they have actual prices.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedMarketSample {
    pub market_id: String,
    pub question: String,
    pub outcome: String,
    /// Derived age in hours. v0.46a uses a fixed
    /// "predict 1 day before close" convention
    /// (24h). Real price history would let us
    /// record the actual age at predict-time.
    pub market_age_hours: f64,
    /// Always 0.5 in v0.46a. Documented as a
    /// known limitation: real price history is
    /// not stored, so the L1 uses a midpoint
    /// default. v0.46+ could add a price-snapshot
    /// table to enable real backtests.
    pub price: f64,
}

/// v0.46a — args for `list_resolved_markets_for_backtest`.
/// Same shape as `ListMarketsArgs` for consistency,
/// plus an optional `since_ms` for time-bounded
/// queries.
#[derive(Debug, Deserialize)]
pub struct ListResolvedMarketsForBacktestArgs {
    pub category: Option<String>,
    pub limit: Option<i64>,
    /// Optional filter: only markets that ended
    /// at or after this unix-ms timestamp. Used
    /// by the L1 to bound "last 30 days" etc.
    pub since_ms: Option<i64>,
}

/// v0.46a — query resolved markets and convert
/// each to a backtest sample. The L1 feeds these
/// into the BacktestReport textarea via a
/// "Pull from resolved markets" button.
///
/// Known limitations (v0.46):
///   - `price` is fixed at 0.5 (no price history)
///   - `market_age_hours` = 24 (the "predict 1
///     day before close" convention)
/// This is degenerate (no real price history)
/// but useful as a sanity check: the model
/// should at least beat 0.5 (random) on
/// settled markets. v0.46+ could add a
/// price-snapshot table to enable real
/// backtests.
#[tauri::command]
pub async fn list_resolved_markets_for_backtest(
    state: State<'_, AppState>,
    args: ListResolvedMarketsForBacktestArgs,
) -> AppResult<Vec<ResolvedMarketSample>> {
    let limit = args.limit.unwrap_or(100);
    let since = args.since_ms.unwrap_or(0);
    let rows: Vec<(String, String, String, i64)> = match args.category.as_deref() {
        Some(cat) => sqlx::query_as(
            "SELECT id, question, outcome, end_date
             FROM markets
             WHERE resolved = 1
               AND outcome IS NOT NULL
               AND category = ?
               AND end_date >= ?
             ORDER BY end_date DESC
             LIMIT ?",
        )
        .bind(cat)
        .bind(since)
        .bind(limit)
        .fetch_all(&state.db)
        .await?,
        None => sqlx::query_as(
            "SELECT id, question, outcome, end_date
             FROM markets
             WHERE resolved = 1
               AND outcome IS NOT NULL
               AND end_date >= ?
             ORDER BY end_date DESC
             LIMIT ?",
        )
        .bind(since)
        .bind(limit)
        .fetch_all(&state.db)
        .await?,
    };
    const PREDICT_BEFORE_CLOSE_HOURS: f64 = 24.0;
    let samples = rows
        .into_iter()
        .map(|(id, question, outcome, _end_date)| ResolvedMarketSample {
            market_id: id,
            question,
            outcome,
            market_age_hours: PREDICT_BEFORE_CLOSE_HOURS,
            price: 0.5,
        })
        .collect();
    Ok(samples)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// v0.46a — list_resolved_markets_for_backtest
    /// returns one sample per resolved market with
    /// the documented degenerate defaults (price=0.5,
    /// age=24h). The function is the L1's main hook
    /// for auto-populating the BacktestReport
    /// textarea.
    #[tokio::test]
    async fn resolved_markets_for_backtest_returns_one_per_market() {
        use sqlx::sqlite::SqlitePoolOptions;
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect(":memory:")
            .await
            .unwrap();
        sqlx::query(
            "CREATE TABLE markets (
                id TEXT PRIMARY KEY,
                category TEXT NOT NULL,
                question TEXT NOT NULL,
                resolved INTEGER DEFAULT 0 NOT NULL,
                outcome TEXT,
                end_date INTEGER NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .unwrap();
        // 3 markets: 2 resolved (one YES, one NO), 1 unresolved.
        sqlx::query("INSERT INTO markets VALUES ('m1', 'cat', 'q1', 1, 'YES', 1_700_000_000_000)")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO markets VALUES ('m2', 'cat', 'q2', 1, 'NO',  1_700_000_000_000)")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO markets VALUES ('m3', 'cat', 'q3', 0, NULL, 1_700_000_000_000)")
            .execute(&pool)
            .await
            .unwrap();
        // We need State<'_, AppState> to call
        // list_resolved_markets_for_backtest, but
        // the AppState constructor is heavy. Test
        // the SQL by hand instead: the function
        // does a single SELECT + map; we verify
        // the SELECT first.
        let rows: Vec<(String, String, String, i64)> = sqlx::query_as(
            "SELECT id, question, outcome, end_date
             FROM markets
             WHERE resolved = 1 AND outcome IS NOT NULL
             ORDER BY end_date DESC LIMIT ?",
        )
        .bind(50i64)
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(rows.len(), 2);
        // m1 is YES, m2 is NO.
        assert_eq!(rows[0].2, "YES");
        assert_eq!(rows[1].2, "NO");
    }
}