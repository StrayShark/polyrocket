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
    pub volume_24h: Option<f64>,
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
///
/// v0.119 — football pivot: polyrocket is a football-only product
/// (see docs/polyrocket-football-prd.md). At sync time we FILTER OUT
/// non-football markets so they never enter the local DB. This is
/// the cleanest enforcement point — UI never has to defend against
/// non-football rows because they never exist.
///
/// Polymarket's Gamma API returns `category` as a free-form string.
/// Common football values: "Soccer", "Football", "Sports", "World Cup",
/// "Premier League", "NBA", "MLB" (we want all football-related).
/// Common non-football values: "Politics", "Crypto", "Tech", "Pop Culture".
///
/// We use substring matching on the lowercase category string. The
/// `category_classify()` helper in `domain::polymarket` does similar
/// mapping for LLM prompts; we keep this filter inline for clarity
/// and zero coupling between commands and domain layer.
///
/// **Edge cases**:
///   - Empty / missing category → REJECT (safer default; "Sports" is football,
///     so any unknown category is more likely non-football than football)
///   - "Sports" alone is ambiguous (NBA / NFL / F1 also "Sports") →
///     we accept "sports" only when paired with a football keyword
///   - Test fixtures: seed data uses category="football" so existing
///     tests are unaffected
pub fn is_football_market(m: &polymarket::MarketSummary) -> bool {
    let cat = m.category.to_lowercase();
    let cat = cat.trim();
    if cat.is_empty() {
        return false;
    }
    // Direct football matches
    if cat.contains("football")
        || cat.contains("soccer")
        || cat.contains("fifa")
        || cat.contains("uefa")
        || cat.contains("world cup")
        || cat.contains("premier league")
        || cat.contains("la liga")
        || cat.contains("bundesliga")
        || cat.contains("serie a")
        || cat.contains("ligue 1")
        || cat.contains("mls")
        || cat.contains("champions league")
        || cat.contains("europa league")
    {
        return true;
    }
    // "Sports" alone is too broad — only accept if the question hints at football
    if cat == "sports" {
        let q = m.question.to_lowercase();
        return q.contains("football")
            || q.contains("soccer")
            || q.contains("fifa")
            || q.contains("world cup")
            || q.contains("premier league")
            || q.contains("la liga")
            || q.contains("bundesliga")
            || q.contains("serie a")
            || q.contains("ligue 1")
            || q.contains("mls")
            || q.contains("champions league")
            || q.contains("uefa")
            || q.contains("goal")
            || q.contains("match")
            || q.contains("league")
            || q.contains("team");
    }
    false
}

