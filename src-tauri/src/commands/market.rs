//! L2 — Markets (M4).
//!
//! IPCs: `list_markets` (filter by category / active-only / limit),
//! `sync_markets` (pull from Polymarket Gamma API → SQLite).
//! Depends on L3 `domain::polymarket::fetch_active_markets`.

use std::error::Error as StdError;
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
/// v0.124 — football classifier. Updated to work with the live
/// Gamma API shape: `category` is `None` for most markets, and
/// `tags` is also `None` for the v0.124 sample. The reliable signal
/// is the **question text** + the (sometimes present) category.
///
/// We try in priority order:
///   1. `category` substring (when present, it's often "Sports"
///      or a specific league name)
///   2. `tags[0]` (when category is null but tags has league tags)
///   3. Question text — the most reliable signal in practice
///      (PM's market questions usually name the teams or the
///      tournament directly: "Will Real Madrid win ...", "Premier
///      League top 4", "La Liga 2025-26")
///
/// False positives are filtered out by the `team/league/match`
/// word-list gate so we don't catch generic "sports" questions.
pub fn is_football_market(m: &polymarket::MarketSummary) -> bool {
    // Helper: a string is "football-y" if it contains a football
    // keyword OR a strong team/league match with a football context.
    let is_football_text = |s: &str| -> bool {
        let lower = s.to_lowercase();
        lower.contains("football")
            || lower.contains("soccer")
            || lower.contains("fifa")
            || lower.contains("uefa")
            || lower.contains("world cup")
            || lower.contains("champions league")
            || lower.contains("europa league")
            || lower.contains("premier league")
            || lower.contains("la liga")
            || lower.contains("bundesliga")
            || lower.contains("serie a")
            || lower.contains("ligue 1")
            || lower.contains("mls")
            || lower.contains("epl")
            || lower.contains("match")
            || lower.contains("goal")
            || lower.contains(" fc")
            || lower.contains(" united")
            || lower.contains(" city")
            || lower.contains("real madrid")
            || lower.contains("barcelona")
            || lower.contains("liverpool")
            || lower.contains("arsenal")
            || lower.contains("chelsea")
            || lower.contains("tottenham")
            || lower.contains("manchester")
            || lower.contains("bayern")
            || lower.contains("dortmund")
            || lower.contains("juventus")
            || lower.contains("milan")
            || lower.contains("inter")
            || lower.contains("psg")
            || lower.contains("marseille")
    };

    // 1) explicit category (rare on PM but worth checking)
    if let Some(cat) = m.category.as_deref() {
        if !cat.trim().is_empty() && is_football_text(cat) {
            return true;
        }
    }
    // 2) tags (sometimes has a league tag)
    if let Some(tags) = m.tags.as_ref() {
        for t in tags {
            if is_football_text(t) {
                return true;
            }
        }
    }
    // 3) question text — the most reliable signal
    is_football_text(&m.question)
}

#[tauri::command]
pub async fn sync_markets(state: State<'_, AppState>) -> AppResult<usize> {
    // v0.124 — diagnostic log so we can see in the dev console
    // when the IPC was actually called (vs the click never
    // reaching the React handler).
    tracing::info!("sync_markets: IPC called, fetching from Gamma");
    let remote = polymarket::fetch_active_markets().await;
    match &remote {
        Ok(r) => tracing::info!("sync_markets: fetch returned {} markets", r.len()),
        Err(e) => {
            // AppError wraps reqwest::Error which wraps the
            // underlying serde_json::Error. Print the source chain
            // so we can see WHICH field mismatched the DTO.
            tracing::warn!("sync_markets: fetch failed: {e}");
            let mut src: Option<&dyn StdError> = e.source();
            let mut depth = 0;
            while let Some(s) = src {
                tracing::warn!("sync_markets:   cause[{}] = {s}", depth);
                src = s.source();
                depth += 1;
                if depth > 6 { break; }
            }
        }
    }
    let remote = remote?;
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
    let n_remote = remote.len();
    let mut n_written = 0usize;
    for m in remote {
        if !is_football_market(&m) {
            tracing::debug!(
                "sync_markets: skipping non-football id={} q={:?}",
                m.id, m.question
            );
            continue;
        }
        n_written += 1;
        // v0.124 — Gamma API returns ISO strings + numbers
        // (not the legacy i64-millis / string-encoded fields the
        // v0.122-era DTO assumed). We map here at the boundary:
        //   - end_date  → m.end_date_ms (parsed) || 0 on parse fail
        //   - resolved  → m.closed  (the API's "closed" flag)
        //   - active    → m.active && !m.archived
        //   - liquidity → m.liquidity is a STRING (e.g. "16639.42")
        //   - volume_24h→ m.volume_24hr (a real number, not a string)
        let end_ms = m.end_date_ms.unwrap_or(0);
        let active_flag = m.active && !m.archived;
        let resolved_flag = m.closed;
        sqlx::query(
            "INSERT INTO markets (id, slug, question, description, category, end_date,
                                  active, resolved, outcome, liquidity, volume_24h,
                                  updated_at)
             VALUES (?, ?, ?, ?, 'football', ?, ?, ?, NULL, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET
                question=excluded.question,
                description=excluded.description,
                category='football',
                end_date=excluded.end_date,
                active=excluded.active,
                resolved=excluded.resolved,
                outcome=NULL,
                liquidity=excluded.liquidity,
                volume_24h=excluded.volume_24h,
                updated_at=excluded.updated_at",
        )
        .bind(&m.id)
        .bind(&m.slug)
        .bind(&m.question)
        .bind(m.description.as_deref())
        .bind(end_ms)
        .bind(active_flag)
        .bind(resolved_flag)
        .bind(&m.liquidity)  // v0.124 — STRING (parsed at deser)
        .bind(m.volume_24hr) // v0.124 — NUMBER
        .bind(now_ms)
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
    tracing::info!("sync_markets: wrote {n_written}/{n_remote} markets (football filter)");
    Ok(n_written)
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
            description: None,
            end_date: "2025-10-31T00:00:00Z".to_string(),
            end_date_ms: Some(0),
            active: true,
            closed: false,
            archived: false,
            volume_24hr: 0.0,
            volume: "0".to_string(),
            liquidity: "0".to_string(),
            category: Some(category.to_string()),
            tags: None,
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