//! L3 — 投注生命周期。
//!
//! 负责 `bets` 表的状态机：`open` → `won` / `lost` / `cancelled`。
//! 提供一个 L2 命令层可调用的 `place()` 入口；签单下单（Mode B）
//! 和 jump-link 构建（Mode A）都在此模块中。
//!
//! **状态（v0.3c）：存根。** 当前 `commands::bet` 模块含有可用 SQL，
//! 后续 M6「Bet placement」里程碑会迁移到本模块的辅助函数中。

use crate::AppError;
use crate::AppResult;
use serde::{Deserialize, Serialize};

/// 下单模式。Mode A 是 jump-link（用户去 Polymarket UI 签），Mode B 是用 OS keyring
/// 里的私钥直接签订单（自动化）。
///
/// **业务流程**：
///   - Mode A: build_jump_link → L1 拿到 URL，用户在浏览器完成
///   - Mode B: sign_order → 用 private key 在 Rust 端签 → 调 CLOB
///
/// **为什么两套**：Polymarket 的真实 CLOB 订单必须 EOA 签 keyring，但 demo / 早期
/// 阶段用户没有 wallet 或不想给 polyrocket 私钥，jump-link 是过渡方案。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BetMode {
    /// Mode A：仅 jump-link。用户前往 Polymarket UI 签单。
    AJump,
    /// Mode B：通过 OS keyring 签单下单。
    BSigned,
}

impl BetMode {
    pub fn as_str(self) -> &'static str {
        match self {
            BetMode::AJump => "A_jump",
            BetMode::BSigned => "B_signed",
        }
    }
    pub fn parse(s: &str) -> AppResult<Self> {
        match s {
            "A_jump" => Ok(BetMode::AJump),
            "B_signed" => Ok(BetMode::BSigned),
            _ => Err(AppError::Invalid(format!("unknown bet mode: {s}"))),
        }
    }
}

/// 单笔 bet 的状态机。`Open` → ( `Won` | `Lost` | `Cancelled` )。
///
/// **`Open` 含义**：已下单但市场还没 resolve，PnL 仍是 0。
/// **`Won` / `Lost`**：市场 resolve 后由 `reconcile_paper_fills` 标记。
/// **`Cancelled`**：用户手动取消或 RPC 失败。
///
/// **invariant**：`bets.status = 'open'` 的行表示「还有悬念」，PnL 计算时排除。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BetStatus {
    Open,
    Won,
    Lost,
    Cancelled,
}

impl BetStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            BetStatus::Open => "open",
            BetStatus::Won => "won",
            BetStatus::Lost => "lost",
            BetStatus::Cancelled => "cancelled",
        }
    }
    pub fn parse(s: &str) -> AppResult<Self> {
        match s {
            "open" => Ok(BetStatus::Open),
            "won" => Ok(BetStatus::Won),
            "lost" => Ok(BetStatus::Lost),
            "cancelled" => Ok(BetStatus::Cancelled),
            _ => Err(AppError::Invalid(format!("unknown bet status: {s}"))),
        }
    }
}

/// 下单方向。YES 买「事件发生」token，NO 买「事件不发生」token。
///
/// **PnL 对称性**：YES token 在 market resolve = YES 时付 $1（NO 0），
/// NO token 反之。`pnl()` 把两边统一到一个公式里。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BetSide {
    Yes,
    No,
}

impl BetSide {
    pub fn as_str(self) -> &'static str {
        match self {
            BetSide::Yes => "YES",
            BetSide::No => "NO",
        }
    }
    pub fn parse(s: &str) -> AppResult<Self> {
        match s.to_uppercase().as_str() {
            "YES" => Ok(BetSide::Yes),
            "NO" => Ok(BetSide::No),
            _ => Err(AppError::Invalid(format!("unknown side: {s}"))),
        }
    }
}

