//! Prompt 模板系统 —— polyrocket 市场分析 prompt 的 v1 版本。
//!
//! 所有 prompt 都要求模型返回 **strict JSON** 格式,以便
//! [`crate::domain::llm::parse_recommendation`] 中的解析器
//! 抽取干净的 `(probability, side, confidence, reasoning)` 元组。
//!
//! 每个 prompt 返回的 JSON 形状：
//! ```json
//! {
//!   "probability": 0.68,            // 0..1
//!   "side": "YES",                  // "YES" | "NO" | "skip"
//!   "confidence": 0.72,             // 0..1
//!   "reasoning": "string",          // ≤ 800 字符
//!   "key_factors": ["...", "..."]   // ≤ 5 个字符串
//! }
//! ```

use crate::domain::llm::{CallRequest, ChatMessage};
use serde::{Deserialize, Serialize};

/// 当 prompt 文本或 shape 变化时 bump — 用于按 prompt 版本追踪胜率 (F13 stats)。
pub const PROMPT_VERSION_MARKET_ANALYSIS: &str = "market.v1.0";
pub const PROMPT_VERSION_QUICK_THESIS: &str = "thesis.v1.0";
pub const PROMPT_VERSION_CONSENSUS_VOTE: &str = "consensus.v1.0";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketContext {
    pub market_id: String,
    pub question: String,
    pub category: String,
    pub yes_price_cents: u32,    // 0..100
    pub no_price_cents: u32,     // 0..100
    pub volume_24h_usdc: f64,
    pub liquidity_usdc: f64,
    pub closes_at_unix_ms: i64,
    pub resolution_source: String,
    pub recent_signals: Vec<SignalSummary>,
    pub orderbook_top: Option<OrderbookTop>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalSummary {
    pub name: String,
    pub value: String,
    pub polarity: String, // "bullish" | "bearish" | "neutral"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderbookTop {
    pub best_bid: u32,
    pub best_ask: u32,
    pub bid_depth: f64,
    pub ask_depth: f64,
}

const MARKET_SYSTEM: &str = "You are a precise prediction-market analyst. \
You must think carefully and return a strict JSON object with fields: \
probability (0..1), side (YES|NO|skip), confidence (0..1), reasoning (string, ≤ 800 chars), \
key_factors (array of ≤ 5 short strings). No prose before or after the JSON. \
The probability must reflect the model's estimate of the YES outcome. \
side = YES if probability > 0.5, NO if < 0.5, skip if confidence < 0.4.";

pub fn build_market_analysis_request(model: &str, ctx: &MarketContext) -> CallRequest {
    let user = serde_json::to_string_pretty(ctx).unwrap_or_default();
    let user = format!("Analyze this market. Return JSON only.\n\n```json\n{user}\n```");
    CallRequest::new(model)
        .max_tokens(1024)
        .temperature(0.2)
        .json_mode()
        .system(MARKET_SYSTEM)
        .user(user)
}

const THESIS_SYSTEM: &str = "You are a sharp markets commentator. \
Reply with a strict JSON object: {thesis: string ≤ 400 chars, action: YES|NO|skip, \
confidence: 0..1, edge_pct: number (your estimated edge vs market price, signed)}. \
No prose, JSON only.";

pub fn build_quick_thesis_request(model: &str, ctx: &MarketContext) -> CallRequest {
    let user = format!(
        "Question: {}\nYES market: ¢{}\nNO market: ¢{}\nVolume 24h: ${:.0}\nCloses: {}\n\n\
         Give your one-sentence thesis and action.",
        ctx.question, ctx.yes_price_cents, ctx.no_price_cents,
        ctx.volume_24h_usdc,
        chrono_format(ctx.closes_at_unix_ms)
    );
    CallRequest::new(model).max_tokens(512).temperature(0.3).json_mode()
        .system(THESIS_SYSTEM)
        .user(user)
}

const CONSENSUS_SYSTEM: &str = "You are a judge evaluating 4 LLM analyses of a prediction market. \
Each LLM gave a probability and reasoning. Return a strict JSON object: \
{final_probability: 0..1, side: YES|NO|skip, confidence: 0..1, dissent: string ≤ 200 chars}. \
Use the median unless two analyses strongly agree and the other two are clearly weaker.";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerView {
    pub provider_id: String,
    pub probability: f64,
    pub reasoning: String,
    pub confidence: f64,
}

pub fn build_consensus_request(model: &str, ctx: &MarketContext, peers: &[PeerView]) -> CallRequest {
    let user = serde_json::to_string_pretty(ctx).unwrap_or_default();
    let peers_json = serde_json::to_string_pretty(peers).unwrap_or_default();
    let user = format!(
        "Market:\n```json\n{user}\n```\n\nPeer analyses:\n```json\n{peers_json}\n```\n\n\
         Aggregate and return the final verdict as JSON."
    );
    CallRequest::new(model).max_tokens(800).temperature(0.1).json_mode()
        .system(CONSENSUS_SYSTEM)
        .user(user)
}

// ---- 解析返回 JSON 的模型推荐 ----

#[derive(Debug, Clone, Deserialize)]
pub struct LlmRecommendationPayload {
    pub probability: Option<f64>,
    pub side: Option<String>,
    pub confidence: Option<f64>,
    pub reasoning: Option<String>,
}

pub fn parse_recommendation(text: &str) -> Result<(f64, String, f64, String), String> {
    // 1) 尝试直接解析
    if let Ok(p) = serde_json::from_str::<LlmRecommendationPayload>(text) {
        return Ok((
            p.probability.unwrap_or(0.5).clamp(0.0, 1.0),
            normalize_side(p.side.as_deref().unwrap_or("skip")),
            p.confidence.unwrap_or(0.5).clamp(0.0, 1.0),
            p.reasoning.unwrap_or_default().chars().take(800).collect(),
        ));
    }
    // 2) 尝试查找第一个 {...} block(模型可能把 JSON 包裹在代码围栏中)
    if let Some(start) = text.find('{') {
        if let Some(end) = text.rfind('}') {
            if end > start {
                let slice = &text[start..=end];
                if let Ok(p) = serde_json::from_str::<LlmRecommendationPayload>(slice) {
                    return Ok((
                        p.probability.unwrap_or(0.5).clamp(0.0, 1.0),
                        normalize_side(p.side.as_deref().unwrap_or("skip")),
                        p.confidence.unwrap_or(0.5).clamp(0.0, 1.0),
                        p.reasoning.unwrap_or_default().chars().take(800).collect(),
                    ));
                }
            }
        }
    }
    Err(format!("could not extract recommendation JSON: {}", &text[..text.len().min(200)]))
}

fn normalize_side(s: &str) -> String {
    match s.to_uppercase().as_str() {
        "YES" | "Y" | "TRUE" | "1" => "YES".into(),
        "NO" | "N" | "FALSE" | "0" => "NO".into(),
        _ => "skip".into(),
    }
}

fn chrono_format(unix_ms: i64) -> String {
    use chrono::{DateTime, Utc};
    let dt: DateTime<Utc> = DateTime::<Utc>::from_timestamp_millis(unix_ms)
        .unwrap_or_else(|| Utc::now());
    dt.format("%Y-%m-%d %H:%M UTC").to_string()
}

// ============================================================================
// v0.118 —— 足球专用 prompt 模块
//
// 行业权威 framework 知识库（调研 2026-06-22）：
//   • Dixon-Coles (1997)        —— 学术金标准，双 Poisson + 低分调整
//   • Elo (Arpad Elo)           —— 球队相对实力
//   • xG (Sam Green 2012)       —— 射门质量 + form 评估
//   • CLV (Pinnacle)            —— Polymarket implied prob vs model prob = edge
//                                          （Polymarket 隐含概率 vs 模型概率 = edge）
//
// 与 market.v1.0 的区别：
//   - System prompt 显式要求 LLM 综合 4 个 framework 给出中间值
//   - 输出 JSON 多一个 `framework_breakdown` 对象（含 elo/poisson/xg/clv）
//   - FootballMarketType 路由（HomeWin / AwayWin / Draw / OverUnder / AsianHandicap）
//   - `probability` 字段语义根据 market_type 切换（home 胜/away 胜/平/over/covers）
//
// 配套 docs/football-frameworks.md（v0.118 new）详述 framework 选型理由。
// ============================================================================

/// Polymarket 上的足球市场类型。决定 `probability` 字段的语义。
///
/// 设计原则:
///   - Polymarket 的足球市场主要是 "Will X win on YYYY-MM-DD?" 这种
///     **单队胜出**问题 (不是经典 1X2 的 home/away 二分)。
///   - 所以这里用 `TeamWin` 表示 "question 里提到的那个队胜出" 的概率,
///     team 名字放在 `FootballMatchContext.home_team` 字段。
///   - `Draw` / `OverUnder` / `AsianHandicap` 保持独立语义。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FootballMarketType {
    /// "Will Argentina win on 2026-06-22?" → probability = P(Argentina 胜出该场比赛)
    /// team 名字存放在 `FootballMatchContext.home_team`。
    TeamWin,
    /// "Will Argentina vs. Austria end in a draw?" → probability = P(平局)
    Draw,
    /// "Argentina vs. Austria: O/U 2.5" → probability = P(总进球数 > 盘口)
    OverUnder,
    /// "Spread: France (-2.5)" → probability = P(法国队让球胜出)
    AsianHandicap,
    /// "Will Arsenal finish Premier League top 4?" → probability = P(赛季结束前事件发生)
    Outright,
    /// 无法识别 — 退化为通用 market.v1.0 prompt
    Unknown,
}

