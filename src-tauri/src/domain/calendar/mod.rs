//! L3 — 市场日历（P0-2）。
//!
//! 按关盘所在的日历日对足球市场进行分组，
//! 生成 `Vec<CalendarDay>` 供前端以月视图网格展示。
//! 该分组是基于 `MarketRow` 切片的**纯函数** ——
//! 所有数据库访问都位于 L2 命令层（`commands::calendar`）。

use serde::{Deserialize, Serialize};
use specta::Type;

/// `group_by_date` 消费的扁平化市场行。命令层将
/// `sqlx` 行映射为该结构，使领域层与数据库无关。
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct MarketRow {
    pub market_id: String,
    pub question: String,
    pub closes_at: i64,       // unix 毫秒
    pub yes_price: Option<f64>,
    pub competition: Option<String>,
}

/// 单个日历日上的一场赛事。
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CalendarFixture {
    pub market_id: String,
    pub home: String,
    pub away: String,
    pub time: String,
    pub edge: Option<f64>,
    pub competition: Option<String>,
}

/// 单个日历日及其赛事。
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CalendarDay {
    pub date: String, // ISO 日期：YYYY-MM-DD
    pub fixtures: Vec<CalendarFixture>,
}

/// 按 `closes_at` 日期（ISO `YYYY-MM-DD`）对市场分组。
///
/// `closes_at <= 0` 的市场会被跳过（无有效关盘时间）。
/// 返回的日期按升序排列。
pub fn group_by_date(markets: &[MarketRow]) -> Vec<CalendarDay> {
    let mut by_date: std::collections::BTreeMap<String, Vec<CalendarFixture>> =
        std::collections::BTreeMap::new();

    for m in markets {
        if m.closes_at <= 0 {
            continue;
        }
        let date = iso_date_from_ms(m.closes_at);
        let (home, away) = parse_teams(&m.question);
        let time = iso_time_from_ms(m.closes_at);
        let edge = m.yes_price.map(|p| (p - 0.5) * 100.0);
        by_date.entry(date).or_default().push(CalendarFixture {
            market_id: m.market_id.clone(),
            home,
            away,
            time,
            edge,
            competition: m.competition.clone(),
        });
    }

    by_date
        .into_iter()
        .map(|(date, mut fixtures)| {
            fixtures.sort_by(|a, b| a.time.cmp(&b.time));
            CalendarDay { date, fixtures }
        })
        .collect()
}

/// 从市场问题中解析出 "Team A vs Team B"（或 "v"、"vs."、"-"）。
/// 未找到分隔符时回退为 ("Unknown", "Unknown")。
pub fn parse_teams(question: &str) -> (String, String) {
    let lower = question.to_lowercase();
    for sep in &[" vs. ", " vs ", " v. ", " v ", " - "] {
        if let Some(idx) = lower.find(sep) {
            let home = question[..idx].trim().to_string();
            let away = question[idx + sep.len()..].trim().to_string();
            if !home.is_empty() && !away.is_empty() {
                return (home, away);
            }
        }
    }
    ("Unknown".to_string(), "Unknown".to_string())
}

/// 将 unix 毫秒时间戳转换为 UTC 日历的 ISO 日期 `YYYY-MM-DD`。
/// 纯函数（调用方不依赖 `chrono` —— 手动完成除法，使
/// 领域层保持轻量）。
fn iso_date_from_ms(ms: i64) -> String {
    let secs = (ms / 1000).max(0);
    let days = secs / 86_400;
    // 民用日转换算法（Howard Hinnant）。自 1970-01-01 起的天数。
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64; // [0, 146096]
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    let y = if m <= 2 { y + 1 } else { y };
    format!("{:04}-{:02}-{:02}", y, m, d)
}

/// 将 unix 毫秒时间戳转换为 `HH:MM`（UTC）。
fn iso_time_from_ms(ms: i64) -> String {
    let secs = (ms / 1000).rem_euclid(86_400);
    let h = secs / 3_600;
    let min = (secs % 3_600) / 60;
    format!("{:02}:{:02}", h, min)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: &str, q: &str, closes: i64) -> MarketRow {
        MarketRow {
            market_id: id.into(),
            question: q.into(),
            closes_at: closes,
            yes_price: Some(0.6),
            competition: Some("EPL".into()),
        }
    }

    #[test]
    fn groups_by_date() {
        // 2023-06-15 00:00:00 UTC = 1686787200000 毫秒
        // 2023-06-15 12:00:00 UTC = 1686830400000 毫秒
        // 2023-06-16 00:00:00 UTC = 1686873600000 毫秒
        let markets = vec![
            row("a", "Arsenal vs Chelsea", 1_686_787_200_000),
            row("b", "Liverpool v Man City", 1_686_830_400_000),
            row("c", "Barcelona vs Real Madrid", 1_686_873_600_000),
        ];
        let days = group_by_date(&markets);
        assert_eq!(days.len(), 2);
        assert_eq!(days[0].date, "2023-06-15");
        assert_eq!(days[0].fixtures.len(), 2);
        assert_eq!(days[1].date, "2023-06-16");
        assert_eq!(days[1].fixtures.len(), 1);
    }

    #[test]
    fn skips_invalid_closes_at() {
        let markets = vec![row("a", "X vs Y", 0), row("b", "X vs Y", -1)];
        let days = group_by_date(&markets);
        assert!(days.is_empty());
    }

    #[test]
    fn parses_teams_vs() {
        let (h, a) = parse_teams("Arsenal vs Chelsea");
        assert_eq!(h, "Arsenal");
        assert_eq!(a, "Chelsea");
    }

    #[test]
    fn parses_teams_v() {
        let (h, a) = parse_teams("Liverpool v Man City");
        assert_eq!(h, "Liverpool");
        assert_eq!(a, "Man City");
    }

    #[test]
    fn parse_teams_fallback() {
        let (h, a) = parse_teams("Will it rain?");
        assert_eq!(h, "Unknown");
        assert_eq!(a, "Unknown");
    }

    #[test]
    fn fixtures_sorted_by_time() {
        let markets = vec![
            row("a", "A vs B", 1_686_830_400_000), // 12:00
            row("b", "C vs D", 1_686_787_200_000), // 00:00
        ];
        let days = group_by_date(&markets);
        // 都在 2023-06-15
        assert_eq!(days[0].fixtures[0].market_id, "b"); // 00:00 在前
        assert_eq!(days[0].fixtures[1].market_id, "a"); // 12:00 在后
    }
}