// ============================================================
// ============== v0.50a — Order type ==========================
// ============================================================
//
// Polymarket 支持 Market（按最优可成交价立即执行）和
// Limit（仅以 `limit_price` 或更优价格成交）。v0.50 还新增
// StopLoss（市价穿越 `stop_price` 时触发）作为 UX 原语 ——
// 底层 CLOB 调用仍是触发后下达的 Limit 订单。
//
// PostOnly（v0.50b）是一个 flag 而非 type，挂在 Limit
// 订单上：订单必须挂单在簿上，绝不立即吃单。

/// 订单类型。Market 立即成交（best available price），Limit 必须挂在 book 上等撮合，
/// StopLoss 是「触发后转 Limit」的模式。
///
/// **PostOnly 是什么**：一个 `bool` flag 不是 order type，挂在 Limit 上 —— 表示
/// 「必须挂单，绝不立刻吃单」（做市友好）。`check_post_only` 验证。
///
/// **StopLoss 当前状态**：DB 字段已存，但真实 trigger 逻辑待 v0.51+（需要 CLOB feed
/// 检测价格穿越）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum OrderType {
    /// 按最优可成交价立即成交。忽略 `limit_price`。
    #[default]
    Market,
    /// 挂单在簿上；仅以 `limit_price` 或更优价格成交。
    Limit,
    /// 市价穿越 `stop_price` 时触发，然后按下 Limit 单
    /// （`limit_price`，默认 = `stop_price`）。
    /// 当前该类型会被记录到订单中，但实际触发逻辑
    /// 待 v0.51+ 实现（需要真实 CLOB feed）。
    StopLoss,
}

impl OrderType {
    pub fn as_str(self) -> &'static str {
        match self {
            OrderType::Market => "market",
            OrderType::Limit => "limit",
            OrderType::StopLoss => "stop_loss",
        }
    }
    pub fn parse(s: &str) -> AppResult<Self> {
        match s.to_lowercase().as_str() {
            "market" => Ok(OrderType::Market),
            "limit" => Ok(OrderType::Limit),
            "stop_loss" | "stoploss" | "stop-loss" => Ok(OrderType::StopLoss),
            _ => Err(AppError::Invalid(format!("unknown order type: {s}"))),
        }
    }
}

// ============================================================
// ============== PnL 数学（纯函数） ==============================
// ============================================================

/// 买入的份额数 = USDC 金额 / 价格。
/// 当 size 或 price 非正或无法解析时返回 0。
pub fn shares_for_size(size_usdc: &str, price: f64) -> String {
    match size_usdc.parse::<f64>() {
        Ok(s) if s > 0.0 && price > 0.0 => (s / price).to_string(),
        _ => "0".to_string(),
    }
}

/// 市场以 YES 结算时的 PnL。
/// 对于 YES 投注：pnl = shares * (1 - price)（以 `price` 买入，YES 结算时获得 $1）。
/// 对于 NO 投注：pnl = -size（NO 份额归零）。
pub fn pnl_on_yes(side: BetSide, shares: f64, price: f64, size_usdc: f64) -> f64 {
    match side {
        BetSide::Yes => shares * (1.0 - price),
        BetSide::No => -size_usdc,
    }
}

/// 市场以 NO 结算时的 PnL。
/// 对于 YES 投注：pnl = -size。
/// 对于 NO 投注：pnl = shares * price（NO 份额付 $1）。
pub fn pnl_on_no(side: BetSide, shares: f64, price: f64, size_usdc: f64) -> f64 {
    match side {
        BetSide::Yes => -size_usdc,
        BetSide::No => shares * price,
    }
}

/// 根据结算结果计算 PnL。
pub fn pnl(outcome: BetSide, side: BetSide, shares: f64, price: f64, size_usdc: f64) -> f64 {
    match outcome {
        BetSide::Yes => pnl_on_yes(side, shares, price, size_usdc),
        BetSide::No => pnl_on_no(side, shares, price, size_usdc),
    }
}

/// 该状态下的投注是否仍处于「进行中」（不计入 PnL）。
pub fn is_open(s: BetStatus) -> bool {
    matches!(s, BetStatus::Open)
}

