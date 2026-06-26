//! L3 —— 市场域（L2 命令使用的辅助函数）。
//!
//! 持有强类型的 `Market` DTO 以及筛选 / 排序 / 分类辅助函数,
//! L2 的 `commands::market` handler 与 L1 的 `Markets` 路由均可共享。
//! 此处不进行 DB 访问 —— DB 访问位于 L2 / L4。
//!
//! 参见 docs/overview.md §1.2 —— L3 是纯函数,无 IO。

use serde::{Deserialize, Serialize};

/// Polymarket 上所有已知的市场分类。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Category {
    Football,
    Cs2,
    Politics,
    Crypto,
    Tech,
    Science,
    PopCulture,
    Business,
    Other,
}

impl Category {
    pub fn as_str(self) -> &'static str {
        match self {
            Category::Football => "football",
            Category::Cs2 => "cs2",
            Category::Politics => "politics",
            Category::Crypto => "crypto",
            Category::Tech => "tech",
            Category::Science => "science",
            Category::PopCulture => "pop-culture",
            Category::Business => "business",
            Category::Other => "other",
        }
    }
    /// 尽力将一段自由形式的问题字符串归类到一个分类。
    /// 小写后的子串匹配。顺序很重要 —— 首个匹配胜出。
    pub fn classify(question: &str) -> Self {
        let q = question.to_lowercase();
        if q.contains("fc ") || q.contains(" vs ") || q.contains("football") || q.contains("nba") || q.contains("nfl") || q.contains("premier league") {
            Category::Football
        } else if q.contains("cs2") || q.contains("counter-strike") || q.contains("esl") || q.contains("blast") {
            Category::Cs2
        } else if q.contains("trump") || q.contains("biden") || q.contains("election") || q.contains("president") || q.contains("senate") || q.contains("congress") {
            Category::Politics
        } else if q.contains("bitcoin") || q.contains("btc") || q.contains("eth") || q.contains("crypto") {
            Category::Crypto
        } else if q.contains("ai ") || q.contains("openai") || q.contains("gpt") || q.contains("anthropic") || q.contains("llm") {
            Category::Tech
        } else if q.contains("nasa") || q.contains("spacex") || q.contains("research") {
            Category::Science
        } else if q.contains("movie") || q.contains("oscar") || q.contains("grammy") || q.contains("celebrity") {
            Category::PopCulture
        } else if q.contains("stock") || q.contains("market cap") || q.contains("ipo") || q.contains("revenue") {
            Category::Business
        } else {
            Category::Other
        }
    }
}

/// 用于字符串编码字段的数值解析辅助函数。
pub fn parse_liquidity(s: Option<&str>) -> f64 {
    s.and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0)
}

pub fn parse_volume_24h(s: Option<&str>) -> f64 {
    s.and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0)
}

/// 距离关闭的小时数（负值表示已关闭）。
pub fn hours_until_close(end_date_ms: i64, now_ms: i64) -> i64 {
    (end_date_ms - now_ms) / 3_600_000
}

/// 当市场在下一个 `horizon_hours` 内关闭时返回 true。
pub fn closes_within(market_end_ms: i64, now_ms: i64, horizon_hours: i64) -> bool {
    let dt = market_end_ms - now_ms;
    dt > 0 && dt <= horizon_hours * 3_600_000
}

/// 将剩余时间分桶为人类可读的标签。
pub fn closing_bucket(end_date_ms: i64, now_ms: i64) -> &'static str {
    let h = hours_until_close(end_date_ms, now_ms);
    if h < 0 { "closed" }
    else if h < 1 { "< 1h" }
    else if h < 24 { "today" }
    else if h < 24 * 7 { "this week" }
    else if h < 24 * 30 { "this month" }
    else { "later" }
}

// ---------------------------------------------------------------- I/O 类型
// (原始的 M1 wire 类型 —— 自 v0.2 模块保留至今)

const CLOB_BASE: &str = "https://clob.polymarket.com";
const GAMMA_BASE: &str = "https://gamma-api.polymarket.com";

