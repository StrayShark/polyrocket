//! L2 —— 市场（M4）。
//!
//! IPC:`list_markets`（按 category / active-only / limit 过滤）、
//! `sync_markets`（从 Polymarket Gamma API 拉取 → SQLite）。
//! 依赖 L3 `domain::polymarket::fetch_active_markets`。

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

/// 从 Polymarket Gamma API 同步到本地 SQLite。
///
/// v0.119 —— 足球聚焦:polyrocket 是足球专一产品
/// （见 docs/polyrocket-football-prd.md）。同步时过滤掉
/// 非足球市场,使其永不进入本地 DB。这是
/// 最干净的强制执行点 —— UI 永远无需防御
/// 非足球行,因为它们根本不存在。
///
/// Polymarket Gamma API 的 `category` 是自由格式字符串。
/// 常见的足球值: "Soccer"、"Football"、"Sports"、"World Cup"、
/// "Premier League"、"NBA"、"MLB"（我们想要所有足球相关）。
/// 常见的非足球值: "Politics"、"Crypto"、"Tech"、"Pop Culture"。
///
/// 我们在小写 category 字符串上做子串匹配。
/// `domain::polymarket` 中的 `category_classify()` 辅助函数
/// 为 LLM prompt 做类似映射;此处保留内联过滤,
/// 保持命令层与 domain 层零耦合,且更清晰。
///
/// **边界情况**:
///   - 空 / 缺失 category → 拒绝（更安全的默认; "Sports" 属足球,
///     任何未知类别相比之下更可能是非足球）
///   - 仅 "Sports" 有歧义（NBA / NFL / F1 也属 "Sports"）→
///     仅当伴随足球关键词时才接受 "sports"
///   - 测试 fixture:seed 数据使用 category="football",
///     不影响现有测试
/// v0.124 —— 足球分类器。已适配实时 Gamma API 形态:
/// 大多数 market 的 `category` 为 None,v0.124 样本的
/// `tags` 也为 None。**question 文本**（加上有时存在的 category）
/// 是最可靠的信号。
///
/// 按优先级尝试:
///   1. `category` 子串（若存在,通常为 "Sports" 或具体联赛名）
///   2. `tags[0]`（category 为空、但 tags 含联赛标签时）
///   3. question 文本 —— 实践中最可靠的信号
///      （PM 的 market question 通常直接写明球队或赛事:
///      "Will Real Madrid win ..."、"Premier League top 4"、
///      "La Liga 2025-26"）
///
/// 通过 team/league/match 词表门控过滤误报,
/// 避免误抓通用 "sports" 问题。
pub fn is_football_market(m: &polymarket::MarketSummary) -> bool {
    // 辅助:字符串若包含足球关键词,或与足球上下文中的
    // 球队/联赛高度匹配,则视为「足球味」。
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

    // 1) 显式 category（PM 上少见,但值得检查）
    if let Some(cat) = m.category.as_deref() {
        if !cat.trim().is_empty() && is_football_text(cat) {
            return true;
        }
    }
    // 2) tags（有时含联赛标签）
    if let Some(tags) = m.tags.as_ref() {
        for t in tags {
            if is_football_text(t) {
                return true;
            }
        }
    }
    // 3) question 文本 —— 最可靠的信号
    is_football_text(&m.question)
}

