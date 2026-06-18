//! L3 — Bet lifecycle.
//!
//! Owns the state machine for `bets` rows: `open` → `won` / `lost` /
//! `cancelled`. Provides a single `place()` entry that the L2 command
//! layer can call; signed order placement (mode B) and jump-link
//! construction (mode A) live here.
//!
//! **Status (v0.3c): stub.** The current `commands::bet` module has
//! the working SQL — it will be migrated into the helpers below as
//! M6 "Bet placement" milestone progresses.

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
    /// Mode A: jump-link only. User signs on Polymarket UI.
    AJump,
    /// Mode B: signed order via OS keyring.
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
// Polymarket supports Market (immediate execution
// at best available price) and Limit (only fill at
// `limit_price` or better). For v0.50 we also add
// StopLoss (trigger when market crosses `stop_price`)
// as a UX primitive — the underlying CLOB call is
// still a Limit order placed when the trigger fires.
//
// PostOnly (v0.50b) is a flag, not a type, on Limit
// orders: the order must rest on the book, never
// take liquidity.

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
    /// Fill at best available price. `limit_price` is ignored.
    #[default]
    Market,
    /// Rest on the book; only fill at `limit_price` or better.
    Limit,
    /// Trigger when market crosses `stop_price`, then submit
    /// as a Limit at `limit_price` (default = stop_price).
    /// Today this is captured in the order record but the
    /// actual trigger is v0.51+ (requires real CLOB feed).
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
// ============== PnL math (pure) ==============================
// ============================================================

/// Number of shares bought = USDC size / price.
/// Returns 0 if size or price is non-positive or unparseable.
pub fn shares_for_size(size_usdc: &str, price: f64) -> String {
    match size_usdc.parse::<f64>() {
        Ok(s) if s > 0.0 && price > 0.0 => (s / price).to_string(),
        _ => "0".to_string(),
    }
}

/// PnL when the market resolves YES.
/// For a YES bet: pnl = shares * (1 - price)  (you bought at `price`,
/// the share pays $1 on YES).
/// For a NO bet: pnl = -size  (NO share worthless).
pub fn pnl_on_yes(side: BetSide, shares: f64, price: f64, size_usdc: f64) -> f64 {
    match side {
        BetSide::Yes => shares * (1.0 - price),
        BetSide::No => -size_usdc,
    }
}

/// PnL when the market resolves NO.
/// For a YES bet: pnl = -size.
/// For a NO bet: pnl = shares * price  (NO share pays $1).
pub fn pnl_on_no(side: BetSide, shares: f64, price: f64, size_usdc: f64) -> f64 {
    match side {
        BetSide::Yes => -size_usdc,
        BetSide::No => shares * price,
    }
}

/// Compute PnL given the resolved outcome.
pub fn pnl(outcome: BetSide, side: BetSide, shares: f64, price: f64, size_usdc: f64) -> f64 {
    match outcome {
        BetSide::Yes => pnl_on_yes(side, shares, price, size_usdc),
        BetSide::No => pnl_on_no(side, shares, price, size_usdc),
    }
}

/// Whether a bet in this status is still "active" (excluded from PnL).
pub fn is_open(s: BetStatus) -> bool {
    matches!(s, BetStatus::Open)
}

// ============================================================
// ============== Args validation ==============================
// ============================================================

/// Maximum size in USDC (defensive cap to prevent typos like
/// "10000" instead of "100").
pub const MAX_BET_SIZE_USDC: f64 = 10_000.0;
/// Minimum price (1¢) and maximum (99¢) — anything outside is invalid
/// for a real CLOB market.
pub const MIN_PRICE: f64 = 0.01;
pub const MAX_PRICE: f64 = 0.99;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaceArgs {
    pub market_id: String,
    pub side: BetSide,
    pub size_usdc: String,
    /// The "reference" price — what the user thinks the share
    /// is worth right now. For Market orders this is unused;
    /// for Limit orders it must equal `limit_price` (we store
    /// the user's "expectation" in `price` for analytics); for
    /// StopLoss it is the entry price the user wants.
    pub price: f64,
    pub key_alias: Option<String>,
    /// v0.50a — order type. Default = Market (back-compat).
    #[serde(default)]
    pub order_type: OrderType,
    /// v0.50a — for Limit orders: only fill at this price or
    /// better. Required when `order_type = Limit`. Optional
    /// for StopLoss (defaults to `stop_price`).
    #[serde(default)]
    pub limit_price: Option<f64>,
    /// v0.50a — for StopLoss orders: trigger when market
    /// crosses this price (in the direction opposite to the
    /// desired position). Required when `order_type = StopLoss`.
    #[serde(default)]
    pub stop_price: Option<f64>,
    /// v0.50b — for Limit orders: must rest on book, never
    /// take liquidity. Ignored for Market and StopLoss.
    #[serde(default)]
    pub post_only: bool,
}