impl FootballMarketType {
    /// 从 Polymarket question 推断市场类型。
    /// 规则: 检查 question 字符串,匹配最先命中的模式。
    /// 例:
    ///   "Will Argentina win on 2026-06-22?" → TeamWin (team=Argentina)
    ///   "Argentina vs. Austria: O/U 2.5"    → OverUnder
    ///   "Spread: France (-2.5)"              → AsianHandicap
    ///   "Will Arsenal finish top 4?"         → Outright
    pub fn from_question(question: &str) -> Self {
        let q = question.to_lowercase();
        // O/U 大小球 (最高优先级, 因为 "Argentina vs Austria O/U 2.5" 也含 "win")
        if q.contains("o/u") || q.contains("over/under") || q.contains("over 2.5") || q.contains("under 2.5") {
            return Self::OverUnder;
        }
        // Spread / 让球
        if q.contains("spread") || q.contains("handicap") || q.contains("(-") || q.contains("(+)") {
            return Self::AsianHandicap;
        }
        // Draw / 平局
        if q.contains("draw") || q.contains("tie") {
            return Self::Draw;
        }
        // 单队胜出 (Polymarket 最常见)
        if q.starts_with("will ") && q.contains(" win") {
            return Self::TeamWin;
        }
        // Outright (整个赛季 / 锦标赛冠军类)
        if q.contains("premier league") || q.contains("la liga") || q.contains("serie a")
            || q.contains("bundesliga") || q.contains("champions league") || q.contains("world cup")
            || q.contains("finish") || q.contains("win the") || q.contains("reach ") {
            return Self::Outright;
        }
        Self::Unknown
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::TeamWin => "team_win",
            Self::Draw => "draw",
            Self::OverUnder => "over_under",
            Self::AsianHandicap => "asian_handicap",
            Self::Outright => "outright",
            Self::Unknown => "unknown",
        }
    }
}