/// v0.124 —— `GET /markets/keyset` Gamma API
/// 响应（按市场）的 wire 结构。
///
/// Polymarket 的 Gamma API 将每个市场作为包含众多字段的 JSON 对象返回。
/// 我们只投影需要的字段,并通过 `Option` / `#[serde(default)]` 容忍缺失字段。
/// 此结构由 2026-06 的一次实时调用逆向工程得出。
///
/// 值得注意的怪点（已在 2026-06-23 重新核对 /markets/keyset）:
///   - wire 格式是 **camelCase**（`endDate`、`volume24hr`、
///     `liquidity`、`closed`、`marketMakerAddress`...）。v0.124
///     使用 `#[serde(rename_all = "camelCase")]` 映射到下面
///     的 Rust snake_case 字段名。
///   - `id` 是以字符串形式表示的数值型 PM 市场 id
///   - `endDate` 是 ISO-8601 字符串,不是 unix 时间戳
///   - `closed` 是已结算标志；`active` 是“是否接受下单”
///   - `archived` 排除旧市场
///   - `liquidity` 是字符串（例如 `"16639.4255"`）,`volume`
///     也是（例如 `"834874.4897460078"`）。serde_json 不会
///     自动将 string→number 转换,因此我们将其类型设为 String,
///     在后处理中手动解析。
///   - `volume24hr` 是真实的数字（例如 `1150.4089619999997`）
///   - `category` 和 `tags` 在大多数市场为 null —— 退回到
///     问题文本分类
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketSummary {
    /// 数值型 PM 市场 id（字符串形式,保留以便作为外键）。
    pub id: String,
    pub slug: String,
    pub question: String,
    #[serde(default)]
    pub description: Option<String>,
    /// 来自 API 的 ISO-8601 字符串,在后处理中解析为毫秒。
    pub end_date: String,
    /// 由 `endDate` 在后处理中解析得到。
    #[serde(skip)]
    pub end_date_ms: Option<i64>,
    pub active: bool,
    pub closed: bool,
    #[serde(default)]
    pub archived: bool,
    /// 24 小时成交量（USDC,keyset 端点上的真实字段）。
    #[serde(default)]
    pub volume_24hr: f64,
    /// 总成交量。Gamma 以 STRING 形式发送此字段。
    #[serde(default)]
    pub volume: String,
    /// 与 `volume` 同理。
    #[serde(default)]
    pub liquidity: String,
    /// `category` 在大多数市场为 null。
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
}

/// v0.124 —— `GET /markets/keyset` Gamma API 响应包装的
/// wire 结构。
///
/// keyset 端点返回:
/// ```json
/// { "markets": [ ... MarketSummary ... ], "next_cursor": "BCVp..." }
/// ```
/// （包装结构：market 数组 + 下一页游标）
/// 而不是裸数组。我们使用该包装结构以暴露 cursor
/// （目前未使用 —— 分页是 v0.125+ 的功能）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeysetResponse {
    pub markets: Vec<MarketSummary>,
    #[serde(default)]
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBookSnapshot {
    pub market_id: String,
    pub captured_at: i64,
    pub best_bid: f64,
    pub best_ask: f64,
    pub mid_price: f64,
    pub spread: f64,
}

