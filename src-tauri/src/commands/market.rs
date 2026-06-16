use crate::AppResult;
use crate::polymarket;
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