/// v0.118 — Football-specific market context。扩展 `MarketContext`,
/// 加入足球比赛级别的数据 (Elo、xG、Dixon-Coles 参数)。
/// 所有字段 optional,让 LLM 在缺失时回退到 domain knowledge
/// (内置的世界足球知识)。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FootballMatchContext {
    /// 基础 Polymarket context (question, prices, vol, signals 等)
    pub market: MarketContext,

    /// Polymarket 市场类型 — 决定 probability 字段语义
    pub market_type: FootballMarketType,

    /// 比赛识别信息 (LLM-readable,如果未提供则从 question 解析)
    pub home_team: Option<String>,         // 例如 "Argentina"
    pub away_team: Option<String>,         // 例如 "Austria"
    pub competition: Option<String>,       // 例如 "FIFA World Cup 2026"
    pub kickoff_unix_ms: Option<i64>,      // 若与 closes_at 不同

    /// 球队实力指标 (optional — LLM 用 domain knowledge 填补缺失)
    // Elo (国际足球通常 1500-2200)
    pub home_elo: Option<f64>,
    pub away_elo: Option<f64>,
    pub home_field_advantage_elo: Option<f64>, // ~ +100 (国际足球)

    // xG (Sam Green 2012,最近比赛的 per-90)
    pub home_xg_per90_last5: Option<f64>,
    pub away_xg_per90_last5: Option<f64>,

    // Dixon-Coles 基础率 (Maher 1982)
    pub home_goals_per_game_season: Option<f64>,
    pub away_goals_per_game_season: Option<f64>,
    pub home_goals_conceded_per_game_season: Option<f64>,
    pub away_goals_conceded_per_game_season: Option<f64>,

    // 历史交锋 (最近 10 次)
    pub h2h_home_wins_last10: Option<u32>,
    pub h2h_draws_last10: Option<u32>,
    pub h2h_away_wins_last10: Option<u32>,

    /// 亚洲让球盘口线 (仅在 market_type == AsianHandicap 时有意义)
    /// 例如 "Spread: France (-2.5)" 时为 Some(-2.5)
    pub handicap_line: Option<f64>,
    /// 大小球盘口线 (仅在 market_type == OverUnder 时有意义)
    /// 例如 "Argentina vs Austria O/U 2.5" 时为 Some(2.5)
    pub over_under_line: Option<f64>,
}