// ============================================================
// ============== Args validation ==============================
// ============================================================

/// USDC 金额上限（防御性上限，避免将 "100" 误输为 "10000"）。
pub const MAX_BET_SIZE_USDC: f64 = 10_000.0;
/// 最低价（1¢）和最高价（99¢）—— 真实 CLOB 市场中，区间外的价格无效。
pub const MIN_PRICE: f64 = 0.01;
pub const MAX_PRICE: f64 = 0.99;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaceArgs {
    pub market_id: String,
    pub side: BetSide,
    pub size_usdc: String,
    /// 「参考」价格 —— 用户认为该份额当前的价值。
    /// 对 Market 订单忽略；对 Limit 订单必须等于 `limit_price`
    /// （在 `price` 中存储用户「预期」用于分析）；对 StopLoss
    /// 则是用户希望的入场价。
    pub price: f64,
    pub key_alias: Option<String>,
    /// v0.50a —— 订单类型。默认 = Market（向后兼容）。
    #[serde(default)]
    pub order_type: OrderType,
    /// v0.50a —— 对 Limit 订单：仅以该价格或更优价格成交。
    /// 当 `order_type = Limit` 时必填。对 StopLoss 可选
    ///（默认等于 `stop_price`）。
    #[serde(default)]
    pub limit_price: Option<f64>,
    /// v0.50a —— 对 StopLoss 订单：当市价穿越该价格时触发
    ///（方向与目标持仓相反）。当 `order_type = StopLoss` 时必填。
    #[serde(default)]
    pub stop_price: Option<f64>,
    /// v0.50b —— 对 Limit 订单：必须挂单在簿上，绝不立即吃单。
    /// 对 Market 和 StopLoss 忽略。
    #[serde(default)]
    pub post_only: bool,
}

/// 校验 `place_*` args 负载。成功时返回已解析的 size。
pub fn validate_place_args(args: &PlaceArgs) -> AppResult<f64> {
    if args.market_id.trim().is_empty() {
        return Err(AppError::Invalid("market_id is empty".into()));
    }
    if args.market_id.len() > 128 {
        return Err(AppError::Invalid("market_id > 128 chars".into()));
    }
    if !(args.price.is_finite() && (MIN_PRICE..=MAX_PRICE).contains(&args.price)) {
        return Err(AppError::Invalid(format!(
            "price {} out of [{}, {}]",
            args.price, MIN_PRICE, MAX_PRICE
        )));
    }
    let size: f64 = args.size_usdc.parse().map_err(|_| {
        AppError::Invalid(format!("size_usdc not a number: {}", args.size_usdc))
    })?;
    if !size.is_finite() || size <= 0.0 {
        return Err(AppError::Invalid(format!("size_usdc must be > 0 (got {size})")));
    }
    if size > MAX_BET_SIZE_USDC {
        return Err(AppError::Invalid(format!(
            "size_usdc {size} > max {MAX_BET_SIZE_USDC}"
        )));
    }
    // v0.50a —— 订单类型相关的校验。
    validate_order_type_specifics(args)?;
    Ok(size)
}

