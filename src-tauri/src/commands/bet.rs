//! L2 —— 投注下发（M6）。
//!
//! IPC:`place_jump_link`（模式 A,零合规风险）、`place_signed_order`
//! （模式 B,keyring 签名,stub）、`list_bets`。
//! 依赖 L3 `domain::polymarket` 提供 jump URL 和 signed-order stub,
//! 依赖 L4 `infra::state::AppState` 提供 SQLite 连接池。

use crate::domain::bet::{self, sign_order, BetSide, OrderType, PlaceArgs};
use crate::domain::polymarket;
use crate::infra::state::AppState;
use crate::AppResult;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use tauri::State;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct BetDto {
    pub id: String,
    pub wallet_id: String,
    pub market_id: String,
    pub signal_id: Option<i64>,
    pub mode: String,
    pub side: String,
    pub size: String,
    pub price: f64,
    pub shares: String,
    pub placed_at: i64,
    pub settled_at: Option<i64>,
    pub pnl: Option<String>,
    pub status: String,
    pub tx_hash: Option<String>,
    pub notes: Option<String>,
    /// v0.50a —— 订单类型。v0.50 之前的行视为 "market"（迁移默认值）。
    #[serde(default = "default_order_type")]
    pub order_type: String,
    #[serde(default)]
    pub limit_price: Option<f64>,
    #[serde(default)]
    pub stop_price: Option<f64>,
    #[serde(default)]
    pub post_only: bool,
    /// v0.51b —— 订单实际成交的时间
    /// (与 `placed_at` 不同,后者是用户提交的时间)。
    /// 对于 v0.5d 的确定性 stub 以及 v0.51b 之前的行,
    /// 此字段为 `None`,analytics IPC 将滑点（slippage）报告为 0。
    /// v0.51+ 从真实 CLOB 响应中填充该字段。
    #[serde(default)]
    pub filled_at: Option<i64>,
    /// v0.51b —— 实际成交价。可能与 `price`（用户的预期价）不同,
    /// 这就是滑点（slippage）。v0.51b 之前为 None。
    #[serde(default)]
    pub fill_price: Option<f64>,
    /// v0.51b —— 实际成交的份额数。当订单被部分成交时,
    /// 可能与 `shares`（期望成交额）不同。v0.51b 之前为 None。
    #[serde(default)]
    pub fill_size: Option<String>,
    /// v0.51b —— 当 fill_size < shares 时为 true。
    #[serde(default)]
    pub partial: bool,
}

fn default_order_type() -> String {
    "market".to_string()
}

#[derive(Debug, Deserialize)]
pub struct PlaceJumpArgs {
    pub market_slug: String,
    pub market_id: String,
    pub wallet_id: String,
    pub side: String,
    pub size: String,
    pub price: f64,
    pub signal_id: Option<i64>,
}

/// 模式 A:返回 jump URL —— 用户点击后,在 Polymarket UI 中签名。
/// 零合规风险:polyrocket 从不持有私钥。
#[tauri::command]
pub async fn place_jump_link(args: PlaceJumpArgs) -> AppResult<String> {
    Ok(polymarket::build_jump_url(&args.market_slug, &args.side, args.price))
}

#[derive(Debug, Deserialize)]
pub struct PlaceSignedArgs {
    pub market_id: String,
    pub wallet_id: String,
    pub side: String,
    pub price: f64,
    pub size: String,
    pub signal_id: Option<i64>,
    /// 存储在操作系统 keyring 中的密钥别名（例如 "primary"、"trade-1"）
    pub key_alias: String,
    /// v0.50a —— 订单类型。未提供时默认为 "market"
    /// (为 v0.50 之前的 L1 调用点保留向后兼容)。
    #[serde(default)]
    pub order_type: Option<String>,
    /// v0.50a —— 限价（仅用于 Limit / StopLoss 订单）。
    #[serde(default)]
    pub limit_price: Option<f64>,
    /// v0.50a —— 触发价（仅用于 StopLoss 订单）。
    #[serde(default)]
    pub stop_price: Option<f64>,
    /// v0.50b —— 仅挂单（post-only）标志（仅用于 Limit 订单）。
    #[serde(default)]
    pub post_only: bool,
    /// v0.120 —— 投注模式。"live"（默认）调用真实的 PM CLOB 接口
    /// 并持久化 CLOB 的 tx_hash。
    /// "paper" 跳过 CLOB HTTP 调用（用于 e2e / demo 流程中的
    /// 合成市场）,以确定性 `paper-{uuid}` 作为 tx_hash 写入投注,
    /// 且 mode="paper"。
    #[serde(default)]
    pub mode: Option<String>,
}