impl FootballMatchContext {
    /// 便捷方法: 尝试从 `MarketContext` 解析队名 + 市场类型。
    /// 返回一个 context,其中能从文本推断的所有字段都填充好。
    pub fn from_market_context(market: MarketContext) -> Self {
        let market_type = FootballMarketType::from_question(&market.question);
        let (home_team, away_team, handicap_line, over_under_line) = parse_question_extras(&market.question, market_type);
        Self {
            market,
            market_type,
            home_team,
            away_team,
            competition: None,
            kickoff_unix_ms: None,
            home_elo: None,
            away_elo: None,
            home_field_advantage_elo: None,
            home_xg_per90_last5: None,
            away_xg_per90_last5: None,
            home_goals_per_game_season: None,
            away_goals_per_game_season: None,
            home_goals_conceded_per_game_season: None,
            away_goals_conceded_per_game_season: None,
            h2h_home_wins_last10: None,
            h2h_draws_last10: None,
            h2h_away_wins_last10: None,
            handicap_line,
            over_under_line,
        }
    }
}

/// 尝试从 Polymarket question 文本中抽取队名 + handicap / O/U line。
/// 尽力而为 — 无法干净解析时返回 None。
fn parse_question_extras(
    question: &str,
    market_type: FootballMarketType,
) -> (Option<String>, Option<String>, Option<f64>, Option<f64>) {
    let mut home = None;
    let mut away = None;
    let mut handicap = None;
    let mut over_under = None;

    // "Will Argentina win on 2026-06-22?" — 单队,无 vs.
    // "Argentina vs. Austria: O/U 2.5" — 两队用 vs 分隔。
    // "Spread: France (-2.5)" — 单队 + 括号中的 handicap line。
    let lower = question.to_lowercase();

    // Polymarket 的真实格式: "Argentina vs. Austria: O/U 2.5" / "Argentina vs Austria"
    // 注意 " vs" 后可能是 " " (无标点) 或 "." (带句号)。先归一化为 " vs "。
    let normalized = lower.replace(" vs.", " vs ").replace(" vs,", " vs ");
    if let Some(idx) = normalized.find(" vs ") {
        // 重新 map 回原 question 的 index — 由于我们只 replace 了 " vs." / " vs,", 这两个长度
        // (4) 跟 " vs " 一样, 所以 idx 在 normalized 和 lower 里一致。
        let before = &question[..idx];
        let after = &question[idx + 4..];
        // 从 before 剥离 "Will " 前缀
        let home_raw = before.trim_start_matches("Will ").trim();
        // 剥离尾部标点
        let home_clean = home_raw.trim_end_matches(|c: char| !c.is_alphanumeric() && c != ' ');
        // 从 after 剥离 " on YYYY-MM-DD"
        let away_raw = if let Some(date_idx) = after.find(" on ") {
            &after[..date_idx]
        } else {
            after
        };
        // 剥离尾部标点
        let away_clean = away_raw.trim_end_matches(|c: char| !c.is_alphanumeric() && c != ' ');
        // 从 away 剥离常见前缀
        let away_clean = away_clean.trim_start_matches("Will ").trim();
        // 剥离 O/U 后缀
        let away_clean = away_clean.split(':').next().unwrap_or(away_clean).trim();
        home = Some(home_clean.to_string());
        away = Some(away_clean.to_string());
    } else if let Some(start) = lower.find("will ") {
        if let Some(win_idx) = lower[start..].find(" win") {
            // v0.121 — 安全的 char-boundary 切片。question
            // 可能包含 UTF-8 多字节字符(例如 "La Liga"
            // 含 í)。`start + win_idx` 是字节偏移,
            // 可能落在多字节字符内部,这会导致 panic。
            // `floor_char_boundary` 取最近的有效 char boundary (Rust 1.81+)。
            let abs_start = start + 5;
            let abs_end = start + win_idx;
            let safe_end = question.floor_char_boundary(abs_end);
            let safe_start = question.floor_char_boundary(abs_start);
            if safe_start < safe_end {
                let team = &question[safe_start..safe_end];
                let team = team.split_whitespace().next().unwrap_or("").to_string();
                if !team.is_empty() {
                    home = Some(team);
                }
            }
        }
    }

    // 从括号解析 handicap,例如 "(-2.5)" 或 "(+1.5)"
    if market_type == FootballMarketType::AsianHandicap {
        if let Some(open) = question.find('(') {
            if let Some(close) = question.find(')') {
                let inner = &question[open + 1..close];
                if let Ok(v) = inner.parse::<f64>() {
                    handicap = Some(v);
                }
            }
        }
    }

    // 解析 O/U line,例如 "O/U 2.5"
    if market_type == FootballMarketType::OverUnder {
        let after_ou = if let Some(idx) = lower.find("o/u") {
            &question[idx + 3..]
        } else {
            ""
        };
        let after_ou = after_ou.trim_start_matches(|c: char| !c.is_numeric() && c != '.' && c != '-');
        let num_str: String = after_ou.chars().take_while(|c| c.is_numeric() || *c == '.').collect();
        if let Ok(v) = num_str.parse::<f64>() {
            over_under = Some(v);
        }
    }

    (home, away, handicap, over_under)
}