/// v0.50a —— 额外的校验，仅依赖订单类型相关字段。
/// 独立拆分出来，便于 L1 在提交前单独执行「预检」调用。
pub fn validate_order_type_specifics(args: &PlaceArgs) -> AppResult<()> {
    match args.order_type {
        OrderType::Market => {
            if args.limit_price.is_some() || args.stop_price.is_some() {
                return Err(AppError::Invalid(
                    "market orders must not include limit_price or stop_price".into(),
                ));
            }
        }
        OrderType::Limit => {
            let lp = args.limit_price.ok_or_else(|| {
                AppError::Invalid("limit orders require limit_price".into())
            })?;
            if !lp.is_finite() || !(MIN_PRICE..=MAX_PRICE).contains(&lp) {
                return Err(AppError::Invalid(format!(
                    "limit_price {} out of [{}, {}]",
                    lp, MIN_PRICE, MAX_PRICE
                )));
            }
            if args.stop_price.is_some() {
                return Err(AppError::Invalid(
                    "limit orders must not include stop_price (use StopLoss)".into(),
                ));
            }
        }
        OrderType::StopLoss => {
            let sp = args.stop_price.ok_or_else(|| {
                AppError::Invalid("stop_loss orders require stop_price".into())
            })?;
            if !sp.is_finite() || !(MIN_PRICE..=MAX_PRICE).contains(&sp) {
                return Err(AppError::Invalid(format!(
                    "stop_price {} out of [{}, {}]",
                    sp, MIN_PRICE, MAX_PRICE
                )));
            }
            // StopLoss 触发方向：
            //   YES 投注 → 当价格上升至 stop_price 时触发
            //     （当行情反向运动时希望限制亏损，因此 stop_price 应 > price）
            //   NO 投注  → 当价格下跌至 stop_price 时触发
            //     （stop_price 应 < price）
            // v0.50a 不强制此关系（仅记录意图），但会保存该信息。
            let _ = args.limit_price.unwrap_or(sp); // 默认 = stop_price
        }
    }
    if args.post_only && args.order_type != OrderType::Limit {
        return Err(AppError::Invalid(
            "post_only is only valid for limit orders".into(),
        ));
    }
    Ok(())
}

// ============================================================
// ============== v0.50b — post-only 强制实施 ==============
// ============================================================
//
// Polymarket CLOB 沿用标准的「post-only」语义：
// 订单必须挂单在簿上，绝不立即吃单。
// 我们使用该市场最新的 price_snapshots（v0.47a）行来实现校验。
//
// 在 v0.51+ 引入真实订单簿 feed 之前，快照仅是占位
//（best_bid = best_ask = 0.5）。此时 `would_cross_book`
// 返回 false（总是挂单），对新装用户来说 post-only 实际
// 是 no-op。这可以接受：v0.50b 的重点是先把校验管道打通，
// 等真实 feed 接入时即可自动生效。
//
// 这里的 book 快照对应 YES token 视角；对 NO 投注
// 我们以 `1 - limit_price` 计算隐含 YES 价格并与 `best_bid` 比较。

/// v0.50b —— 纯辅助函数。给定 side、limit_price 和最新
/// book 快照，当订单会立即吃单时返回 `true`
///（在 post-only 下应被拒绝）。
///
/// 快照对应 YES token 的订单簿：`best_bid` 和 `best_ask`
/// 为 YES token 价格，范围 [0.01, 0.99]。
pub fn would_cross_book(
    side: BetSide,
    limit_price: f64,
    best_bid: f64,
    best_ask: f64,
) -> bool {
    match side {
        // 以限价 P 买入 YES：当 P >= best_ask 时会吃单（撮合卖单）。
        BetSide::Yes => limit_price >= best_ask,
        // 以限价 P 买入 NO：NO token 价格 = 1 - YES 价格。
        // 等价地，当 (1 - P) <= best_bid 时吃单，
        // 即 P >= 1 - best_bid。
        BetSide::No => limit_price >= 1.0 - best_bid,
    }
}

/// v0.50b —— post-only 强制检查的结果。
/// 返回类型化结果，让调用方能够区分「会穿越」
/// 与「快照缺失」（后者当前静默通过 —— 参见模块文档）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostOnlyCheck {
    /// 该市场暂无快照；post-only 当前为 no-op
    ///（真实强制逻辑延后到 v0.51+）。
    NoSnapshot,
    /// 存在快照；订单将挂单。OK。
    Rests,
    /// 存在快照；订单会穿越。拒绝。
    WouldCross,
}

/// v0.50b —— 给定一个快照（可为 None）和 post-only 标志，
/// 返回强制检查结果。
pub fn check_post_only(
    side: BetSide,
    limit_price: f64,
    snapshot: Option<(f64, f64)>,
) -> PostOnlyCheck {
    let Some((best_bid, best_ask)) = snapshot else {
        return PostOnlyCheck::NoSnapshot;
    };
    if would_cross_book(side, limit_price, best_bid, best_ask) {
        PostOnlyCheck::WouldCross
    } else {
        PostOnlyCheck::Rests
    }
}