#[tauri::command]
pub async fn sync_markets(state: State<'_, AppState>) -> AppResult<usize> {
    // v0.124 —— 诊断日志,便于在 dev 控制台查看
    // IPC 是否真的被调用（与点击未触达 React
    // handler 形成对比）。
    tracing::info!("sync_markets: IPC called, fetching from Gamma");
    let remote = polymarket::fetch_active_markets().await;
    match &remote {
        Ok(r) => tracing::info!("sync_markets: fetch returned {} markets", r.len()),
        Err(e) => {
            // AppError 包装 reqwest::Error,后者又包装
            // 底层的 serde_json::Error。打印 source chain,
            // 以便查看究竟是哪个字段与 DTO 不匹配。
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
    // v0.119 —— 同步时仅保留 football 的过滤。
    let remote: Vec<_> = remote.into_iter().filter(is_football_market).collect();
    let mut tx = state.db.begin().await?;
    let mut n = 0usize;
    // v0.47a —— 同时记录每个 market 的 price snapshot。
    // 目前使用占位（best_bid=0.5、best_ask=0.5）,
    // 因为 Gamma API 不暴露订单簿 —— 只有 metadata。
    // v0.50+ 可接入真实 CLOB 订单簿行情,
    // schema 已就绪。v0.47b backtest 在无真实 snapshot 时
    // 回退到 0.5,所以 v0.50 之前的 market 仍得到合理默认值。
    let now_ms = chrono::Utc::now().timestamp_millis();
    let n_remote = remote.len();
    let mut n_written = 0usize;
    for m in remote {
        if !is_football_market(&m) {
            tracing::info!(
                "sync_markets: skipping non-football id={} q={:?}",
                m.id, m.question
            );
            continue;
        }
        tracing::info!("sync_markets: inserting id={} q={:?}", m.id, m.question);
        n_written += 1;
        // v0.124 —— Gamma API 返回 ISO 字符串 + 数字
        // （不是 v0.122 时代 DTO 假设的
        // i64-millis / string-encoded 字段）。
        // 我们在边界处做映射:
        //   - end_date  → m.end_date_ms（解析后）|| 0 on parse fail
        //   - resolved  → m.closed（API 的 "closed" 标志）
        //   - active    → m.active && !m.archived
        //   - liquidity → m.liquidity 为 STRING（如 "16639.42"）
        //   - volume_24h→ m.volume_24hr（实数,非字符串）
        let end_ms = m.end_date_ms.unwrap_or(0);
        let active_flag = m.active && !m.archived;
        let resolved_flag = m.closed;
        sqlx::query(
            "INSERT INTO markets (id, slug, question, description, category, end_date,
                                  active, resolved, outcome, liquidity, volume_24h,
                                  created_at, updated_at)
             VALUES (?, ?, ?, ?, 'football', ?, ?, ?, NULL, ?, ?, ?, ?)
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
        .bind(&m.liquidity)  // v0.124 —— STRING（反序列化时已解析）
        .bind(m.volume_24hr) // v0.124 —— NUMBER
        .bind(now_ms)        // v0.125 —— created_at（schema 要求 NOT NULL,新增）
        .bind(now_ms)        // updated_at
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            tracing::warn!(
                "sync_markets: INSERT failed for id={} slug={:?} end={}: {e}",
                m.id, m.slug, m.end_date
            );
            e
        })?;
        // v0.47a —— 占位 snapshot。会在 v0.50+
        // 被真实订单簿数据替换。
        // 目前用于打通路径,backtest 在其为
        // 最新一条时回退到 0.5。
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
// ============== v0.46a —— backtest 样本源 ==================
// =================================================================

/// v0.46a —— 一条来自已结算 market 的预格式化 backtest 样本。
/// L1 据此构建 `BacktestSample[]`。注意:我们
/// 没有历史 price snapshot,因此 `price` 是
/// 固定默认值（0.5 —— 「无信号」中点）。
/// 如果用户掌握真实价格,可在点击 Run 之前编辑 textarea。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedMarketSample {
    pub market_id: String,
    pub question: String,
    pub outcome: String,
    /// 派生出的年龄（小时）。v0.46a 使用固定的
    /// 「在收市前 1 天预测」约定（24h）。
    /// 真实价格历史可让我们记录预测时的实际年龄。
    pub market_age_hours: f64,
    /// v0.46a 中始终为 0.5。已记录的已知限制:
    /// 未存储真实价格历史,因此 L1 使用中点默认值。
    /// v0.46+ 可新增 price-snapshot 表来支持真实 backtest。
    pub price: f64,
}

/// v0.46a —— `list_resolved_markets_for_backtest` 的参数。
/// 与 `ListMarketsArgs` 形状一致以保持统一,
/// 另含可选的 `since_ms` 用于时间窗口查询。
#[derive(Debug, Deserialize)]
pub struct ListResolvedMarketsForBacktestArgs {
    pub category: Option<String>,
    pub limit: Option<i64>,
    /// 可选过滤:仅保留 end_date ≥ 此 unix-ms 时间戳的
    /// market。L1 用它来限定「最近 30 天」等。
    pub since_ms: Option<i64>,
}

/// v0.46a —— 查询已结算的 market,并把每条
/// 转换为 backtest 样本。L1 通过「从已结算 market 拉取」
/// 按钮把这些样本填入 BacktestReport 的 textarea。
///
/// v0.47b —— 与 `price_snapshots` 做 JOIN,
/// 以展示最近观察到的价格。在 v0.50+ 接入
/// 真实 CLOB 订单簿之前,snapshot 是占位
/// （best_bid = best_ask = 0.5）,因此对一直
/// 同步市场的用户而言,行为与 v0.46 一致。
/// v0.50+ 落地后,无需任何 L1 改动即变为
/// 真正的 backtest。
#[tauri::command]
pub async fn list_resolved_markets_for_backtest(
    state: State<'_, AppState>,
    args: ListResolvedMarketsForBacktestArgs,
) -> AppResult<Vec<ResolvedMarketSample>> {
    let limit = args.limit.unwrap_or(100);
    let since = args.since_ms.unwrap_or(0);
    // v0.47b —— 与每个 market 的最近 price_snapshots
    // 条目做 JOIN。LATERAL 子查询模式（或相关子查询）
    // 会选出每个 market_id 下 captured_at 最大的
    // 行。为求清晰,这里使用相关子查询;SQLite 会基于
    // price_snapshots_market_recent_idx 优化。
    //
    // 若某个 market 没有 snapshot（v0.47 之前的 DB
    // 常见情况,从不为已结算 market 写 snapshot）,
    // LEFT JOIN 会让 snapshot 字段为 NULL;
    // COALESCE 回退到 0.5 / 24 —— 即 v0.46 的退化默认。
    // 这样 v0.47 之前的 DB 可保持原状继续工作。
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
                // v0.47b —— 优先使用 snapshot 的
                // mid_price;无 snapshot 时
                // 回退到 0.5（退化默认值）。
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

    // v0.119 —— 仅 football 同步过滤测试。
    // polyrocket 是足球专一产品（见 docs/polyrocket-football-prd.md）。
    // `sync_markets` 必须在其进入 DB 前拒绝非足球 market。

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
        assert!(is_football_market(&mk("Sports", "Will Lakers win?")) == false); // 单独的 sports 太宽泛
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
        assert!(!is_football_market(&mk("", "?"))); // 空字符串
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
        // NBA / NFL / MLB / NHL / F1 —— 属 Sports 类别但不属于足球。
        // 问题文本不含足球关键词。
        assert!(!is_football_market(&mk("Sports", "Will Lakers beat Celtics?")));
        assert!(!is_football_market(&mk("Sports", "NFL Super Bowl winner?")));
        assert!(!is_football_market(&mk("Sports", "MLB World Series?")));
    }

    /// v0.46a —— `list_resolved_markets_for_backtest`
    /// 对每个已结算 market 返回一条样本,采用
    /// 已记录的退化默认值（price=0.5、age=24h）。
    /// 该函数是 L1 自动填充 BacktestReport
    /// textarea 的主要挂钩点。
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
        // 3 个 market:2 个已结算（一个 YES、一个 NO）,1 个未结算。
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
        // 调用 list_resolved_markets_for_backtest 需要
        // State<'_, AppState>,但 AppState 构造较重。
        // 因此改为手测 SQL:该函数只做
        // 一次 SELECT + map;我们先验证 SELECT。
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
        // m1 为 YES,m2 为 NO。
        assert_eq!(rows[0].2, "YES");
        assert_eq!(rows[1].2, "NO");
    }

    /// v0.47b —— LEFT JOIN 每个 market 最近的
    /// price_snapshots 行,返回最近的 mid_price
    /// （无 snapshot 时为 NULL）。函数把 NULL 映射为
    /// 0.5（v0.46 回退值）。
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
        // 一个已结算 market 含两条 snapshot（旧的 + 新的）。
        // 新的那条胜出。
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
        // 新的 snapshot（0.75）胜出,
        // 而不是旧的 0.5。
        assert_eq!(rows[0].1, Some(0.75));
    }
}