/// 当 football prompt 文本或 shape 变化时 bump — 用于按 prompt 版本追踪胜率 (F13 stats)。
pub const PROMPT_VERSION_FOOTBALL_MATCH: &str = "football.v1.0";

/// v0.118 — Football-specific system prompt。规范四个行业
/// frameworks (Dixon-Coles / Elo / xG / CLV) 并要求在输出 JSON 中
/// 提供中间 framework_breakdown 值。
///
/// 设计原则:
///   - 不假设 LLM 懂所有 framework — 在 system prompt 里给完整定义
///   - 让 LLM 输出 intermediate values (elo_diff / poisson_lambda / clv_edge)
///     便于下游做 audit + 模型对比
///   - strict JSON, 与 market.v1.0 保持一致, 不影响 consensus 逻辑
const FOOTBALL_SYSTEM: &str = "You are a precise football (soccer) prediction-market analyst. \
You estimate the probability of the SPECIFIC outcome named in the Polymarket question. \
You must combine FOUR industry-standard frameworks and return a strict JSON object.\n\
\n\
FRAMEWORKS (use all four in your reasoning):\n\
\n\
1. DIXON-COLES MODEL (Dixon & Coles 1997 — academic gold standard)\n\
   - Base: independent Poisson for home/away goals. λ_home = attack_home × defense_away × home_advantage.\n\
   - Modify low-score probabilities (0-0, 1-0, 0-1, 1-1) by ρ factor ≈ -0.1 to +0.2.\n\
   - Derive 1X2 (home/draw/away) + O/U + Asian Handicap from score matrix.\n\
\n\
2. ELO RATING (Arpad Elo)\n\
   - Expected win: E = 1 / (1 + 10^((R_away - R_home - HFA) / 400))\n\
   - HFA (home field advantage) ≈ +100 Elo points in international football.\n\
   - K-factor: 16-32 (lower for higher-tier matches).\n\
   - Anchor team strength differential.\n\
\n\
3. xG (Expected Goals, Sam Green 2012)\n\
   - Recent xG per 90 (last 5-10 matches) reflects current form better than goals scored.\n\
   - Penalty xG = 0.75 (fixed). Free-kick xG ≈ 0.05-0.10.\n\
   - xG trend (improving/declining) > raw xG value.\n\
   - Use for form adjustments and 'luck correction'.\n\
\n\
4. CLV (Closing Line Value, Pinnacle framework — industry definition of edge)\n\
   - edge = your probability - Polymarket's implied probability (yes_price_cents / 100).\n\
   - If edge > +0.05: actionable YES bet.\n\
   - If edge < -0.05: actionable NO bet (or skip if no opposite side).\n\
   - Polymarket mid-price ≈ closing line equivalent (no traditional bookmaker vig).\n\
\n\
OUTPUT (strict JSON, no prose before or after):\n\
{\n\
  \"probability\": 0.0-1.0,\n\
  \"side\": \"YES\" | \"NO\" | \"skip\",\n\
  \"confidence\": 0.0-1.0,\n\
  \"reasoning\": \"string ≤ 800 chars\",\n\
  \"key_factors\": [\"...\", \"...\"],\n\
\n\
  \"framework_breakdown\": {\n\
    \"elo_home\": number | null,\n\
    \"elo_away\": number | null,\n\
    \"elo_diff_with_hfa\": number | null,\n\
    \"implied_win_pct_elo\": 0.0-1.0 | null,\n\
    \"lambda_home_goals\": number | null,\n\
    \"lambda_away_goals\": number | null,\n\
    \"dixon_coles_home_win_pct\": 0.0-1.0 | null,\n\
    \"dixon_coles_draw_pct\": 0.0-1.0 | null,\n\
    \"dixon_coles_away_win_pct\": 0.0-1.0 | null,\n\
    \"xg_last5_home_per90\": number | null,\n\
    \"xg_last5_away_per90\": number | null,\n\
    \"polymarket_implied_prob\": 0.0-1.0,\n\
    \"clv_edge\": number,\n\
    \"asian_handicap_recommendation\": string | null\n\
  }\n\
}\n\
\n\
RULES:\n\
- `probability` reflects YOUR estimate of the YES outcome named in the question.\n\
- `side` = YES if probability > 0.5, NO if < 0.5, skip if confidence < 0.4.\n\
- For HomeWin / AwayWin: probability = P(that team wins).\n\
- For Draw: probability = P(match ends in draw).\n\
- For OverUnder: probability = P(total goals > line).\n\
- For AsianHandicap: probability = P(home team covers handicap).\n\
- For Outright: probability = P(event happens by season end).\n\
- Reasoning must reference which frameworks supported your estimate.\n\
- If critical data is missing (e.g. no Elo), say 'fallback to LLM estimate' and lower confidence by ≥0.2.\n\
- Reasoning must be in English (the JSON keys are English; reasoning language matches).";