/// Validate a `place_*` args payload. Returns parsed size on success.
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
    // v0.50a — order-type-specific validation.
    validate_order_type_specifics(args)?;
    Ok(size)
}

/// v0.50a — additional validation that depends only on the
/// order type fields. Split out so the L1 can call it
/// independently for "preflight" checks before submitting.
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
            // StopLoss trigger direction:
            //   YES bet → trigger when price RISES to stop_price
            //     (you want to cap loss if market moves against you,
            //     so stop_price should be > price)
            //   NO bet  → trigger when price FALLS to stop_price
            //     (stop_price should be < price)
            // We don't ENFORCE the relationship (v0.50a is just
            // capturing intent) but we record it.
            let _ = args.limit_price.unwrap_or(sp); // default = stop_price
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
// ============== v0.50b — post-only enforcement ==============
// ============================================================
//
// Polymarket CLOB has the standard "post-only" semantics:
// the order must rest on the book, never take liquidity.
// We implement the check using the latest price_snapshots
// row (v0.47a) for the market.
//
// Until v0.51+ brings a real order-book feed, the
// snapshot is a placeholder (best_bid = best_ask = 0.5).
// In that case `would_cross_book` returns false (it
// always rests), so post-only is effectively a no-op
// for new installs. This is acceptable: v0.50b is
// about getting the validation plumbing right so the
// moment a real feed lands, enforcement is automatic.
//
// The book snapshot here is the YES-token view; for
// NO bets we compute the implied YES price as
// `1 - limit_price` and compare against `best_bid`.

/// v0.50b — pure helper. Given the side, the limit
/// price, and the latest book snapshot, returns
/// `true` when the order would take liquidity
/// (and thus must be rejected under post-only).
///
/// The snapshot represents the YES token's order
/// book: `best_bid` and `best_ask` are YES-token
/// prices in [0.01, 0.99].
pub fn would_cross_book(
    side: BetSide,
    limit_price: f64,
    best_bid: f64,
    best_ask: f64,
) -> bool {
    match side {
        // Buying YES at limit P: takes liquidity when
        // P >= best_ask (you'd match the ask).
        BetSide::Yes => limit_price >= best_ask,
        // Buying NO at limit P: NO token price = 1 - YES_price.
        // Equivalent: takes liquidity when
        // (1 - P) <= best_bid
        // i.e. P >= 1 - best_bid
        BetSide::No => limit_price >= 1.0 - best_bid,
    }
}

/// v0.50b — outcome of the post-only enforcement
/// check. We return a typed result so callers can
/// distinguish "would cross" from "snapshot missing"
/// (which is currently a silent pass — see the
/// module docs).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostOnlyCheck {
    /// No snapshot for this market yet; post-only
    /// is a no-op (real enforcement defers to v0.51+).
    NoSnapshot,
    /// Snapshot exists; order rests on book. OK.
    Rests,
    /// Snapshot exists; order would cross. REJECT.
    WouldCross,
}

/// v0.50b — given a snapshot (or None) and the
/// post-only flag, return the enforcement outcome.
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
// ============== Mode B signed-order simulation =================
// ============================================================

/// Outcome of the mode B signing path. Even without a real CLOB SDK,
/// we deterministically derive a fake `tx_hash` from the args so the
/// audit log and `bets.tx_hash` column are populated.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedOrderResult {
    pub tx_hash: String,
    pub signed_at_ms: i64,
    pub shares: String,
    /// Always "submitted" — we never get on-chain confirmation in v0.5.
    pub status: &'static str,
}