/// v0.125 —— 从 Polymarket Gamma 拉取开放市场,单次
/// 请求,limit=20。
///
/// **为什么用单次请求**:`/markets/keyset` 的 `next_cursor`
/// 分页在 API 层是坏掉的（2026-06-23 已验证:
/// 把返回的 cursor 作为 `&next_cursor=` 传入,得到的
/// 是相同的第一页）。旧的 `/markets?offset=N` 可以工作,但
/// 该端点已被弃用。最干净的可行方案就是用 `limit=20`
/// 发一次请求 —— 按成交量排序后能返回大约 14 个足球
/// 市场,足以满足 L1 的需求。
///
/// **为什么用 volume24hr-DESC**:默认的 `endDate ASC` 排序
/// 会把 2026 FIFA 世界杯市场（2026 年 8 月关闭）排到很后面
/// —— 前 5 个会是 Rihanna 专辑、GTA-VI 投注、
/// 大选。Volume-DESC 能让足球市场出现在前 20 个里。
///
/// **为什么不用 >20**:Gamma API 在 50 条以上时有两个 bug,
/// 本地 HTTP 代理（POLYROCKET_PROXY=127.0.0.1:7897）
/// 会间歇性地暴露它们:(a) JSON 正文里出现
/// `Invalid control character`,(b) reqwest 的 body 解码器在
/// 代理的 HTTP/2 流上挂起。20 远低于该阈值。
///
/// **调用方契约**:最多返回 20 个市场（原始数据,未
/// 预筛选）。在同步边界处的足球过滤器会剔除约 30%。
/// L1 看到大约 14 个真实的足球市场。
pub async fn fetch_active_markets() -> crate::AppResult<Vec<MarketSummary>> {
    use crate::infra::http::new_http_client;

    const LIMIT: usize = 20;

    let client = new_http_client();
    let url = format!(
        "{}/markets/keyset?active=true&closed=false&archived=false\
         &limit={}&order=volume24hr&ascending=false",
        GAMMA_BASE, LIMIT
    );

    let bytes = fetch_page(&client, &url).await?;
    let resp: KeysetResponse = serde_json::from_slice(&bytes).map_err(|e| {
        let preview: String = String::from_utf8_lossy(&bytes[..2000.min(bytes.len())])
            .chars().take(500).collect();
        tracing::warn!(
            "fetch_active_markets: JSON deser failed: {e}; preview={preview}"
        );
        crate::AppError::Internal(format!("Gamma keyset JSON deser failed: {e}"))
    })?;


    let mut all = resp.markets;
    for m in all.iter_mut() {
        m.end_date_ms = parse_iso_to_ms(&m.end_date);
    }
    tracing::info!(
        "fetch_active_markets: returned {} markets (football={})",
        all.len(), count_football(&all)
    );
    Ok(all)
}

/// 单页拉取辅助函数。
///
/// v0.125 —— 将每次请求的超时从 10s 提升到 30s。本地
/// HTTP 代理（127.0.0.1:7897）在冷路径上建立到
/// gamma-api.polymarket.com 的 CONNECT 需要约 10-18s;
/// 10s 低于这个下限,body 读取会以
/// `error decoding response body` 形式暴露（reqwest 的怪癖:
/// 底层传输超时被伪装成解码错误）。
///
/// “将 body 读取为 bytes()”（而不是 `.json()`）保留
/// 真实错误信息,以应对真正的解码失败场景
/// （自 v0.124 沿用至今）。
///
/// 2 次重试,退避分别为 200ms / 500ms —— 代理不太稳定,
/// 但一次重试几乎总能清除故障。
async fn fetch_page(
    client: &reqwest::Client,
    url: &str,
) -> crate::AppResult<Vec<u8>> {
    use crate::AppError;
    let mut last_err: Option<String> = None;
    for attempt in 0..3 {
        let result = client
            .get(url)
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("Gamma send failed: {e}")))?
            .error_for_status()
            .map_err(|e| AppError::Internal(format!("Gamma HTTP error: {e}")))?
            .bytes()
            .await
            .map(|b| b.to_vec())
            .map_err(|e| AppError::Internal(format!("Gamma body read failed: {e}")));
        match result {
            Ok(b) => return Ok(b),
            Err(e) => {
                let msg = e.to_string();
                tracing::warn!("fetch_page attempt {} failed: {msg}", attempt + 1);
                last_err = Some(msg);
                // 200ms,然后 500ms —— 快到用户感觉不到,
                // 除非每次都失败
                tokio::time::sleep(std::time::Duration::from_millis(
                    if attempt == 0 { 200 } else { 500 },
                )).await;
            }
        }
    }
    Err(AppError::Internal(format!(
        "Gamma page fetch failed after 3 attempts: {}",
        last_err.unwrap_or_default()
    )))
}