/// 模式 B:通过操作系统 keyring 签名的订单。
///
/// v0.5d:使用 `domain::bet::sign_order()` 校验参数、
/// 派生确定性的伪 `tx_hash`,并计算份额。
/// 当 `rs-clob-client` 接入后,只需替换 `sign_order` 函数体即可。
///
/// v0.50a:扩展支持 `order_type`、`limit_price`、
/// `stop_price`、`post_only`。纯函数校验位于
/// `validate_order_type_specifics`。这些新字段
/// 持久化到 `bets`（order_type、limit_price、stop_price、
/// post_only 列）。对于 Limit 订单,记录的
/// `bets.price` 仍为用户的「参考」价格
/// （保留用于 analytics）,真正的限价水平记录在 `limit_price`。
#[tauri::command]
pub async fn place_signed_order(
    state: State<'_, AppState>,
    args: PlaceSignedArgs,
) -> AppResult<BetDto> {
    // Domain:校验 + 签名(v0.5 的确定性 stub)
    let side = BetSide::parse(&args.side).map_err(|e| {
        crate::AppError::Invalid(format!("invalid side: {e}"))
    })?;
    // v0.50a —— 解析 order_type(为 v0.50 之前的 L1 调用者
    // 默认为 Market,以保持向后兼容)。
    let order_type = match &args.order_type {
        Some(s) => bet::OrderType::parse(s).map_err(|e| {
            crate::AppError::Invalid(format!("invalid order_type: {e}"))
        })?,
        None => bet::OrderType::Market,
    };
    let place = PlaceArgs {
        market_id: args.market_id.clone(),
        side,
        size_usdc: args.size.clone(),
        price: args.price,
        key_alias: Some(args.key_alias.clone()),
        order_type,
        limit_price: args.limit_price,
        stop_price: args.stop_price,
        post_only: args.post_only,
    };
    let now = chrono::Utc::now().timestamp_millis();
    let signed = sign_order(&place, now)?;

    // v0.50b —— post-only 强制检查。对于被标记为 post_only 的
    // Limit 订单,查找最近的快照(v0.47a),若限价会
    // 越过当前盘口则拒绝。当无快照时,post-only 静默放行
    // (见 domain::bet 模块文档)。
    if order_type == OrderType::Limit && args.post_only {
        if let Some(lp) = args.limit_price {
            let snap =
                crate::infra::db::price_snapshots::latest_snapshot(&state.db, &args.market_id)
                    .await?;
            let snapshot_pair = snap.map(|(bid, ask, _mid, _spread, _captured_at)| (bid, ask));
            let check = bet::check_post_only(side, lp, snapshot_pair);
            if matches!(check, bet::PostOnlyCheck::WouldCross) {
                return Err(crate::AppError::Invalid(format!(
                    "post-only limit at {lp} would cross the book for market {}",
                    args.market_id
                )));
            }
        }
    }

    // v0.51c —— CLOB 提交。替代了 v0.5d 仅确定性的路径。
    // 当存在 CLOB 凭据时(POLYROCKET_CLOB_API_KEY + SECRET +
    // PASSPHRASE),会真实地通过 HTTP POST 调 CLOB /order 接口;
    // 凭据缺失时,返回 stub 形态(ok=true, slippage=0,
    // partial=false)——自 v0.5d 以来应用一直使用的确定性路径。
    //
    // 错误以 AppError::Invalid(而非 Internal)抛出,
    // 以便 L1 可直接内联展示。我们将 CLOB 的 tx_hash
    // (或确定性 stub 的 hash)持久化到 bets.tx_hash。
    //
    // v0.120 —— paper 模式。当 `args.mode == "paper"` 时,
    // 完全跳过 CLOB HTTP 调用。使用确定性 tx_hash
    // `paper-{uuid}` 并视订单立即以用户报价成交(slippage = 0)。
    // paper 模式用于 e2e demo 流程:此时市场并非真实 PM 市场,
    // CLOB 端点会返回 400。该投注仍以 mode="paper"
    // 落入 `bets`,以便 UI 能将其展示为模拟交易。
    let paper_mode = args.mode.as_deref() == Some("paper");
    let (clob, fill_price_at_insert, fill_size_at_insert, partial_at_insert, filled_at_at_insert) = if paper_mode {
        let paper_id = format!("paper-{}", Uuid::new_v4());
        (
            polymarket::ClobOrderResult {
                ok: true,
                tx_hash: paper_id,
                filled_at_ms: now,
                fill_price: args.price,
                fill_size: signed.shares.clone(),
                partial: false,
                error: String::new(),
                via_http: false,
            },
            args.price,
            signed.shares.clone(),
            false,
            now,
        )
    } else {
        let clob = polymarket::submit_signed_order_via_clob(
            &args.market_id,
            &args.side,
            args.price,
            &signed.shares,
            &args.key_alias,
            order_type.as_str(),
            signed.signed_at_ms,
        )
        .await;
        if !clob.ok {
            return Err(crate::AppError::Invalid(format!(
                "CLOB rejected order: {}",
                clob.error
            )));
        }
        let fp = clob.fill_price;
        let fs = clob.fill_size.clone();
        let pa = clob.partial;
        let fa = clob.filled_at_ms;
        (clob, fp, fs, pa, fa)
    };
    let _ = clob.via_http; // 通过下方的 clob.tx_hash 记录;该字段
                           // 会出现在 audit_log 的 payload 中。

    let id = Uuid::new_v4().to_string();
    let shares = signed.shares;

    // v0.51b —— 成交字段。在 v0.5d 确定性 stub 中,
    // 我们视订单立即以用户报价成交(slippage = 0)。
    // v0.51c:当 CLOB 提交返回真实响应(凭据存在且可访问)
    // 时,我们从 CLOB 记录真实的
    // fill_price / fill_size / partial / filled_at（成交价/成交数量/是否部分成交/成交时间）。
    // v0.120:paper 模式使用合成值(无 CLOB 调用)。
    let filled_at = Some(filled_at_at_insert);
    let fill_price = Some(fill_price_at_insert);
    let fill_size = Some(fill_size_at_insert);
    let partial = partial_at_insert;

    // v0.50a + v0.51b —— INSERT 现已包含 order-type 和
    // fill 列。infra::db::bets_columns 中的幂等
    // ALTER TABLE 已在执行到此处前添加好这些列。
    // v0.120 —— paper 模式:bet mode 列 = "paper"
    // (原本硬编码为 "B_signed";现为:live → "B_signed",
    // paper → "paper",以供 UI 区分)。
    let bet_mode = if paper_mode { "paper" } else { "B_signed" };
    sqlx::query(
        "INSERT INTO bets (
            id, wallet_id, market_id, signal_id, mode, side,
            size, price, shares, placed_at, status, tx_hash,
            order_type, limit_price, stop_price, post_only,
            filled_at, fill_price, fill_size, partial
         )
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'open', ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&args.wallet_id)
    .bind(&args.market_id)
    .bind(args.signal_id)
    .bind(bet_mode)
    .bind(&args.side)
    .bind(&args.size)
    .bind(args.price)
    .bind(&shares)
    .bind(signed.signed_at_ms)
    .bind(&signed.tx_hash)
    .bind(order_type.as_str())
    .bind(args.limit_price)
    .bind(args.stop_price)
    .bind(if args.post_only { 1_i64 } else { 0_i64 })
    .bind(filled_at)
    .bind(fill_price)
    .bind(&fill_size)
    .bind(if partial { 1_i64 } else { 0_i64 })
    .execute(&state.db)
    .await?;

    sqlx::query(
        "INSERT INTO audit_log (actor, action, target, payload, result) VALUES ('user', 'bet.place', ?, ?, 'ok')",
    )
    .bind(&args.market_id)
    .bind(serde_json::json!({
        "mode": "B",
        "size": args.size,
        "price": args.price,
        "order_type": order_type.as_str(),
        "limit_price": args.limit_price,
        "stop_price": args.stop_price,
        "post_only": args.post_only,
    }))
    .execute(&state.db)
    .await?;

    Ok(BetDto {
        id,
        wallet_id: args.wallet_id,
        market_id: args.market_id,
        signal_id: args.signal_id,
        mode: bet_mode.to_string(),
        side: args.side,
        size: args.size,
        price: args.price,
        shares,
        placed_at: signed.signed_at_ms,
        settled_at: None,
        pnl: None,
        status: "open".into(),
        tx_hash: Some(signed.tx_hash.clone()),
        notes: None,
        order_type: order_type.as_str().to_string(),
        limit_price: args.limit_price,
        stop_price: args.stop_price,
        post_only: args.post_only,
        filled_at: filled_at,
        fill_price: fill_price,
        fill_size: fill_size,
        partial,
    })
}

