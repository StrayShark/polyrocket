//! L2 —— Mirror executor IPC（M5 自动执行）。
//!
//! 管理 SQLite 中的 `copy_mirror_queue` 表，并运行
//! domain::mirror executor 来挑选哪些待处理 mirror
//! 作为 Mode B 投注提交。
//!
//! 流程：
//! 1. L4 scheduler（infra::scheduler）每 N 秒 tick 一次
//! 2. 调用 `run_mirror_executor_pass` IPC（或直接通过 Rust）
//! 3. 这一轮读取 pending mirror，运行 pick_next_mirror + find_rejections
//! 4. 被选中的 mirror 通过 sign_order 作为 Mode B 投注提交
//! 5. 被拒绝的 mirror 标记其原因

use crate::AppError;
use crate::AppResult;
use crate::domain::bet::sign_order;
use crate::domain::copy::{MirrorOrder, MirrorStatus};
use crate::domain::mirror::{
    current_exposure, execute_pass, ExecutorConfig, ExecutorPassResult, RejectReason,
};
use crate::infra::state::AppState;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Row};
use std::collections::HashMap;
use tauri::State;

/// 已持久化的 mirror 单（DB 行）。镜像 `copy_mirror_queue` 表结构。
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct MirrorRow {
    pub id: String,
    pub event_id: i64,
    pub target_id: String,
    pub market_id: String,
    pub side: String,
    pub size: String,
    pub flipped: bool,
    pub status: String,
    pub created_at: i64,
    pub submitted_at: Option<i64>,
    pub filled_at: Option<i64>,
    pub bet_id: Option<String>,
    pub reject_reason: Option<String>,
}