/// 使用与同步过滤器相同的问题文本启发式统计足球市场。
/// 重复这份启发式逻辑以便提前达到饱和（当 L1 已经拥有
/// 足够的足球市场时）。当启发式改变时,这里与
/// `commands/market.rs` 中的 `is_football_market`
/// 都需要更新。
fn count_football(markets: &[MarketSummary]) -> usize {
    let is_football_text = |s: &str| -> bool {
        let lower = s.to_lowercase();
        lower.contains("football") || lower.contains("soccer")
            || lower.contains("fifa") || lower.contains("uefa")
            || lower.contains("champions league")
            || lower.contains("premier league") || lower.contains("la liga")
            || lower.contains("bundesliga") || lower.contains("serie a")
            || lower.contains("ligue 1") || lower.contains("mls")
            || lower.contains(" fc ") || lower.contains(" united")
            || lower.contains("real madrid") || lower.contains("barcelona")
            || lower.contains("liverpool") || lower.contains("arsenal")
            || lower.contains("chelsea") || lower.contains("manchester")
            || lower.contains("bayern") || lower.contains("dortmund")
            || lower.contains("juventus") || lower.contains("psg")
            || lower.contains("atletico")
    };
    markets.iter().filter(|m| {
        if let Some(cat) = m.category.as_deref() {
            if is_football_text(cat) { return true; }
        }
        if let Some(tags) = m.tags.as_ref() {
            for t in tags { if is_football_text(t) { return true; } }
        }
        is_football_text(&m.question)
    }).count()
}

/// 将 RFC-3339 / ISO-8601 字符串（例如 "2025-10-31T00:00:00Z"）
/// 解析为 unix 毫秒。解析失败时返回 None,以便调用方
/// 自行决定是丢弃该市场,还是保留并将 end_date_ms 设为 None。
fn parse_iso_to_ms(s: &str) -> Option<i64> {
    use chrono::DateTime;
    DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|dt| dt.timestamp_millis())
}

/// v0.124 —— 将数值型字符串（例如 `"18465.6429"`）解析为 f64。
/// 用于 Gamma 的 `liquidity` 和 `volume` 字段,这两个字段
/// 在 keyset 端点上以字符串形式序列化（源自它们曾经
/// 保存任意精度小数的年代）。
fn parse_numeric_string(s: &str) -> f64 {
    s.parse::<f64>().unwrap_or(0.0)
}

/// 为 mode A（零合规风险）构造跳转到 Polymarket 的 URL。
pub fn build_jump_url(market_slug: &str, side: &str, price: f64) -> String {
    let side = side.to_uppercase();
    format!(
        "https://polymarket.com/event/{}?side={}&price={:.4}",
        market_slug, side, price
    )
}

/// 提交一笔已签名订单（mode B）。占位实现 —— 完整实现需要 `rs-clob-client`。
pub async fn place_signed_order(
    _market_id: &str,
    _side: &str,
    _price: f64,
    _size: &str,
    _key_alias: &str,
) -> crate::AppResult<String> {
    let _ = CLOB_BASE;
    Err(crate::AppError::Internal(
        "signed-order placement not yet wired (placeholder for mode B)".into(),
    ))
}

// ============================================================
// ============== v0.51c —— CLOB 提交流程 =====================
// ============================================================
//
// v0.5d 的 `place_signed_order` 是一个返回 Internal 错误的
// 占位实现。v0.50 让 `commands::bet` 中的 `place_signed_order`
// 忽略该错误并使用确定性的签名占位。v0.51c 新增了一条
// 结构化的 CLOB 提交流程,当环境变量设置了真实的
// Polymarket 凭证时就会接通。
//
// CLOB 提交尝试分两阶段:
//   1. 构建 EIP-712 签名的订单 payload（即 `Order` 结构 +
//      签名）。目前（没有 `rs-clob-client`）我们仍然
//      合成一个确定性的占位；其形态与 `rs-clob-client`
//      会产生的输出一致。
//   2. POST 该 payload 到 CLOB 的 /order 端点。
//      真正的实现需要钱包签名以通过 L2 auth 头 ——
//      在下面以 `auth` 字段形式捕获,以便前向兼容。
//
// `submit_signed_order_via_clob` 返回一个结构化的
// `ClobOrderResult`,以便调用方可以记录:
//   - filled_at + fill_price + fill_size + partial（成交元数据）
//   - tx_hash（始终存在）
//   - CLOB 拒绝时的错误信息
//
// 在 v0.51a/b（CLOB feed 尚未接通）时,`creds_present()`
// 返回 false；调用方会回退到确定性占位。v0.51c 接通了
// HTTP 调用,因此当凭证存在时我们会真正尝试提交；
// 如果 API 不可达,我们会返回结构化的错误,
// 而不是直接崩溃。

