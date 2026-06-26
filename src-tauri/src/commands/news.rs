//! L2 —— 新闻关联命令（P1-1）。
//!
//! IPC:market_news（指定市场的新闻）和 list_all_news（全部新闻条目，按时间倒序）。

use crate::AppResult;
use crate::domain::news_correlator::NewsItem;
use crate::infra::state::AppState;
use serde::{Deserialize, Serialize};
use specta::Type;
use sqlx::FromRow;
use tauri::State;

/// 新闻条目 DTO —— 镜像 `news_items` SQLite 表。
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, Type)]
pub struct NewsItemDto {
    pub id: i64,
    pub title: String,
    pub source: String,
    pub url: String,
    pub published_at: i64,
    pub market_id: Option<String>,
    pub relevance_score: Option<f64>,
    pub impact_direction: Option<String>,
    pub summary: Option<String>,
}

impl From<NewsItemDto> for NewsItem {
    fn from(dto: NewsItemDto) -> Self {
        NewsItem {
            id: dto.id,
            title: dto.title,
            source: dto.source,
            url: dto.url,
            published_at: dto.published_at,
            market_id: dto.market_id,
            relevance_score: dto.relevance_score,
            impact_direction: dto.impact_direction,
            summary: dto.summary,
        }
    }
}

/// IPC:market_news —— 获取与指定市场关联的新闻条目。
#[tauri::command]
pub async fn market_news(
    state: State<'_, AppState>,
    market_id: String,
) -> AppResult<Vec<NewsItemDto>> {
    let rows = sqlx::query_as::<_, NewsItemDto>(
        "SELECT id, title, source, url, published_at, market_id, relevance_score,
                impact_direction, summary
         FROM news_items
         WHERE market_id = ?
         ORDER BY published_at DESC",
    )
    .bind(market_id)
    .fetch_all(&state.db)
    .await?;
    Ok(rows)
}

/// IPC:list_all_news —— 获取全部新闻条目，按时间倒序。
#[tauri::command]
pub async fn list_all_news(
    state: State<'_, AppState>,
    limit: Option<i64>,
) -> AppResult<Vec<NewsItemDto>> {
    let limit = limit.unwrap_or(100);
    let rows = sqlx::query_as::<_, NewsItemDto>(
        "SELECT id, title, source, url, published_at, market_id, relevance_score,
                impact_direction, summary
         FROM news_items
         ORDER BY published_at DESC
         LIMIT ?",
    )
    .bind(limit)
    .fetch_all(&state.db)
    .await?;
    Ok(rows)
}