/// "Sign" a mode B order. Deterministic stub for v0.5 — when
/// `rs-clob-client` lands, replace the body of this function.
pub fn sign_order(args: &PlaceArgs, now_ms: i64) -> AppResult<SignedOrderResult> {
    let size = validate_place_args(args)?;
    if args.key_alias.as_deref().unwrap_or("").is_empty() {
        return Err(AppError::Invalid("key_alias required for mode B".into()));
    }
    // Deterministic pseudo-tx-hash from a hash of the canonical args.
    // We use a simple djb2 hash → hex. (Not cryptographic; just stable.)
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
        // Buy 100 YES at 0.40 → if resolves YES: pnl = (1-0.40) * 250 = 150
        let p = pnl_on_yes(BetSide::Yes, 250.0, 0.40, 100.0);
        assert!((p - 150.0).abs() < 1e-6);
    }

    #[test]
    fn pnl_yes_bet_loses() {
        // If YES resolves NO, full size is lost
        let p = pnl_on_no(BetSide::Yes, 250.0, 0.40, 100.0);
        assert!((p - -100.0).abs() < 1e-6);
    }

    #[test]
    fn pnl_no_bet_wins() {
        // Buy 100 NO at 0.60 → if resolves NO: pnl = 100/0.60 * 0.60 = 100
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

    // ----- v0.50a — OrderType -----

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
        // post_only on Market -> error
        let r = validate_place_args(&order_args(
            "m", "100", 0.5,
            OrderType::Market, None, None, true,
        ));
        assert!(r.is_err(), "post_only + market should be invalid");
        // post_only on StopLoss -> error
        let r = validate_place_args(&order_args(
            "m", "100", 0.5,
            OrderType::StopLoss, None, Some(0.6), true,
        ));
        assert!(r.is_err(), "post_only + stop_loss should be invalid");
        // post_only on Limit -> ok
        let r = validate_place_args(&order_args(
            "m", "100", 0.5,
            OrderType::Limit, Some(0.45), None, true,
        ));
        assert!(r.is_ok(), "got: {r:?}");
    }

    #[test]
    fn validate_order_type_specifics_is_pure_helper() {
        // The split-out helper should reject the same
        // things as the integrated validate_place_args
        // for order-type-specific fields.
        let a = order_args("m", "100", 0.5, OrderType::Limit, None, None, false);
        assert!(validate_order_type_specifics(&a).is_err());
        let b = order_args("m", "100", 0.5, OrderType::Limit, Some(0.4), None, false);
        assert!(validate_order_type_specifics(&b).is_ok());
    }

    // ----- v0.50b — post-only -----

    /// YES buy at limit equal to best_ask crosses.
    #[test]
    fn would_cross_yes_at_ask() {
        assert!(would_cross_book(BetSide::Yes, 0.50, 0.49, 0.50));
    }

    /// YES buy at limit one tick below best_ask rests.
    #[test]
    fn would_not_cross_yes_below_ask() {
        assert!(!would_cross_book(BetSide::Yes, 0.49, 0.49, 0.50));
    }

    /// NO buy at limit equal to (1 - best_bid) crosses.
    /// best_bid=0.40 → 1 - best_bid = 0.60 → limit at 0.60 crosses.
    #[test]
    fn would_cross_no_at_implied_ask() {
        assert!(would_cross_book(BetSide::No, 0.60, 0.40, 0.50));
    }

    /// NO buy at limit well above implied ask rests.
    /// NO limit at 0.55 < 0.60 = 1 - best_bid → rests.
    #[test]
    fn would_not_cross_no_above_implied_ask() {
        // wait — NO limit >= 1-best_bid crosses. So limit 0.55 with
        // best_bid=0.40 → 1-0.40=0.60; 0.55 < 0.60 → does NOT cross.
        assert!(!would_cross_book(BetSide::No, 0.55, 0.40, 0.50));
    }

    #[test]
    fn check_post_only_no_snapshot_is_silent_pass() {
        // Today: missing snapshot = no enforcement.
        let r = check_post_only(BetSide::Yes, 0.99, None);
        assert_eq!(r, PostOnlyCheck::NoSnapshot);
    }

    #[test]
    fn check_post_only_yes_rests_below_ask() {
        // bid=0.40, ask=0.50; YES limit 0.45 < 0.50 → rests.
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
        // bid=0.40, ask=0.50; NO limit 0.60 >= 1-0.40 = 0.60 → crosses.
        let r = check_post_only(BetSide::No, 0.60, Some((0.40, 0.50)));
        assert_eq!(r, PostOnlyCheck::WouldCross);
    }

    #[test]
    fn check_post_only_no_rests_above_implied_ask() {
        // NO limit 0.50 < 1-0.40 = 0.60 → rests.
        let r = check_post_only(BetSide::No, 0.50, Some((0.40, 0.50)));
        assert_eq!(r, PostOnlyCheck::Rests);
    }

    #[test]
    fn validate_place_args_with_post_only_and_limit_accepts_syntax() {
        // v0.50b — post_only is a SYNTAX-valid flag for limit
        // orders. The actual book-cross check happens in
        // place_signed_order after looking up the snapshot.
        // Here we only assert that the validator doesn't
        // reject the combination on syntactic grounds.
        let r = order_args("m", "100", 0.5, OrderType::Limit, Some(0.45), None, true);
        assert!(validate_place_args(&r).is_ok());
    }
}