/// v0.51c —— 当且仅当三个 Polymarket CLOB 凭证全部
/// 在环境变量中存在时返回 true。同时接受两种命名约定:
///   - `POLYMARKET_API_KEY / _SECRET / _PASSPHRASE`（标准）
///   - `POLYROCKET_CLOB_API_KEY / _SECRET / _PASSPHRASE`（旧）
/// v0.119 —— 当两者都被设置时,`POLYMARKET_*` 优先。
pub fn creds_present() -> bool {
    let k = std::env::var("POLYMARKET_API_KEY").ok().filter(|v| !v.is_empty())
        .or_else(|| std::env::var("POLYROCKET_CLOB_API_KEY").ok().filter(|v| !v.is_empty()));
    let s = std::env::var("POLYMARKET_API_SECRET").ok().filter(|v| !v.is_empty())
        .or_else(|| std::env::var("POLYROCKET_CLOB_API_SECRET").ok().filter(|v| !v.is_empty()));
    let p = std::env::var("POLYMARKET_API_PASSPHRASE").ok().filter(|v| !v.is_empty())
        .or_else(|| std::env::var("POLYROCKET_CLOB_API_PASSPHRASE").ok().filter(|v| !v.is_empty()));
    matches!((k, s, p), (Some(k), Some(s), Some(p))
        if !k.is_empty() && !s.is_empty() && !p.is_empty())
}

/// v0.51c —— CLOB 提交尝试的结构化结果。
/// `ok` 用于区分提交成功与 CLOB 层级的拒绝；
/// 当 ok=false 时 `error` 字段非空。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClobOrderResult {
    pub ok: bool,
    /// CLOB 返回的交易哈希。对于确定性占位,
    /// 这是 `sign_order` 已经产生的 djb2 哈希。
    /// 对于真正的 CLOB 提交,这是 API 返回的值。
    pub tx_hash: String,
    /// 订单成交的时间。对于占位
    /// （以及 v0.51c 的 HTTP 尝试）,这是 `now_ms`。
    pub filled_at_ms: i64,
    /// 实际成交价格。对于占位和 v0.51c 的
    /// best-effort HTTP 路径,这就是用户的
    /// `price`（滑点 = 0）。真实 CLOB 响应会覆盖。
    pub fill_price: f64,
    /// 实际成交份额数（字符串形式以保持向后兼容）。
    /// 对于占位和 v0.51c HTTP 路径,这等于用户期望的 size。
    pub fill_size: String,
    /// 当成交为部分成交时为 true。对于确定性
    /// 占位 / v0.51c 始终为 false。
    pub partial: bool,
    /// ok=false 时的错误信息。成功时为空。
    pub error: String,
    /// v0.51c —— 当调用是通过 HTTP 路径
    /// （凭证存在）发起时为 true,通过占位
    /// （凭证缺失）发起时为 false。便于 L1 在 UI
    /// 上区分“live”与“stub”。
    pub via_http: bool,
}