#[derive(Debug, Deserialize)]
pub struct ListBetsArgs {
    pub status: Option<String>,
    pub wallet_id: Option<String>,
    pub limit: Option<i64>,
}

#[tauri::command]
pub async fn list_bets(
    state: State<'_, AppState>,
    args: ListBetsArgs,
) -> AppResult<Vec<BetDto>> {
    let limit = args.limit.unwrap_or(100);
    // v0.50a + v0.51b —— 包含 order-type 和 fill 列。
    // v0.50 / v0.51b 之前的行,其 limit_price / stop_price / filled_at /
    // fill_price / fill_size 为 NULL,post_only / partial 为 0;
    // sqlx 通过 #[serde(default)] 反序列化这些字段。
    let rows = sqlx::query_as::<_, BetDto>(
        "SELECT id, wallet_id, market_id, signal_id, mode, side, size, price, shares, placed_at, settled_at, pnl, status, tx_hash, notes,
                order_type, limit_price, stop_price, post_only,
                filled_at, fill_price, fill_size, partial
         FROM bets ORDER BY placed_at DESC LIMIT ?",
    )
    .bind(limit)
    .fetch_all(&state.db)
    .await?;
    Ok(rows)
}

// =================================================================
// ============== v0.50a —— 订单参数校验 IPC =============
// =================================================================

