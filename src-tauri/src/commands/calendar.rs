//! L2 —— 市场日历（P0-2）。
//!
//! IPC:`market_calendar` —— 查询 `closes_at` 落在指定
//! （年,月）内的足球市场,将其映射为 `MarketRow`,
//! 并把分组逻辑委托给 `domain::calendar::group_by_date`。

use crate::AppResult;
use crate::domain::calendar::{CalendarDay, CalendarFixture, MarketRow, group_by_date};
use crate::infra::state::AppState;
use serde::{Deserialize, Serialize};
use specta::Type;
use sqlx::FromRow;
use tauri::State;

/// 日历单日单个赛事的 wire DTO。
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CalendarFixtureDto {
    pub market_id: String,
    pub home: String,
    pub away: String,
    pub time: String,
    pub edge: Option<f64>,
    pub competition: Option<String>,
}

impl From<CalendarFixture> for CalendarFixtureDto {
    fn from(f: CalendarFixture) -> Self {
        Self {
            market_id: f.market_id,
            home: f.home,
            away: f.away,
            time: f.time,
            edge: f.edge,
            competition: f.competition,
        }
    }
}

/// 日历单日及其赛事的 wire DTO。
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CalendarDayDto {
    pub date: String,
    pub fixtures: Vec<CalendarFixtureDto>,
}

impl From<CalendarDay> for CalendarDayDto {
    fn from(d: CalendarDay) -> Self {
        Self {
            date: d.date,
            fixtures: d.fixtures.into_iter().map(Into::into).collect(),
        }
    }
}

/// 直接来自 `markets` 表的原始行。
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
struct RawMarketRow {
    id: String,
    question: String,
    closes_at: i64,
    yes_price: Option<f64>,
}

/// IPC:`market_calendar` —— 返回指定年+月内会关闭的
/// 足球市场,按其关闭所在的日历日分组。
///
/// `year` / `month` 按 UTC 解释。我们计算 unix-ms 区间
/// [start_of_month, start_of_next_month),将 `closes_at`
/// 过滤到该区间。`closes_at` 为 NULL 的市场会被排除。
#[tauri::command]
pub async fn market_calendar(
    state: State<'_, AppState>,
    year: i32,
    month: u32,
) -> AppResult<Vec<CalendarDayDto>> {
    // 计算请求月份（UTC）的 unix-ms 边界。
    let start_ms = month_start_ms(year, month);
    let end_ms = month_start_ms(year, if month == 12 { 1 } else { month + 1 });

    let rows: Vec<RawMarketRow> = sqlx::query_as::<_, RawMarketRow>(
        "SELECT id, question, closes_at, yes_price
         FROM markets
         WHERE category = 'football'
           AND closes_at IS NOT NULL
           AND closes_at >= ?
           AND closes_at < ?
         ORDER BY closes_at ASC",
    )
    .bind(start_ms)
    .bind(end_ms)
    .fetch_all(&state.db)
    .await?;

    // 将原始 DB 行映射为 domain 中的 MarketRow。
    // `competition` 列在 markets 表中并不存在
    // （在更完整的实现里由 tags 派生）,此处先传 None。
    let market_rows: Vec<MarketRow> = rows
        .iter()
        .map(|r| MarketRow {
            market_id: r.id.clone(),
            question: r.question.clone(),
            closes_at: r.closes_at,
            yes_price: r.yes_price,
            competition: None,
        })
        .collect();

    // 把分组 + 球队名解析委托给 domain 纯函数。
    let days = group_by_date(&market_rows);

    Ok(days.into_iter().map(Into::into).collect())
}

/// 计算给定（年,月）第一天的 unix-ms 时间戳（00:00:00 UTC）。
fn month_start_ms(year: i32, month: u32) -> i64 {
    use chrono::{TimeZone, Utc};
    let m = month.clamp(1, 12) as u32;
    let d = chrono::NaiveDate::from_ymd_opt(year, m, 1)
        .unwrap_or_else(|| chrono::NaiveDate::from_ymd_opt(1970, 1, 1).unwrap());
    Utc.from_utc_datetime(&d.and_hms_opt(0, 0, 0).unwrap())
        .timestamp_millis()
}