impl From<MirrorRow> for MirrorOrder {
    fn from(r: MirrorRow) -> Self {
        MirrorOrder {
            id: r.id,
            event_id: r.event_id,
            target_id: r.target_id,
            market_id: r.market_id,
            side: r.side,
            size: r.size,
            flipped: r.flipped,
            status: MirrorStatus::parse(&r.status).unwrap_or(MirrorStatus::Pending),
            created_at: r.created_at,
            submitted_at: r.submitted_at,
            filled_at: r.filled_at,
            bet_id: r.bet_id,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct EnqueueArgs {
    pub event_id: i64,
    pub target_id: String,
    pub market_id: String,
    pub side: String,
    pub size: String,
    pub flipped: bool,
}

/// 将新 mirror 入队（在 `should_mirror` 返回决策时调用）。
#[tauri::command]
pub async fn enqueue_mirror(
    state: State<'_, AppState>,
    args: EnqueueArgs,
) -> AppResult<MirrorRow> {
    let now = chrono::Utc::now().timestamp_millis();
    let id = format!("mir_{}_{}_{}", args.event_id, args.market_id, now);
    sqlx::query(
        "INSERT INTO copy_mirror_queue
            (id, event_id, target_id, market_id, side, size, flipped, status, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, 'pending', ?)
         ON CONFLICT(id) DO NOTHING",
    )
    .bind(&id)
    .bind(args.event_id)
    .bind(&args.target_id)
    .bind(&args.market_id)
    .bind(&args.side)
    .bind(&args.size)
    .bind(args.flipped)
    .bind(now)
    .execute(&state.db)
    .await?;
    let row: MirrorRow = sqlx::query_as("SELECT * FROM copy_mirror_queue WHERE id = ?")
        .bind(&id)
        .fetch_one(&state.db)
        .await?;
    Ok(row)
}

#[derive(Debug, Deserialize)]
pub struct ListMirrorsArgs {
    pub status: Option<String>,
    pub limit: Option<i64>,
}

/// 列出 mirror，按时间倒序，可按 status 过滤。
#[tauri::command]
pub async fn list_mirrors(
    state: State<'_, AppState>,
    args: ListMirrorsArgs,
) -> AppResult<Vec<MirrorRow>> {
    let limit = args.limit.unwrap_or(100).clamp(1, 1000);
    let rows: Vec<MirrorRow> = match args.status.as_deref() {
        Some(s) => {
            sqlx::query_as(
                "SELECT * FROM copy_mirror_queue WHERE status = ? ORDER BY created_at DESC LIMIT ?",
            )
            .bind(s)
            .bind(limit)
            .fetch_all(&state.db)
            .await?
        }
        None => {
            sqlx::query_as("SELECT * FROM copy_mirror_queue ORDER BY created_at DESC LIMIT ?")
                .bind(limit)
                .fetch_all(&state.db)
                .await?
        }
    };
    Ok(rows)
}

#[derive(Debug, Deserialize)]
pub struct RunPassArgs {
    pub key_alias: String,
    pub wallet_id: String,
}

/// 运行一轮 executor：挑选 pending → 提交，拒绝过期项。
#[tauri::command]
pub async fn run_mirror_executor_pass(
    state: State<'_, AppState>,
    args: RunPassArgs,
) -> AppResult<ExecutorPassResult> {
    run_pass_impl(&state, args.key_alias, args.wallet_id).await
}

// =================================================================
// ============== v0.44c —— paper mode IPC + paper_fills query ======
// =================================================================

/// v0.44c —— `set_mirror_paper_mode` 的参数。L1 在
/// Settings 挂载时以及每次切换时,把用户的 paper 模式偏好
/// 推送给 Rust。scheduler 在每次 tick 时读取当前值。
#[derive(Debug, Clone, Deserialize)]
pub struct SetMirrorPaperModeArgs {
    pub enabled: bool,
}

/// v0.44c —— paper 模式的运行时覆盖。本次调用之后,
/// 下一轮 mirror executor 把被选中的订单写入 `paper_fills`
/// （paper 模式开启）或 `bets`（paper 模式关闭）。环境变量
/// `POLYROCKET_MIRROR_PAPER_MODE` 仅在进程启动时生效;
/// 本次 IPC 之后,以用户的选择为准。
#[tauri::command]
pub async fn set_mirror_paper_mode(
    state: State<'_, AppState>,
    args: SetMirrorPaperModeArgs,
) -> AppResult<bool> {
    let mut guard = state
        .mirror_paper_mode
        .lock()
        .map_err(|e| AppError::Internal(format!("mirror_paper_mode lock: {e}")))?;
    *guard = args.enabled;
    Ok(args.enabled)
}

/// v0.44c —— 读取当前的 paper 模式覆盖值。
/// L1 在 Settings 挂载时调用此接口,以使开关
/// 反映 Rust 侧当前持有的值（防止环境变量在启动时设置）。
#[tauri::command]
pub async fn get_mirror_paper_mode(
    state: State<'_, AppState>,
) -> AppResult<bool> {
    let guard = state
        .mirror_paper_mode
        .lock()
        .map_err(|e| AppError::Internal(format!("mirror_paper_mode lock: {e}")))?;
    Ok(*guard)
}

/// v0.44c —— 单条 `paper_fills` 行的 wire 格式镜像。
/// 由下面的 `list_paper_fills` 使用。v0.45 —— 新增 4 个结算字段
/// （settled_at、resolved_outcome、won、pnl_usdc）。
/// 均为 Option —— v0.45 之前的 fill 没有这些字段。
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PaperFillDto {
    pub id: String,
    pub mirror_id: String,
    pub market_id: String,
    pub side: String,
    pub size: String,
    pub price: f64,
    pub placed_at: i64,
    pub notes: Option<String>,
    pub settled_at: Option<i64>,
    pub resolved_outcome: Option<String>,
    /// `1` = 获胜（方向匹配 outcome），`0` = 失败，
    /// `None` = 尚未结算。wire 格式的 bool
    /// 对 L1 更友好，但 Option<i64>
    /// 与 DB schema 完全一致。
    pub won: Option<i64>,
    pub pnl_usdc: Option<String>,
}

/// v0.44c —— `list_paper_fills` 的参数。结构与
/// `list_bets` 保持一致。
#[derive(Debug, Deserialize)]
pub struct ListPaperFillsArgs {
    pub limit: Option<i64>,
}

/// v0.44c —— 查询 paper_fills 表。L1 用它在 Copy、
/// PnL 中渲染 [PAPER] 徽章,以及展示仅 paper 的 PnL 汇总。
/// 默认 limit 100,按时间倒序。
#[tauri::command]
pub async fn list_paper_fills(
    state: State<'_, AppState>,
    args: ListPaperFillsArgs,
) -> AppResult<Vec<PaperFillDto>> {
    let limit = args.limit.unwrap_or(100);
    let rows = sqlx::query_as::<_, PaperFillDto>(
        "SELECT id, mirror_id, market_id, side, size, price, placed_at, notes,
                settled_at, resolved_outcome, won, pnl_usdc
         FROM paper_fills ORDER BY placed_at DESC LIMIT ?",
    )
    .bind(limit)
    .fetch_all(&state.db)
    .await?;
    Ok(rows)
}

pub async fn run_pass_impl(
    state: &AppState,
    key_alias: String,
    wallet_id: String,
) -> AppResult<ExecutorPassResult> {
    let cfg = ExecutorConfig::from_env();
    let now = chrono::Utc::now().timestamp_millis();

    // 1. 加载所有 pending mirror
    let rows: Vec<MirrorRow> = sqlx::query_as(
        "SELECT * FROM copy_mirror_queue WHERE status IN ('pending','submitted')",
    )
    .fetch_all(&state.db)
    .await?;
    let orders: Vec<MirrorOrder> = rows.into_iter().map(Into::into).collect();

    // 2. 加载市场收盘时间
    let market_ids: Vec<String> = orders.iter().map(|o| o.market_id.clone()).collect();
    let market_closes = if market_ids.is_empty() {
        HashMap::new()
    } else {
        let placeholders = market_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let query = format!(
            "SELECT id, end_date FROM markets WHERE id IN ({})",
            placeholders
        );
        let mut q = sqlx::query(&query);
        for id in &market_ids {
            q = q.bind(id);
        }
        let rows = q.fetch_all(&state.db).await?;
        let mut map = HashMap::new();
        for row in rows {
            let id: String = row.try_get("id")?;
            let end: i64 = row.try_get("end_date")?;
            map.insert(id, end);
        }
        map
    };

    // 3. 运行纯决策逻辑
    let result = execute_pass(&orders, &market_closes, now, &cfg)?;

    // 4. 应用拒绝
    for (id, reason) in &result.rejected {
        sqlx::query(
            "UPDATE copy_mirror_queue
             SET status = 'rejected', reject_reason = ?
             WHERE id = ? AND status = 'pending'",
        )
        .bind(reason.as_str())
        .bind(id)
        .execute(&state.db)
        .await?;
        // 审计 log
        sqlx::query(
            "INSERT INTO audit_log (actor, action, target, payload, result)
             VALUES ('system', 'mirror.reject', ?, ?, 'ok')",
        )
        .bind(id)
        .bind(serde_json::json!({"reason": reason.as_str()}))
        .execute(&state.db)
        .await?;
    }

    // 5. 应用 picks —— 提交为 Mode B 投注,或当 paper_mode
    // 开启时提交为 paper fill。决策逻辑（选什么、拒什么）
    // 不变;只有写入路径不同。
    for id in &result.picked {
        let order = orders.iter().find(|o| &o.id == id).cloned();
        let Some(o) = order else { continue };
        // 使用 domain::bet 中的 sign_order 获取确定性的 tx_hash
        let side = match crate::domain::bet::BetSide::parse(&o.side) {
            Ok(s) => s,
            Err(_) => {
                // 若非法则跳过
                sqlx::query(
                    "UPDATE copy_mirror_queue
                     SET status = 'rejected', reject_reason = 'invalid'
                     WHERE id = ?",
                )
                .bind(&o.id)
                .execute(&state.db)
                .await?;
                continue;
            }
        };

        if cfg.paper_mode {
            // v0.44 —— paper 模式。完全跳过签名;
            // 只写一行 paper_fills,并把该 mirror 标记为
            // paper-submitted。fill 是真实的（size、
            // price、market_id、side）,但用户并未真实下单。
            // mirror 同时被标记为 `paper_submitted`（而非
            // `submitted`）,方便 L1 过滤。
            let paper_id = uuid::Uuid::new_v4().to_string();
            sqlx::query(
                "INSERT INTO paper_fills
                    (id, mirror_id, market_id, side, size, price, placed_at, notes)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(&paper_id)
            .bind(&o.id)
            .bind(&o.market_id)
            .bind(&o.side)
            .bind(&o.size)
            .bind(0.5)
            .bind(now)
            .bind("paper mode — no CLOB submission")
            .execute(&state.db)
            .await?;

            sqlx::query(
                "UPDATE copy_mirror_queue
                 SET status = 'paper_submitted', submitted_at = ?, bet_id = ?
                 WHERE id = ?",
            )
            .bind(now)
            .bind(&paper_id)
            .bind(&o.id)
            .execute(&state.db)
            .await?;

            sqlx::query(
                "INSERT INTO audit_log (actor, action, target, payload, result)
                 VALUES ('system', 'mirror.paper_submit', ?, ?, 'ok')",
            )
            .bind(&o.id)
            .bind(serde_json::json!({
                "paper_id": paper_id,
                "market_id": o.market_id,
                "size": o.size,
                "side": o.side,
                "note": "paper mode — fill not submitted to CLOB",
            }))
            .execute(&state.db)
            .await?;
            continue;
        }

        let place = crate::domain::bet::PlaceArgs {
            market_id: o.market_id.clone(),
            side,
            size_usdc: o.size.clone(),
            price: 0.5, // mirror uses mid for v0.6; v0.7 will resolve best_ask
            key_alias: Some(key_alias.clone()),
            order_type: crate::domain::bet::OrderType::Market, // v0.50a — mirrors always Market
            limit_price: None,
            stop_price: None,
            post_only: false,
        };
        let signed = sign_order(&place, now).map_err(|e| {
            AppError::Internal(format!("sign_order: {e}"))
        })?;

        // 插入 bet 行
        let bet_id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO bets
                (id, wallet_id, market_id, signal_id, mode, side, size, price, shares, placed_at, status, tx_hash)
             VALUES (?, ?, ?, NULL, 'B_signed', ?, ?, ?, ?, ?, 'open', ?)",
        )
        .bind(&bet_id)
        .bind(&wallet_id)
        .bind(&o.market_id)
        .bind(&o.side)
        .bind(&o.size)
        .bind(0.5)
        .bind(&signed.shares)
        .bind(signed.signed_at_ms)
        .bind(&signed.tx_hash)
        .execute(&state.db)
        .await?;

        // 将 mirror 标记为 submitted 并关联到 bet
        sqlx::query(
            "UPDATE copy_mirror_queue
             SET status = 'submitted', submitted_at = ?, bet_id = ?
             WHERE id = ?",
        )
        .bind(signed.signed_at_ms)
        .bind(&bet_id)
        .bind(&o.id)
        .execute(&state.db)
        .await?;

        // 审计
        sqlx::query(
            "INSERT INTO audit_log (actor, action, target, payload, result)
             VALUES ('system', 'mirror.submit', ?, ?, 'ok')",
        )
        .bind(&o.id)
        .bind(serde_json::json!({
            "bet_id": bet_id,
            "tx_hash": signed.tx_hash,
            "market_id": o.market_id,
            "size": o.size,
            "side": o.side,
        }))
        .execute(&state.db)
        .await?;
    }

    Ok(result)
}

#[derive(Debug, Serialize)]
pub struct MirrorQueueStats {
    pub n_pending: i64,
    pub n_submitted: i64,
    pub n_filled: i64,
    pub n_rejected: i64,
    pub n_expired: i64,
    pub total_exposure_usdc: f64,
    pub headroom_usdc: f64,
}

/// 队列与 executor 余量的汇总统计。
#[tauri::command]
pub async fn mirror_queue_stats(state: State<'_, AppState>) -> AppResult<MirrorQueueStats> {
    let row = sqlx::query(
        "SELECT status, COUNT(*) as n, COALESCE(SUM(CAST(size AS REAL)), 0.0) as exposure
         FROM copy_mirror_queue
         WHERE status IN ('pending', 'submitted', 'filled', 'rejected', 'expired')
         GROUP BY status",
    )
    .fetch_all(&state.db)
    .await?;
    let mut s = MirrorQueueStats {
        n_pending: 0,
        n_submitted: 0,
        n_filled: 0,
        n_rejected: 0,
        n_expired: 0,
        total_exposure_usdc: 0.0,
        headroom_usdc: 0.0,
    };
    for row in row {
        let status: String = row.try_get("status")?;
        let n: i64 = row.try_get("n")?;
        let exp: f64 = row.try_get("exposure")?;
        match status.as_str() {
            "pending" => s.n_pending = n,
            "submitted" => s.n_submitted = n,
            "filled" => s.n_filled = n,
            "rejected" => s.n_rejected = n,
            "expired" => s.n_expired = n,
            _ => {}
        }
        if status == "pending" || status == "submitted" {
            s.total_exposure_usdc += exp;
        }
    }
    let cfg = ExecutorConfig::from_env();
    s.headroom_usdc = (cfg.max_total_exposure_usdc - s.total_exposure_usdc).max(0.0);
    Ok(s)
}