// ============================================================
// ============== Mode B signed-order simulation Mode B 签名订单模拟 =================
// ============================================================

/// Mode B 签单路径的返回结果。即便没有真实 CLOB SDK，
/// 我们也从 args 派生出确定的伪 `tx_hash`，以便填充
/// 审计日志和 `bets.tx_hash` 列。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedOrderResult {
    pub tx_hash: String,
    pub signed_at_ms: i64,
    pub shares: String,
    /// 始终为 "submitted" —— v0.5 中我们拿不到链上确认。
    pub status: &'static str,
}

/// 「签」Mode B 订单。v0.5 中的确定性存根 —— `rs-clob-client`
/// 接入后，将替换本函数主体。
pub fn sign_order(args: &PlaceArgs, now_ms: i64) -> AppResult<SignedOrderResult> {
    let size = validate_place_args(args)?;
    if args.key_alias.as_deref().unwrap_or("").is_empty() {
        return Err(AppError::Invalid("key_alias required for mode B".into()));
    }
    // 从规范化 args 哈希得到确定性伪 tx-hash。
    // 使用简单的 djb2 哈希 → 十六进制。（非密码学安全；仅要求稳定。）
    let canonical = format!(
        "{}|{}|{}|{}|{}",
        args.market_id,
        args.side.as_str(),
        args.size_usdc,
        args.price,
        args.key_alias.as_deref().unwrap_or(""),
    );
    let h = djb2(canonical.as_bytes());
    let tx_hash = format!("0x{:016x}{:016x}{:016x}{:016x}", h, h.rotate_left(13), h.rotate_left(26), h);
    let shares = shares_for_size(&args.size_usdc, args.price);
    Ok(SignedOrderResult {
        tx_hash,
        signed_at_ms: now_ms,
        shares,
        status: "submitted",
    })
}