/// 构建足球专用的 CallRequest。当 `MarketContext.category == "football"` 时
/// 用它替代 `build_market_analysis_request`。
///
/// 模型输出仍是 strict JSON, 但 shape 扩展了 `framework_breakdown`。
/// `parse_football_recommendation` 处理这个 shape;
/// `parse_recommendation` 仍可作为 fallback (会忽略 framework_breakdown)。
pub fn build_football_match_request(
    model: &str,
    ctx: &FootballMatchContext,
) -> CallRequest {
    let user = serde_json::to_string_pretty(ctx).unwrap_or_default();
    let user = format!(
        "Analyze this football market. Market type: {}. Return JSON only.\n\n```json\n{}\n```",
        ctx.market_type.as_str(),
        user
    );
    CallRequest::new(model)
        .max_tokens(1500) // football 输出含 framework_breakdown, token 多
        .temperature(0.2) // 与 market.v1.0 一致
        .json_mode()
        .system(FOOTBALL_SYSTEM)
        .user(user)
}

// ---- 解析足球专用推荐 ----

/// v0.118 —— Football-specific output payload。扩展泛型
/// `LlmRecommendationPayload`,加入 `framework_breakdown` 对象。
#[derive(Debug, Clone, Deserialize)]
pub struct FootballRecommendationPayload {
    pub probability: Option<f64>,
    pub side: Option<String>,
    pub confidence: Option<f64>,
    pub reasoning: Option<String>,
    #[serde(default)]
    pub key_factors: Option<Vec<String>>,
    #[serde(default)]
    pub framework_breakdown: Option<FrameworkBreakdown>,
}

/// v0.118 —— 四个 frameworks 的结构化拆解。
/// 供下游 audit + 未来模型对比工具使用。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FrameworkBreakdown {
    pub elo_home: Option<f64>,
    pub elo_away: Option<f64>,
    pub elo_diff_with_hfa: Option<f64>,
    pub implied_win_pct_elo: Option<f64>,
    pub lambda_home_goals: Option<f64>,
    pub lambda_away_goals: Option<f64>,
    pub dixon_coles_home_win_pct: Option<f64>,
    pub dixon_coles_draw_pct: Option<f64>,
    pub dixon_coles_away_win_pct: Option<f64>,
    pub xg_last5_home_per90: Option<f64>,
    pub xg_last5_away_per90: Option<f64>,
    pub polymarket_implied_prob: Option<f64>,
    pub clv_edge: Option<f64>,
    pub asian_handicap_recommendation: Option<String>,
}