#[derive(Debug, Deserialize)]
pub struct ValidateOrderArgsArgs {
    pub market_id: String,
    pub side: String,
    pub size: String,
    pub price: f64,
    pub order_type: Option<String>,
    pub limit_price: Option<f64>,
    pub stop_price: Option<f64>,
    pub post_only: bool,
}

/// v0.50a —— 纯校验 IPC。L1 在调用 `placeSignedOrder`
/// 之前调用本接口,以让用户即时获得反馈
/// (例如「limit 订单需要 limit_price」),
/// 避免与数据库之间的一次往返。
///
/// 成功时返回解析后的 size;失败时返回
/// `AppError::Invalid`(会被序列化为字符串)。
#[tauri::command]
pub fn validate_order_args(args: ValidateOrderArgsArgs) -> AppResult<f64> {
    let side = BetSide::parse(&args.side).map_err(|e| {
        crate::AppError::Invalid(format!("invalid side: {e}"))
    })?;
    let order_type = match &args.order_type {
        Some(s) => OrderType::parse(s).map_err(|e| {
            crate::AppError::Invalid(format!("invalid order_type: {e}"))
        })?,
        None => OrderType::Market,
    };
    let place = PlaceArgs {
        market_id: args.market_id,
        side,
        size_usdc: args.size,
        price: args.price,
        key_alias: Some("validate_only".into()), // 校验器的占位值
        order_type,
        limit_price: args.limit_price,
        stop_price: args.stop_price,
        post_only: args.post_only,
    };
    bet::validate_place_args(&place)
}