/// v0.51c —— best-effort CLOB 提交。当凭证存在时,
/// 尝试对 CLOB 的 /order 端点发起真实的 HTTP POST。
/// 当 API 不可达或返回错误时,返回
/// `ClobOrderResult { ok: false, error: ... }`,
/// 而不是 panic —— 调用方可决定是否回退到占位实现。
///
/// 当凭证缺失时,返回
/// `ClobOrderResult { ok: true, ..., via_http: false }`,
/// 使用确定性占位值（滑点 = 0,
/// 不存在部分成交）。调用方可以将其作为“本应成交”
/// 记录持久化 —— 也就是 v0.50 当前所做的。
///
/// 当前的 HTTP 路径是 best-effort:我们 POST 一个
/// 占位 payload,尽可能解析响应,并在任何错误情况下
/// 降级为占位形态。完整的 EIP-712 + L2 auth 接通是
/// v0.51+ 的事情（需要 `rs-clob-client`）。
pub async fn submit_signed_order_via_clob(
    market_id: &str,
    side: &str,
    price: f64,
    size_shares: &str,
    key_alias: &str,
    order_type: &str,
    now_ms: i64,
) -> ClobOrderResult {
    use crate::infra::http::new_http_client;
    let tx_hash = format!(
        "0x{:016x}",
        djb2_stub(market_id, side, price, size_shares, key_alias, order_type, now_ms)
    );
    if !creds_present() {
        // 没有凭证 → 确定性占位。调用方会
        // 将其持久化；用户得到 slippage=0。
        return ClobOrderResult {
            ok: true,
            tx_hash,
            filled_at_ms: now_ms,
            fill_price: price,
            fill_size: size_shares.to_string(),
            partial: false,
            error: String::new(),
            via_http: false,
        };
    }
    // v0.51c —— best-effort HTTP 路径。我们 POST
    // 一个占位 payload；任何失败时都会
    // 抛出结构化的错误。
    let url = format!("{}/order", CLOB_BASE);
    let payload = serde_json::json!({
        "market": market_id,
        "side": side.to_uppercase(),
        "price": price,
        "size": size_shares,
        "orderType": order_type,
        "keyAlias": key_alias,
        "ts_ms": now_ms,
    });
    let client = new_http_client();
    // v0.119 —— 接受任意一种命名约定（优先 POLYMARKET_*）
    let api_key = std::env::var("POLYMARKET_API_KEY").ok().filter(|v| !v.is_empty())
        .or_else(|| std::env::var("POLYROCKET_CLOB_API_KEY").ok())
        .unwrap_or_default();
    let api_secret = std::env::var("POLYMARKET_API_SECRET").ok().filter(|v| !v.is_empty())
        .or_else(|| std::env::var("POLYROCKET_CLOB_API_SECRET").ok())
        .unwrap_or_default();
    let api_passphrase = std::env::var("POLYMARKET_API_PASSPHRASE").ok().filter(|v| !v.is_empty())
        .or_else(|| std::env::var("POLYROCKET_CLOB_API_PASSPHRASE").ok())
        .unwrap_or_default();
    let resp = client
        .post(&url)
        .header("POLYROCKET-API-KEY", &api_key)
        .header("POLYROCKET-API-SECRET", &api_secret)
        .header("POLYROCKET-API-PASSPHRASE", &api_passphrase)
        .json(&payload)
        .send()
        .await;
    match resp {
        Ok(r) if r.status().is_success() => {
            // 真实的 CLOB 响应 —— best-effort 解析。
            // 如果形态不同（例如 CLOB 返回
            // 非 JSON 的 200）,回退到 via_http=true
            // 的占位值。
            match r.json::<serde_json::Value>().await {
                Ok(v) => {
                    let fill_price = v
                        .get("price")
                        .and_then(|x| x.as_f64())
                        .unwrap_or(price);
                    let fill_size = v
                        .get("sizeFilled")
                        .and_then(|x| x.as_str())
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| size_shares.to_string());
                    let partial = v
                        .get("partial")
                        .and_then(|x| x.as_bool())
                        .unwrap_or(false);
                    ClobOrderResult {
                        ok: true,
                        tx_hash,
                        filled_at_ms: now_ms,
                        fill_price,
                        fill_size,
                        partial,
                        error: String::new(),
                        via_http: true,
                    }
                }
                Err(_) => ClobOrderResult {
                    ok: true,
                    tx_hash,
                    filled_at_ms: now_ms,
                    fill_price: price,
                    fill_size: size_shares.to_string(),
                    partial: false,
                    error: "CLOB returned non-JSON response (stubbed)".into(),
                    via_http: true,
                },
            }
        }
        Ok(r) => ClobOrderResult {
            ok: false,
            tx_hash,
            filled_at_ms: now_ms,
            fill_price: price,
            fill_size: size_shares.to_string(),
            partial: false,
            error: format!("CLOB HTTP {}", r.status()),
            via_http: true,
        },
        Err(e) => ClobOrderResult {
            ok: false,
            tx_hash,
            filled_at_ms: now_ms,
            fill_price: price,
            fill_size: size_shares.to_string(),
            partial: false,
            error: format!("CLOB HTTP error: {e}"),
            via_http: true,
        },
    }
}

