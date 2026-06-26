//! L3 — 自然语言查询（P1-2）。
//!
//! 将自然语言搜索字符串转换为对 `markets` 表的安全 SQL SELECT，
//! 再由命令层执行。仅生成 SELECT —— 绝不产生 INSERT/UPDATE/DELETE。
//!
//! 规范：P1-2 NL Query

use serde::{Deserialize, Serialize};
use specta::Type;

/// NL 查询返回的一行 —— 镜像生成的 SQL 从 `markets` 表中选取的列。
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct NlQueryRow {
    pub market_id: String,
    pub question: String,
    pub yes_price: Option<f64>,
    pub model_prob: Option<f64>,
    pub edge: Option<f64>,
    pub category: String,
}

/// NL 查询的聚合结果：生成的 SQL、查询行，以及对查询解读方式的人类可读说明。
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct NlQueryResult {
    pub sql: String,
    pub results: Vec<NlQueryRow>,
    pub explanation: String,
}

/// 基于自然语言查询字符串，构建对 `markets` 表的安全 SQL SELECT。
/// 返回 `(sql, explanation)`，其中 `explanation` 是对查询解读方式的人类可读描述。
///
/// 识别的关键字：
///   - `"premier league"`、`"football"`、`"soccer"` → category = 'football'（足球类）
///   - `"+ev"` / `"+edge"` → 过滤正 edge（yes_price < 0.5）
///   - `"today"` → closes_at 在未来 24h 内
///   - 任何其他裸词 → `question LIKE '%word%'`
///
/// 仅生成 SELECT。结果始终限定为 active 且未 resolve 的市场。
pub fn build_sql_from_keywords(query: &str) -> (String, String) {
    let lower = query.to_lowercase();
    let mut conditions: Vec<String> = vec!["active = 1".into(), "resolved = 0".into()];
    let mut notes: Vec<String> = Vec::new();

    // 类别检测。
    if lower.contains("premier league")
        || lower.contains("football")
        || lower.contains("soccer")
    {
        conditions.push("category = 'football'".into());
        notes.push("category=football".into());
    }
    if lower.contains("nba") || lower.contains("basketball") {
        conditions.push("category = 'basketball'".into());
        notes.push("category=basketball".into());
    }
    if lower.contains("election") || lower.contains("politics") {
        conditions.push("category = 'politics'".into());
        notes.push("category=politics".into());
    }

    // +EV 过滤:正期望值（便宜的 YES 代币）。
    if lower.contains("+ev") || lower.contains("+edge") || lower.contains("positive ev") {
        conditions.push("yes_price IS NOT NULL AND yes_price < 0.5".into());
        notes.push("positive-EV (yes_price<0.5)".into());
    }

    // Today: 24h 内收市。
    if lower.contains("today") {
        conditions.push(
            "closes_at IS NOT NULL AND closes_at <= (unixepoch() * 1000 + 86400000)".into(),
        );
        notes.push("closes within 24h".into());
    }

    // 对 question 列进行裸词 LIKE 搜索。先剥离上述已知关键字短语,
    // 避免产生冗余的 LIKE 子句。
    let stripped = lower
        .replace("premier league", "")
        .replace("football", "")
        .replace("soccer", "")
        .replace("basketball", "")
        .replace("nba", "")
        .replace("election", "")
        .replace("politics", "")
        .replace("+ev", "")
        .replace("+edge", "")
        .replace("positive ev", "")
        .replace("today", "");
    let words: Vec<&str> = stripped
        .split_whitespace()
        .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()))
        .filter(|w| !w.is_empty() && w.len() > 2)
        .collect();
    for w in words {
        let escaped = w.replace('\'', "''");
        conditions.push(format!("LOWER(question) LIKE '%{}%'", escaped));
        notes.push(format!("question~{}", w));
    }

    let where_clause = conditions.join(" AND ");
    let explanation = if notes.is_empty() {
        format!("Showing all active markets (no specific filters detected in \"{}\").", query)
    } else {
        format!("Interpreted \"{}\" as: {}.", query, notes.join(", "))
    };
    let sql = format!(
        "SELECT id AS market_id, question, yes_price, NULL AS model_prob, NULL AS edge, category \
         FROM markets WHERE {} ORDER BY created_at DESC LIMIT 100",
        where_clause
    );
    (sql, explanation)
}

/// 校验一个 SQL 字符串是否为安全的只读 SELECT。
///
/// 仅在（trim 后小写）语句以 `select` 开头且不包含任何写关键字
///（`insert`、`update`、`delete`、`drop`、`alter`、`create`、`replace`（写操作关键字）、
/// `attach`、`detach`、`pragma`）时返回 `true`。
pub fn is_safe_sql(sql: &str) -> bool {
    let lower = sql.trim().to_lowercase();
    if !lower.starts_with("select") {
        return false;
    }
    const FORBIDDEN: &[&str] = &[
        "insert",
        "update",
        "delete",
        "drop",
        "alter",
        "create",
        "replace",
        "attach",
        "detach",
        "pragma",
        "vacuum",
        "reindex",
    ];
    for kw in FORBIDDEN {
        if lower.contains(kw) {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_select_only() {
        let (sql, _) = build_sql_from_keywords("premier league today");
        assert!(sql.to_lowercase().starts_with("select"));
        assert!(sql.contains("category = 'football'"));
        assert!(sql.contains("closes_at"));
    }

    #[test]
    fn positive_ev_filter() {
        let (sql, _) = build_sql_from_keywords("+ev football");
        assert!(sql.contains("yes_price < 0.5"));
    }

    #[test]
    fn explanation_contains_notes() {
        let (_, explanation) = build_sql_from_keywords("premier league +ev");
        assert!(explanation.contains("category=football"));
        assert!(explanation.contains("positive-EV"));
    }

    #[test]
    fn safe_select_passes() {
        assert!(is_safe_sql("SELECT * FROM markets"));
    }

    #[test]
    fn unsafe_delete_rejected() {
        assert!(!is_safe_sql("DELETE FROM markets"));
    }

    #[test]
    fn unsafe_drop_rejected() {
        assert!(!is_safe_sql("SELECT 1; DROP TABLE markets"));
    }
}