fn djb2(bytes: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for b in bytes {
        h = h.wrapping_mul(33).wrapping_add(*b as u64);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_round_trip() {
        for m in [BetMode::AJump, BetMode::BSigned] {
            assert_eq!(BetMode::parse(m.as_str()).unwrap(), m);
        }
    }

    #[test]
    fn status_round_trip() {
        for s in [BetStatus::Open, BetStatus::Won, BetStatus::Lost, BetStatus::Cancelled] {
            assert_eq!(BetStatus::parse(s.as_str()).unwrap(), s);
        }
    }

    #[test]
    fn mode_rejects_unknown() {
        assert!(BetMode::parse("C_signed").is_err());
    }

    #[test]
    fn side_round_trip_case_insensitive() {
        assert_eq!(BetSide::parse("yes").unwrap(), BetSide::Yes);
        assert_eq!(BetSide::parse("NO").unwrap(), BetSide::No);
        assert!(BetSide::parse("maybe").is_err());
    }

    #[test]
    fn shares_for_size_basic() {
        let s = shares_for_size("100", 0.5);
        assert_eq!(s, "200");
    }

    #[test]
    fn shares_for_size_zero_price() {
        assert_eq!(shares_for_size("100", 0.0), "0");
    }

    #[test]
    fn pnl_yes_bet_wins() {
        // 以 0.40 买入 100 USDC 的 YES → 若结算为 YES：pnl = (1-0.40) * 250 = 150
        let p = pnl_on_yes(BetSide::Yes, 250.0, 0.40, 100.0);
        assert!((p - 150.0).abs() < 1e-6);
    }

    #[test]
    fn pnl_yes_bet_loses() {
        // 若 YES 结算为 NO，则损失全部 size
        let p = pnl_on_no(BetSide::Yes, 250.0, 0.40, 100.0);
        assert!((p - -100.0).abs() < 1e-6);
    }

    #[test]
    fn pnl_no_bet_wins() {
        // 以 0.60 买入 100 USDC 的 NO → 若结算为 NO：pnl = 100/0.60 * 0.60 = 100
        let shares = 100.0 / 0.60;
        let p = pnl_on_no(BetSide::No, shares, 0.60, 100.0);
        assert!((p - 100.0).abs() < 1e-6);
    }

    #[test]
    fn is_open_only_for_open_status() {
        assert!(is_open(BetStatus::Open));
        assert!(!is_open(BetStatus::Won));
        assert!(!is_open(BetStatus::Lost));
        assert!(!is_open(BetStatus::Cancelled));
    }

    fn args(market: &str, size: &str, price: f64, key: Option<&str>) -> PlaceArgs {
        PlaceArgs {
            market_id: market.into(),
            side: BetSide::Yes,
            size_usdc: size.into(),
            price,
            key_alias: key.map(String::from),
            order_type: OrderType::Market,
            limit_price: None,
            stop_price: None,
            post_only: false,
        }
    }

    #[test]
    fn validate_place_args_ok() {
        let r = validate_place_args(&args("m1", "100", 0.5, Some("primary")));
        assert!(r.is_ok());
        assert!((r.unwrap() - 100.0).abs() < 1e-9);
    }

    #[test]
    fn validate_place_args_rejects_empty_market() {
        assert!(validate_place_args(&args("", "100", 0.5, Some("k"))).is_err());
    }

    #[test]
    fn validate_place_args_rejects_bad_price() {
        assert!(validate_place_args(&args("m", "100", 1.5, Some("k"))).is_err());
        assert!(validate_place_args(&args("m", "100", 0.0, Some("k"))).is_err());
        assert!(validate_place_args(&args("m", "100", f64::NAN, Some("k"))).is_err());
    }

    #[test]
    fn validate_place_args_rejects_bad_size() {
        assert!(validate_place_args(&args("m", "abc", 0.5, Some("k"))).is_err());
        assert!(validate_place_args(&args("m", "-5", 0.5, Some("k"))).is_err());
        assert!(validate_place_args(&args("m", "0", 0.5, Some("k"))).is_err());
    }

    #[test]
    fn validate_place_args_caps_max_size() {
        assert!(validate_place_args(&args("m", "50000", 0.5, Some("k"))).is_err());
    }

    #[test]
    fn sign_order_requires_key_alias() {
        let r = sign_order(&args("m1", "100", 0.5, None), 1_000_000);
        assert!(r.is_err());
    }

    #[test]
    fn sign_order_produces_stable_tx_hash() {
        let a = sign_order(&args("m1", "100", 0.5, Some("primary")), 1_000_000).unwrap();
        let b = sign_order(&args("m1", "100", 0.5, Some("primary")), 1_000_000).unwrap();
        assert_eq!(a.tx_hash, b.tx_hash);
        assert!(a.tx_hash.starts_with("0x"));
        assert_eq!(a.status, "submitted");
    }

    #[test]
    fn sign_order_different_args_produce_different_hash() {
        let a = sign_order(&args("m1", "100", 0.5, Some("primary")), 1_000_000).unwrap();
        let b = sign_order(&args("m2", "100", 0.5, Some("primary")), 1_000_000).unwrap();
        assert_ne!(a.tx_hash, b.tx_hash);
    }

    #[test]
    fn sign_order_computes_shares() {
        let r = sign_order(&args("m1", "100", 0.4, Some("k")), 0).unwrap();
        // shares = 100 / 0.4 = 250
        assert_eq!(r.shares, "250");
    }

    // ----- v0.50a —— OrderType 订单类型 -----

    fn order_args(
        market: &str,
        size: &str,
        price: f64,
        order_type: OrderType,
        limit_price: Option<f64>,
        stop_price: Option<f64>,
        post_only: bool,
    ) -> PlaceArgs {
        PlaceArgs {
            market_id: market.into(),
            side: BetSide::Yes,
            size_usdc: size.into(),
            price,
            key_alias: Some("primary".into()),
            order_type,
            limit_price,
            stop_price,
            post_only,
        }
    }

    #[test]
    fn order_type_round_trip() {
        for t in [OrderType::Market, OrderType::Limit, OrderType::StopLoss] {
            assert_eq!(OrderType::parse(t.as_str()).unwrap(), t);
        }
    }

    #[test]
    fn order_type_default_is_market() {
        assert_eq!(OrderType::default(), OrderType::Market);
    }

    #[test]
    fn order_type_rejects_unknown() {
        assert!(OrderType::parse("stop").is_err());
        assert!(OrderType::parse("").is_err());
    }

    #[test]
    fn market_order_rejects_limit_price() {
        let r = validate_place_args(&order_args(
            "m", "100", 0.5,
            OrderType::Market, Some(0.4), None, false,
        ));
        assert!(r.is_err(), "market + limit_price should be invalid: {r:?}");
    }

    #[test]
    fn market_order_rejects_stop_price() {
        let r = validate_place_args(&order_args(
            "m", "100", 0.5,
            OrderType::Market, None, Some(0.7), false,
        ));
        assert!(r.is_err(), "market + stop_price should be invalid: {r:?}");
    }

    #[test]
    fn limit_order_requires_limit_price() {
        let r = validate_place_args(&order_args(
            "m", "100", 0.5,
            OrderType::Limit, None, None, false,
        ));
        assert!(r.is_err());
        assert!(format!("{r:?}").contains("limit_price"));
    }

    #[test]
    fn limit_order_accepts_valid_limit_price() {
        let r = validate_place_args(&order_args(
            "m", "100", 0.5,
            OrderType::Limit, Some(0.45), None, false,
        ));
        assert!(r.is_ok(), "got: {r:?}");
    }

    #[test]
    fn limit_order_rejects_out_of_range_limit_price() {
        let r = validate_place_args(&order_args(
            "m", "100", 0.5,
            OrderType::Limit, Some(1.5), None, false,
        ));
        assert!(r.is_err());
    }

    #[test]
    fn limit_order_rejects_stop_price() {
        let r = validate_place_args(&order_args(
            "m", "100", 0.5,
            OrderType::Limit, Some(0.4), Some(0.7), false,
        ));
        assert!(r.is_err(), "limit + stop_price should be invalid: {r:?}");
    }

    #[test]
    fn stop_loss_requires_stop_price() {
        let r = validate_place_args(&order_args(
            "m", "100", 0.5,
            OrderType::StopLoss, None, None, false,
        ));
        assert!(r.is_err());
        assert!(format!("{r:?}").contains("stop_price"));
    }

    #[test]
    fn stop_loss_accepts_with_stop_and_limit() {
        let r = validate_place_args(&order_args(
            "m", "100", 0.5,
            OrderType::StopLoss, Some(0.45), Some(0.6), false,
        ));
        assert!(r.is_ok(), "got: {r:?}");
    }

    #[test]
    fn stop_loss_defaults_limit_to_stop_when_only_stop_given() {
        let r = validate_place_args(&order_args(
            "m", "100", 0.5,
            OrderType::StopLoss, None, Some(0.6), false,
        ));
        assert!(r.is_ok(), "got: {r:?}");
    }

    #[test]
    fn post_only_only_valid_for_limit() {
        // post_only + Market -> 错误
        let r = validate_place_args(&order_args(
            "m", "100", 0.5,
            OrderType::Market, None, None, true,
        ));
        assert!(r.is_err(), "post_only + market should be invalid");
        // post_only + StopLoss -> 错误
        let r = validate_place_args(&order_args(
            "m", "100", 0.5,
            OrderType::StopLoss, None, Some(0.6), true,
        ));
        assert!(r.is_err(), "post_only + stop_loss should be invalid");
        // post_only + Limit -> 正确
        let r = validate_place_args(&order_args(
            "m", "100", 0.5,
            OrderType::Limit, Some(0.45), None, true,
        ));
        assert!(r.is_ok(), "got: {r:?}");
    }

    #[test]
    fn validate_order_type_specifics_is_pure_helper() {
        // 拆分出的纯辅助函数对订单类型相关字段的
        // 拒绝逻辑应与内联在 validate_place_args 中时一致。
        let a = order_args("m", "100", 0.5, OrderType::Limit, None, None, false);
        assert!(validate_order_type_specifics(&a).is_err());
        let b = order_args("m", "100", 0.5, OrderType::Limit, Some(0.4), None, false);
        assert!(validate_order_type_specifics(&b).is_ok());
    }

    // ----- v0.50b —— post-only 仅挂单 -----

    /// YES 以等于 best_ask 的限价买入时会穿越。
    #[test]
    fn would_cross_yes_at_ask() {
        assert!(would_cross_book(BetSide::Yes, 0.50, 0.49, 0.50));
    }

    /// YES 以 best_ask 下方一档的限价买入时挂单。
    #[test]
    fn would_not_cross_yes_below_ask() {
        assert!(!would_cross_book(BetSide::Yes, 0.49, 0.49, 0.50));
    }

    /// NO 以等于 (1 - best_bid) 的限价买入时会穿越。
    /// best_bid=0.40 → 1 - best_bid = 0.60 → 限价 0.60 时穿越。
    #[test]
    fn would_cross_no_at_implied_ask() {
        assert!(would_cross_book(BetSide::No, 0.60, 0.40, 0.50));
    }

    /// NO 以远高于隐含卖单的限价买入时挂单。
    /// NO 限价 0.55 < 0.60 = 1 - best_bid → 挂单。
    #[test]
    fn would_not_cross_no_above_implied_ask() {
        // 注意 —— NO 限价 >= 1-best_bid 时穿越。所以限价 0.55 且
        // best_bid=0.40 → 1-0.40=0.60；0.55 < 0.60 → 不会穿越。
        assert!(!would_cross_book(BetSide::No, 0.55, 0.40, 0.50));
    }

    #[test]
    fn check_post_only_no_snapshot_is_silent_pass() {
        // 当前：缺失快照 = 不强制。
        let r = check_post_only(BetSide::Yes, 0.99, None);
        assert_eq!(r, PostOnlyCheck::NoSnapshot);
    }

    #[test]
    fn check_post_only_yes_rests_below_ask() {
        // bid=0.40, ask=0.50；YES 限价 0.45 < 0.50 → 挂单。
        let r = check_post_only(BetSide::Yes, 0.45, Some((0.40, 0.50)));
        assert_eq!(r, PostOnlyCheck::Rests);
    }

    #[test]
    fn check_post_only_yes_crosses_at_ask() {
        let r = check_post_only(BetSide::Yes, 0.50, Some((0.40, 0.50)));
        assert_eq!(r, PostOnlyCheck::WouldCross);
    }

    #[test]
    fn check_post_only_no_crosses_at_implied_ask() {
        // bid=0.40, ask=0.50；NO 限价 0.60 >= 1-0.40 = 0.60 → 穿越。
        let r = check_post_only(BetSide::No, 0.60, Some((0.40, 0.50)));
        assert_eq!(r, PostOnlyCheck::WouldCross);
    }

    #[test]
    fn check_post_only_no_rests_above_implied_ask() {
        // NO 限价 0.50 < 1-0.40 = 0.60 → 挂单。
        let r = check_post_only(BetSide::No, 0.50, Some((0.40, 0.50)));
        assert_eq!(r, PostOnlyCheck::Rests);
    }

    #[test]
    fn validate_place_args_with_post_only_and_limit_accepts_syntax() {
        // v0.50b —— post_only 是 Limit 订单的合法 SYNTAX 标志。
        // 实际的 book-cross 检查在 place_signed_order 中
        // 查快照后进行。本测试仅断言校验器不会因
        // 组合语法原因拒绝。
        let r = order_args("m", "100", 0.5, OrderType::Limit, Some(0.45), None, true);
        assert!(validate_place_args(&r).is_ok());
    }
}