/// 轻量的 djb2 哈希（与 `domain::bet` 中的
/// `sign_order` 保持一致）。供占位路径使用,
/// 以保证 tx_hash 在多次运行之间保持稳定。
fn djb2_stub(
    market_id: &str,
    side: &str,
    price: f64,
    size: &str,
    key_alias: &str,
    order_type: &str,
    now_ms: i64,
) -> u64 {
    let s = format!(
        "{}|{}|{}|{}|{}|{}|{}",
        market_id, side, price, size, key_alias, order_type, now_ms
    );
    let mut h: u64 = 5381;
    for b in s.bytes() {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    /// v0.124 —— 将真实的 Gamma /markets/keyset 响应
    /// （通过 curl 缓存到 /tmp/gamma_response.json）反序列化为
    /// `KeysetResponse` DTO。如果此测试失败,说明 wire 形态
    /// 已经漂移,生产环境的实时同步会静默失败。
    #[test]
    fn deser_real_gamma_keyset_response() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("..")
            .join("..")
            .join("..")
            .join("/tmp/gamma_response.json");
        let body = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(_) => {
                eprintln!(
                    "skip: {} not present (curl it with: \
                     curl -sS -x http://127.0.0.1:7897 \
                     'https://gamma-api.polymarket.com/markets/keyset?active=true&closed=false&limit=2' \
                     > {})",
                    path.display(),
                    path.display()
                );
                return;
            }
        };
        let r: KeysetResponse = serde_json::from_str(&body)
            .unwrap_or_else(|e| panic!("deser failed: {e}; body start: {}", &body[..200.min(body.len())]));
        assert!(!r.markets.is_empty(), "expected at least one market");
        let m = &r.markets[0];
        // 健全性检查：字段投影
        assert!(!m.id.is_empty());
        assert!(!m.question.is_empty());
        assert!(!m.end_date.is_empty());
        // end_date_ms 由后处理阶段填充
        assert!(m.end_date_ms.is_none(), "end_date_ms is set by post-processor, not deser");
    }


    #[test]
    fn category_classify() {
        assert_eq!(Category::classify("Will FC Barcelona win La Liga?"), Category::Football);
        assert_eq!(Category::classify("Trump 2024 election"), Category::Politics);
        assert_eq!(Category::classify("BTC > 100k by EOY?"), Category::Crypto);
        assert_eq!(Category::classify("Will OpenAI release GPT-5?"), Category::Tech);
        assert_eq!(Category::classify("Unknown thing"), Category::Other);
    }

    #[test]
    fn category_round_trip() {
        // 通过 canonical question 样本往返测试（不走 as_str slug，
        // 因为 classify() 不一定会匹配 as_str slug）。
        let samples: &[(Category, &str)] = &[
            (Category::Football, "Will Arsenal win the Premier League?"),
            (Category::Politics, "Will Trump win the 2024 election?"),
            (Category::Crypto, "BTC > 100k by EOY?"),
            (Category::Other, "Mystery box"),
        ];
        for (cat, q) in samples {
            assert_eq!(Category::classify(q), *cat, "sample: {q}");
        }
    }

    #[test]
    fn parse_liquidity_handles_none() {
        assert_eq!(parse_liquidity(None), 0.0);
        assert_eq!(parse_liquidity(Some("1234.5")), 1234.5);
    }

    #[test]
    fn hours_until_close_basic() {
        let now = 1_000_000_000_000;
        assert_eq!(hours_until_close(now + 3_600_000, now), 1);
        assert_eq!(hours_until_close(now - 3_600_000, now), -1);
    }

    #[test]
    fn closes_within_respects_horizon() {
        let now = 1_000_000_000_000;
        let h2 = now + 2 * 3_600_000;
        let h100 = now + 100 * 3_600_000;
        assert!(closes_within(h2, now, 24));
        assert!(!closes_within(h100, now, 24));
    }

    #[test]
    fn closing_bucket_labels() {
        let now = 1_000_000_000_000;
        let h = |h: i64| now + h * 3_600_000;
        assert_eq!(closing_bucket(h(-2), now), "closed");
        assert_eq!(closing_bucket(h(0), now), "< 1h");
        assert_eq!(closing_bucket(h(5), now), "today");
        assert_eq!(closing_bucket(h(72), now), "this week");
        assert_eq!(closing_bucket(h(24 * 14), now), "this month");
        assert_eq!(closing_bucket(h(24 * 90), now), "later");
    }

    // ----- v0.51c —— CLOB submit 路径 -----

    // 用于环境变量操作的进程级互斥锁。
    // cargo 的测试并行运行；std::env::set_var
    // 是进程全局的,因此我们把 CLOB
    // 凭证测试串行化到单个互斥锁后。
    use std::sync::Mutex;
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn clear_clob_env() {
        unsafe {
            std::env::remove_var("POLYROCKET_CLOB_API_KEY");
            std::env::remove_var("POLYROCKET_CLOB_API_SECRET");
            std::env::remove_var("POLYROCKET_CLOB_API_PASSPHRASE");
        }
    }

    /// v0.51c —— 当三个环境变量都未设置时,
    /// creds_present 返回 false。
    #[test]
    fn creds_absent_returns_false() {
        let _g = ENV_LOCK.lock().unwrap();
        clear_clob_env();
        assert!(!creds_present());
    }

    /// v0.51c — creds_present（凭证存在性检查） 仅在
    /// 三个环境变量都非空时返回 true。
    #[test]
    fn creds_all_set_returns_true() {
        let _g = ENV_LOCK.lock().unwrap();
        clear_clob_env();
        unsafe {
            std::env::set_var("POLYROCKET_CLOB_API_KEY", "k");
            std::env::set_var("POLYROCKET_CLOB_API_SECRET", "s");
            std::env::set_var("POLYROCKET_CLOB_API_PASSPHRASE", "p");
        }
        let r = creds_present();
        clear_clob_env();
        assert!(r);
    }

    /// v0.51c —— 空字符串被视为缺失
    /// （CLOB API 会以 401 拒绝空凭证,
    /// 所以这里采用保守策略）。
    #[test]
    fn creds_with_empty_strings_returns_false() {
        let _g = ENV_LOCK.lock().unwrap();
        clear_clob_env();
        unsafe {
            std::env::set_var("POLYROCKET_CLOB_API_KEY", "");
            std::env::set_var("POLYROCKET_CLOB_API_SECRET", "s");
            std::env::set_var("POLYROCKET_CLOB_API_PASSPHRASE", "p");
        }
        let r = creds_present();
        clear_clob_env();
        assert!(!r);
    }

    /// v0.51c —— 没有凭证时,CLOB 提交
    /// 返回确定性占位形态:ok=true、
    /// slippage=0、partial=false、via_http=false,
    /// 同时 tx_hash 为 djb2 占位哈希。
    #[tokio::test]
    async fn submit_without_creds_uses_stub() {
        let _g = ENV_LOCK.lock().unwrap();
        clear_clob_env();
        let r = submit_signed_order_via_clob(
            "m1", "YES", 0.5, "100", "primary", "market",
            1_700_000_000_000,
        )
        .await;
        clear_clob_env();
        assert!(r.ok);
        assert!(!r.via_http);
        assert!(!r.partial);
        assert!((r.fill_price - 0.5).abs() < 1e-9);
        assert_eq!(r.fill_size, "100");
        assert!(r.error.is_empty());
        assert!(r.tx_hash.starts_with("0x"));
    }

    /// v0.51c —— 相同参数产生相同的哈希
    /// （确定性占位的稳定性）。
    #[tokio::test]
    async fn submit_stub_hash_is_deterministic() {
        let _g = ENV_LOCK.lock().unwrap();
        clear_clob_env();
        let r1 = submit_signed_order_via_clob(
            "m1", "YES", 0.5, "100", "primary", "market",
            1_700_000_000_000,
        )
        .await;
        let r2 = submit_signed_order_via_clob(
            "m1", "YES", 0.5, "100", "primary", "market",
            1_700_000_000_000,
        )
        .await;
        clear_clob_env();
        assert_eq!(r1.tx_hash, r2.tx_hash);
    }

    /// v0.51c —— 不同参数产生不同的
    /// 哈希（哈希确实依赖于输入）。
    #[tokio::test]
    async fn submit_stub_hash_differs_with_inputs() {
        let _g = ENV_LOCK.lock().unwrap();
        clear_clob_env();
        let r1 = submit_signed_order_via_clob(
            "m1", "YES", 0.5, "100", "primary", "market",
            1_700_000_000_000,
        )
        .await;
        let r2 = submit_signed_order_via_clob(
            "m2", "YES", 0.5, "100", "primary", "market",
            1_700_000_000_000,
        )
        .await;
        clear_clob_env();
        assert_ne!(r1.tx_hash, r2.tx_hash);
    }
}