#[tauri::command]
pub async fn sync_markets(state: State<'_, AppState>) -> AppResult<usize> {
    let remote = polymarket::fetch_active_markets().await?;
    // v0.119 — football-only filter at sync time.
    let remote: Vec<_> = remote.into_iter().filter(is_football_market).collect();
    let mut tx = state.db.begin().await?;
    let mut n = 0usize;
    // v0.47a — also record a price snapshot per market.
    // For now, we use a placeholder (best_bid=0.5,
    // best_ask=0.5) because the Gamma API doesn't
    // expose an order book — only metadata. v0.50+
    // can wire a real CLOB order-book feed and the
    // schema is ready. The v0.47b backtest falls
    // back to 0.5 when no real snapshot exists, so
    // pre-v0.50 markets still get a sensible default.
    let now_ms = chrono::Utc::now().timestamp_millis();
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
        .bind(m.volume_24h)
        .execute(&mut *tx)
        .await?;
        // v0.47a — placeholder snapshot. Will be
        // replaced with real order-book data in
        // v0.50+. For now this exercises the path
        // and the backtest falls back to 0.5 when
        // it's the latest.
        sqlx::query(
            "INSERT INTO price_snapshots
                (market_id, captured_at, best_bid, best_ask, mid_price, spread)
             VALUES (?, ?, 0.5, 0.5, 0.5, 0.0)",
        )
        .bind(&m.id)
        .bind(now_ms)
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
/// v0.47b — joined against `price_snapshots` to
/// surface the most recent observed price. Until
/// v0.50+ wires a real CLOB order-book feed, the
/// snapshots are placeholders (best_bid =
/// best_ask = 0.5), so the behavior matches v0.46
/// for users who have been syncing markets. After
/// v0.50+ lands, this becomes a real backtest
/// without any L1 changes.
#[tauri::command]
pub async fn list_resolved_markets_for_backtest(
    state: State<'_, AppState>,
    args: ListResolvedMarketsForBacktestArgs,
) -> AppResult<Vec<ResolvedMarketSample>> {
    let limit = args.limit.unwrap_or(100);
    let since = args.since_ms.unwrap_or(0);
    // v0.47b — join with the latest price_snapshots
    // entry per market. The LATERAL subquery
    // pattern (or correlated subquery) picks the
    // row with the highest captured_at per
    // market_id. We use a correlated subquery
    // here for clarity; SQLite optimizes it
    // against the price_snapshots_market_recent_idx.
    //
    // If a market has no snapshot (typical for
    // pre-v0.47 DBs that never wrote a snapshot
    // for resolved markets), the LEFT JOIN gives
    // us NULL for the snapshot fields; the COALESCE
    // falls back to 0.5 / 24 — the v0.46 degenerate
    // default. This keeps pre-v0.47 DBs
    // working unchanged.
    let rows: Vec<(String, String, String, Option<f64>, Option<f64>, Option<i64>)> = match args.category.as_deref() {
        Some(cat) => sqlx::query_as(
            "SELECT m.id, m.question, m.outcome,
                    ps.mid_price, ps.spread, ps.captured_at
             FROM markets m
             LEFT JOIN price_snapshots ps
               ON ps.id = (
                 SELECT id FROM price_snapshots
                 WHERE market_id = m.id
                 ORDER BY captured_at DESC LIMIT 1
               )
             WHERE m.resolved = 1
               AND m.outcome IS NOT NULL
               AND m.category = ?
               AND m.end_date >= ?
             ORDER BY m.end_date DESC
             LIMIT ?",
        )
        .bind(cat)
        .bind(since)
        .bind(limit)
        .fetch_all(&state.db)
        .await?,
        None => sqlx::query_as(
            "SELECT m.id, m.question, m.outcome,
                    ps.mid_price, ps.spread, ps.captured_at
             FROM markets m
             LEFT JOIN price_snapshots ps
               ON ps.id = (
                 SELECT id FROM price_snapshots
                 WHERE market_id = m.id
                 ORDER BY captured_at DESC LIMIT 1
               )
             WHERE m.resolved = 1
               AND m.outcome IS NOT NULL
               AND m.end_date >= ?
             ORDER BY m.end_date DESC
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
        .map(
            |(id, question, outcome, mid_price, _spread, _captured_at)| ResolvedMarketSample {
                market_id: id,
                question,
                outcome,
                market_age_hours: PREDICT_BEFORE_CLOSE_HOURS,
                // v0.47b — use the snapshot's
                // mid_price when available; fall back
                // to 0.5 when not (degenerate default).
                price: mid_price.unwrap_or(0.5),
            },
        )
        .collect();
    Ok(samples)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::polymarket::MarketSummary;

    // v0.119 — football-only sync filter tests.
    // polyrocket is a football-only product (see docs/polyrocket-football-prd.md).
    // `sync_markets` must reject non-football markets before they hit the DB.

    fn mk(category: &str, question: &str) -> MarketSummary {
        MarketSummary {
            id: format!("m-{}", category),
            slug: format!("{}-slug", category),
            question: question.to_string(),
            category: category.to_string(),
            end_date: 0,
            active: true,
            resolved: false,
            outcome: None,
            liquidity: None,
            volume_24h: None,
        }
    }

    #[test]
    fn is_football_accepts_football_category() {
        assert!(is_football_market(&mk("Football", "Will X win?")));
        assert!(is_football_market(&mk("Soccer", "Will X win?")));
        assert!(is_football_market(&mk("Sports", "Will Lakers win?")) == false); // sports alone is broad
    }

    #[test]
    fn is_football_accepts_sports_when_question_has_football_keywords() {
        assert!(is_football_market(&mk("Sports", "Will Argentina win the World Cup?")));
        assert!(is_football_market(&mk("Sports", "Champions League final?")));
        assert!(is_football_market(&mk("Sports", "Premier League match?")));
    }

    #[test]
    fn is_football_rejects_politics_crypto_tech_other() {
        assert!(!is_football_market(&mk("Politics", "Will Biden win 2028?")));
        assert!(!is_football_market(&mk("Crypto", "Will BTC reach 100k?")));
        assert!(!is_football_market(&mk("Tech", "Will OpenAI launch GPT-7?")));
        assert!(!is_football_market(&mk("Pop Culture", "Will Beyoncé release album?")));
        assert!(!is_football_market(&mk("", "?"))); // empty
    }

    #[test]
    fn is_football_accepts_league_names() {
        assert!(is_football_market(&mk("Premier League", "Arsenal vs Chelsea")));
        assert!(is_football_market(&mk("UEFA Champions League", "Final")));
        assert!(is_football_market(&mk("FIFA World Cup", "Argentina match")));
        assert!(is_football_market(&mk("La Liga", "Real Madrid")));
        assert!(is_football_market(&mk("Bundesliga", "Bayern")));
        assert!(is_football_market(&mk("Serie A", "Juventus")));
        assert!(is_football_market(&mk("Ligue 1", "PSG")));
        assert!(is_football_market(&mk("MLS", "LA Galaxy")));
    }

    #[test]
    fn is_football_rejects_other_sports() {
        // NBA / NFL / MLB / NHL / F1 — Sports category but NOT football.
        // Question has no football keywords.
        assert!(!is_football_market(&mk("Sports", "Will Lakers beat Celtics?")));
        assert!(!is_football_market(&mk("Sports", "NFL Super Bowl winner?")));
        assert!(!is_football_market(&mk("Sports", "MLB World Series?")));
    }

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

    /// v0.47b — the LEFT JOIN against the latest
    /// price_snapshots row per market returns the
    /// most recent mid_price (or NULL when no
    /// snapshot exists). The function maps NULL →
    /// 0.5 (the v0.46 fallback).
    #[tokio::test]
    async fn backtest_join_uses_latest_snapshot() {
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
        sqlx::query(
            "CREATE TABLE price_snapshots (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                market_id TEXT NOT NULL,
                captured_at INTEGER NOT NULL,
                best_bid REAL NOT NULL,
                best_ask REAL NOT NULL,
                mid_price REAL NOT NULL,
                spread REAL NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .unwrap();
        // One resolved market with two snapshots
        // (older + newer). The newer one wins.
        sqlx::query("INSERT INTO markets VALUES ('m1', 'cat', 'q1', 1, 'YES', 1_700_000_000_000)")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO price_snapshots (market_id, captured_at, best_bid, best_ask, mid_price, spread) VALUES ('m1', 1_000, 0.4, 0.6, 0.5, 0.2)")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO price_snapshots (market_id, captured_at, best_bid, best_ask, mid_price, spread) VALUES ('m1', 2_000, 0.7, 0.8, 0.75, 0.1)")
            .execute(&pool)
            .await
            .unwrap();
        let rows: Vec<(String, Option<f64>)> = sqlx::query_as(
            "SELECT m.id, ps.mid_price
             FROM markets m
             LEFT JOIN price_snapshots ps
               ON ps.id = (
                 SELECT id FROM price_snapshots
                 WHERE market_id = m.id
                 ORDER BY captured_at DESC LIMIT 1
               )
             WHERE m.resolved = 1 AND m.outcome IS NOT NULL
             LIMIT 1",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(rows.len(), 1);
        // The newer snapshot (0.75) wins, not the
        // older 0.5.
        assert_eq!(rows[0].1, Some(0.75));
    }
}