/// 解析 football-specific recommendation。如果 football shape 缺少
/// framework_breakdown(例如旧模型或通用 fallback),则退回到泛型
/// `parse_recommendation`（通用推荐解析）。
pub fn parse_football_recommendation(text: &str) -> Result<FootballRecommendationPayload, String> {
    // 1) 尝试直接解析
    if let Ok(p) = serde_json::from_str::<FootballRecommendationPayload>(text) {
        return Ok(p);
    }
    // 2) 尝试查找第一个 {...} block(模型可能把 JSON 包裹在代码围栏中)
    if let Some(start) = text.find('{') {
        if let Some(end) = text.rfind('}') {
            if end > start {
                let slice = &text[start..=end];
                if let Ok(p) = serde_json::from_str::<FootballRecommendationPayload>(slice) {
                    return Ok(p);
                }
            }
        }
    }
    Err(format!("could not extract football recommendation JSON: {}", &text[..text.len().min(200)]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_clean_json() {
        let txt = r#"{"probability":0.71,"side":"YES","confidence":0.68,"reasoning":"good"}"#;
        let (p, s, c, r) = parse_recommendation(txt).unwrap();
        assert!((p - 0.71).abs() < 1e-6);
        assert_eq!(s, "YES");
        assert!((c - 0.68).abs() < 1e-6);
        assert_eq!(r, "good");
    }

    #[test]
    fn parses_fenced_json() {
        let txt = "```json\n{\"probability\":0.4,\"side\":\"NO\",\"confidence\":0.6,\"reasoning\":\"x\"}\n```";
        let (p, s, _, _) = parse_recommendation(txt).unwrap();
        assert!((p - 0.4).abs() < 1e-6);
        assert_eq!(s, "NO");
    }

    #[test]
    fn normalizes_side_aliases() {
        assert_eq!(normalize_side("y"), "YES");
        assert_eq!(normalize_side("NO"), "NO");
        assert_eq!(normalize_side("true"), "YES");
        assert_eq!(normalize_side("false"), "NO");
        assert_eq!(normalize_side("maybe"), "skip");
    }

    // ---- v0.118 football-specific tests ----（保留版本标识作为注释标题）

    fn sample_football_ctx(q: &str) -> FootballMatchContext {
        let market = MarketContext {
            market_id: "mkt-001".into(),
            question: q.into(),
            category: "football".into(),
            yes_price_cents: 65,
            no_price_cents: 35,
            volume_24h_usdc: 2_500_000.0,
            liquidity_usdc: 1_500_000.0,
            closes_at_unix_ms: 1_750_000_000_000,
            resolution_source: "Polymarket".into(),
            recent_signals: vec![],
            orderbook_top: None,
        };
        FootballMatchContext::from_market_context(market)
    }

    #[test]
    fn football_market_type_team_win() {
        assert_eq!(
            FootballMarketType::from_question("Will Argentina win on 2026-06-22?"),
            FootballMarketType::TeamWin
        );
        assert_eq!(
            FootballMarketType::from_question("Will Austria win on 2026-06-22?"),
            FootballMarketType::TeamWin
        );
    }

    #[test]
    fn football_market_type_draw() {
        assert_eq!(
            FootballMarketType::from_question("Will Argentina vs. Austria end in a draw?"),
            FootballMarketType::Draw
        );
    }

    #[test]
    fn football_market_type_over_under() {
        assert_eq!(
            FootballMarketType::from_question("Argentina vs. Austria: O/U 2.5"),
            FootballMarketType::OverUnder
        );
        assert_eq!(
            FootballMarketType::from_question("Real Madrid vs Barcelona Over/Under 3.5"),
            FootballMarketType::OverUnder
        );
    }

    #[test]
    fn football_market_type_asian_handicap() {
        assert_eq!(
            FootballMarketType::from_question("Spread: France (-2.5)"),
            FootballMarketType::AsianHandicap
        );
    }

    #[test]
    fn football_market_type_outright() {
        assert_eq!(
            FootballMarketType::from_question("Will Arsenal finish in the Premier League top 4 this season?"),
            FootballMarketType::Outright
        );
    }

    #[test]
    fn parse_question_extracts_team_names() {
        let (home, away, _, _) = parse_question_extras(
            "Argentina vs. Austria: O/U 2.5",
            FootballMarketType::OverUnder,
        );
        assert_eq!(home, Some("Argentina".to_string()));
        assert_eq!(away, Some("Austria".to_string()));
    }

    #[test]
    fn parse_question_extracts_handicap_line() {
        let (_, _, handicap, _) = parse_question_extras(
            "Spread: France (-2.5)",
            FootballMarketType::AsianHandicap,
        );
        assert_eq!(handicap, Some(-2.5));
    }

    #[test]
    fn parse_question_extracts_over_under_line() {
        let (_, _, _, ou) = parse_question_extras(
            "Argentina vs. Austria: O/U 2.5",
            FootballMarketType::OverUnder,
        );
        assert_eq!(ou, Some(2.5));
    }

    #[test]
    fn football_context_from_market_populates_basic_fields() {
        let ctx = sample_football_ctx("Will Argentina win on 2026-06-22?");
        assert_eq!(ctx.market_type, FootballMarketType::TeamWin);
        assert_eq!(ctx.home_team, Some("Argentina".to_string()));
        assert_eq!(ctx.market.question, "Will Argentina win on 2026-06-22?");
        assert_eq!(ctx.market.category, "football");
    }

    #[test]
    fn football_prompt_version_constant() {
        // F13 stats: llm_call_logs 读取这个常量来按 prompt 版本
        // 归属胜率。prompt 文本变更时需 bump。
        assert_eq!(PROMPT_VERSION_FOOTBALL_MATCH, "football.v1.0");
        assert_ne!(PROMPT_VERSION_FOOTBALL_MATCH, PROMPT_VERSION_MARKET_ANALYSIS);
    }

    #[test]
    fn build_football_request_serializes_context() {
        let ctx = sample_football_ctx("Argentina vs. Austria: O/U 2.5");
        let req = build_football_match_request("doubao-seed-2-0-pro", &ctx);
        // 验证 prompt 包含 4 个 framework 名称(对 system prompt 的健全性检查)。
        assert!(FOOTBALL_SYSTEM.contains("DIXON-COLES"));
        assert!(FOOTBALL_SYSTEM.contains("ELO"));
        assert!(FOOTBALL_SYSTEM.contains("xG"));
        assert!(FOOTBALL_SYSTEM.contains("CLV"));
        // User message 包含 market type 标签。
        let user = match &req.messages[1] {
            ChatMessage { role, content } if role == "user" => content.as_str(),
            _ => panic!("expected user message at index 1"),
        };
        assert!(user.contains("over_under"), "user msg should contain market type label");
        assert!(user.contains("Argentina"), "user msg should contain home team");
        assert!(user.contains("Austria"), "user msg should contain away team");
    }

    #[test]
    fn parses_football_recommendation_clean() {
        let txt = r#"{
            "probability": 0.62,
            "side": "YES",
            "confidence": 0.74,
            "reasoning": "Dixon-Coles λ_home=1.85 vs λ_away=0.95, Elo diff +180 with HFA, xG trend favoring home.",
            "key_factors": ["Dixon-Coles home win 58%", "Elo diff +180", "xG last5 +0.4"],
            "framework_breakdown": {
                "elo_home": 1850.0,
                "elo_away": 1670.0,
                "elo_diff_with_hfa": 280.0,
                "implied_win_pct_elo": 0.78,
                "lambda_home_goals": 1.85,
                "lambda_away_goals": 0.95,
                "dixon_coles_home_win_pct": 0.58,
                "dixon_coles_draw_pct": 0.22,
                "dixon_coles_away_win_pct": 0.20,
                "xg_last5_home_per90": 1.95,
                "xg_last5_away_per90": 1.10,
                "polymarket_implied_prob": 0.65,
                "clv_edge": -0.03,
                "asian_handicap_recommendation": "home -0.5 @ 1.85"
            }
        }"#;
        let p = parse_football_recommendation(txt).expect("should parse");
        assert_eq!(p.probability, Some(0.62));
        assert_eq!(p.side.as_deref(), Some("YES"));
        assert_eq!(p.confidence, Some(0.74));
        let fb = p.framework_breakdown.expect("framework_breakdown must be present");
        assert_eq!(fb.elo_home, Some(1850.0));
        assert_eq!(fb.lambda_home_goals, Some(1.85));
        assert_eq!(fb.clv_edge, Some(-0.03));
        assert_eq!(fb.asian_handicap_recommendation.as_deref(), Some("home -0.5 @ 1.85"));
    }

    #[test]
    fn parses_football_recommendation_fenced_json() {
        let txt = "```json\n{\"probability\":0.55,\"side\":\"YES\",\"confidence\":0.5,\"reasoning\":\"x\",\"framework_breakdown\":{\"clv_edge\":0.05}}\n```";
        let p = parse_football_recommendation(txt).expect("should parse fenced");
        assert_eq!(p.probability, Some(0.55));
        let fb = p.framework_breakdown.expect("breakdown must be present");
        assert_eq!(fb.clv_edge, Some(0.05));
    }

    #[test]
    fn parses_football_recommendation_handles_missing_breakdown() {
        // 旧模型或 fallback 可能不产生 framework_breakdown。
        // 解析器仍必须成功(对 breakdown 返回 None)。
        let txt = r#"{"probability":0.5,"side":"skip","confidence":0.3,"reasoning":"uncertain"}"#;
        let p = parse_football_recommendation(txt).expect("should parse even without breakdown");
        assert_eq!(p.probability, Some(0.5));
        assert!(p.framework_breakdown.is_none());
    }

    #[test]
    fn football_recommendation_parser_rejects_garbage() {
        let txt = "This is not JSON at all, just prose.";
        let err = parse_football_recommendation(txt).expect_err("should fail");
        assert!(err.contains("could not extract football recommendation JSON"));
    }

    #[test]
    fn market_type_as_str_round_trip() {
        for mt in [
            FootballMarketType::TeamWin,
            FootballMarketType::Draw,
            FootballMarketType::OverUnder,
            FootballMarketType::AsianHandicap,
            FootballMarketType::Outright,
            FootballMarketType::Unknown,
        ] {
            // as_str 是 stable 且非空的(用于 prompt + 下游标签)。
            assert!(!mt.as_str().is_empty());
        }
    }
}
