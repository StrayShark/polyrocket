//! L3 —— 新闻关联器（P1-1）。
//!
//! 将新闻条目与预测市场关联，并对其可能的影响方向进行分类。
//! 仅含纯函数 —— 数据库持久化逻辑在 `commands::news` 中。
//!
//! 规范：P1-1 News Correlator

use serde::{Deserialize, Serialize};
use specta::Type;

/// 一条新闻条目行 —— 镜像 `news_items` SQLite 表。
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct NewsItem {
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

/// 通过关键词重叠将新闻标题与市场问题关联。
///
/// 返回 `[0.0, 1.0]` 范围内的相关性评分。该评分是
/// 共享有效词与所有有效词并集之间的类 Jaccard 比率。
/// 停用词（"the"、"a"、"will"……）会被剔除，避免抬高重叠度。
pub fn correlate_news_to_market(news_title: &str, market_question: &str) -> f64 {
    let news_words = tokenize(news_title);
    let market_words = tokenize(market_question);

    if news_words.is_empty() || market_words.is_empty() {
        return 0.0;
    }

    let mut shared = 0usize;
    for w in &news_words {
        if market_words.contains(w) {
            shared += 1;
        }
    }

    // 并集 = |新闻| + |市场| - |共享|
    let union = news_words.len() + market_words.len() - shared;
    if union == 0 {
        return 0.0;
    }
    (shared as f64) / (union as f64)
}

/// 对新闻头条可能的市场影响进行分类。
///
/// 基于关键词的启发式方法：扫描看涨 / 看跌触发词，
/// 并返回 "up"、"down" 或 "neutral"。未找到方向性关键词时
/// 返回 "neutral"。
pub fn classify_impact(news_title: &str) -> String {
    let lower = news_title.to_lowercase();

    const BULLISH: &[&str] = &[
        "win", "wins", "winner", "victory", "beat", "beats", "defeat",
        "champion", "champions", "lead", "leads", "ahead", "rise", "rises",
        "rising", "surge", "surges", "boost", "boosts", "gain", "gains",
        "up", "bullish", "rally", "recover", "recovers", "signing", "signs",
        "extend", "extends", "renew", "renews",
    ];
    const BEARISH: &[&str] = &[
        "lose", "loses", "loss", "lost", "defeat", "defeated", "injury",
        "injured", "out", "down", "fall", "falls", "falling", "drop",
        "drops", "crash", "crashes", "bearish", "decline", "declines",
        "miss", "misses", "suspend", "suspended", "fire", "fires", "sack",
        "sacked", "cancel", "cancelled", "delay", "delayed",
    ];

    let mut bull = 0;
    let mut bear = 0;
    for word in lower.split_whitespace() {
        let w = word.trim_matches(|c: char| !c.is_alphanumeric());
        if BULLISH.contains(&w) {
            bull += 1;
        }
        if BEARISH.contains(&w) {
            bear += 1;
        }
    }

    if bull > bear {
        "up".to_string()
    } else if bear > bull {
        "down".to_string()
    } else {
        "neutral".to_string()
    }
}

/// 将文本拆分为小写化的有效词，并剔除停用词。
fn tokenize(s: &str) -> Vec<String> {
    const STOP: &[&str] = &[
        "the", "a", "an", "and", "or", "but", "in", "on", "at", "to", "for",
        "of", "is", "are", "will", "be", "has", "have", "had", "this",
        "that", "with", "from", "by", "as", "it", "its", "was", "were",
        "can", "could", "would", "should", "may", "might", "do", "does",
        "did", "not", "no", "yes", "if", "then", "than", "so", "such",
        "about", "into", "over", "under", "again", "more", "most", "some",
        "any", "all", "each", "other", "what", "which", "who", "whom",
    ];

    s.to_lowercase()
        .split_whitespace()
        .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).to_string())
        .filter(|w| !w.is_empty() && w.len() > 1 && !STOP.contains(&w.as_str()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn correlate_exact_overlap() {
        let score = correlate_news_to_market("Liverpool win Premier League", "Will Liverpool win the Premier League?");
        assert!(score > 0.5);
    }

    #[test]
    fn correlate_no_overlap() {
        let score = correlate_news_to_market("Stock market crashes", "Will Liverpool win the Premier League?");
        assert_eq!(score, 0.0);
    }

    #[test]
    fn classify_bullish() {
        assert_eq!(classify_impact("Liverpool beat Manchester United"), "up");
    }

    #[test]
    fn classify_bearish() {
        assert_eq!(classify_impact("Star striker injured before match"), "down");
    }

    #[test]
    fn classify_neutral() {
        assert_eq!(classify_impact("Match scheduled for Saturday"), "neutral");
    }
}
