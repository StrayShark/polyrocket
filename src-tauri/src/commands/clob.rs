//! L2 —— CLOB（中央限价订单簿）IPC 命令（v0.51a）。
//!
//! 当前提供三个 IPC:
//! - `clob_feed_status` —— 返回真实 CLOB 行情源是否
//!   已配置并连接。环境中缺少 `POLYROCKET_CLOB_API_KEY`
//!   + `POLYROCKET_CLOB_API_SECRET` 与 `POLYROCKET_CLOB_API_PASSPHRASE`
//!   时,行情源状态为 "not_configured",L1 应回退
//!   至 `price_snapshots`。
//! - `record_clob_snapshot_now` —— 手动一次性插入。
//!   用于测试和「在 UI 粘贴 snapshot」的工作流。
//! - `latest_clob_snapshot` —— 返回某市场最近的 snapshot。
//!
//! 真实的 CLOB WebSocket 监听器位于 v0.51+（仍在等待）。
//! schema 与 IPC 已就绪,只要接上行情,其余部分即自动衔接。

use crate::AppResult;
use crate::infra::db::clob_snapshots::{
    self, ClobSnapshot,
};
use crate::infra::state::AppState;
use serde::Serialize;
use tauri::State;

#[derive(Debug, Clone, Serialize)]
pub struct ClobFeedStatus {
    /// 取值之一:
    ///   "not_configured" —— 环境变量缺失;L1 应改用
    ///     `price_snapshots` 表
    ///   "configured"     —— 凭据齐全,但尚未运行 feed 任务（v0.51+）
    ///   "connected"      —— feed 正在活跃推送（v0.51+
    ///     待 WebSocket 监听器接入后启用）
    pub state: &'static str,
    /// 当前任意市场的 snapshot 总数
    /// （按市场汇总）。尚无 snapshot 时为 0。
    pub total_snapshots: i64,
    /// 至少有一个 snapshot 的独立市场数量。
    pub markets_with_snapshots: i64,
}

/// v0.51a —— 返回当前 CLOB 行情源状态。
/// 环境中无凭据时,返回 `not_configured`。
/// L1 据此决定是渲染「live」指示,
/// 还是回退至 v0.47a 的 `price_snapshots`。
///
/// v0.119 —— 兼容以下任意命名约定:
///   - `POLYROCKET_CLOB_API_KEY / _SECRET / _PASSPHRASE`（旧版）
///   - `POLYMARKET_API_KEY` / `POLYMARKET_API_SECRET` /
///     `POLYMARKET_API_PASSPHRASE`（Polymarket 标准约定）
///
/// 两者同时设置时,以更明确的 `POLYMARKET_*` 为准。
#[tauri::command]
pub async fn clob_feed_status(state: State<'_, AppState>) -> AppResult<ClobFeedStatus> {
    // v0.119 —— 兼容两种命名约定。两者同时设置时,以 `POLYMARKET_*` 为准。
    let api_key = std::env::var("POLYMARKET_API_KEY")
        .ok()
        .filter(|v| !v.is_empty())
        .or_else(|| std::env::var("POLYROCKET_CLOB_API_KEY").ok().filter(|v| !v.is_empty()));
    let api_secret = std::env::var("POLYMARKET_API_SECRET")
        .ok()
        .filter(|v| !v.is_empty())
        .or_else(|| std::env::var("POLYROCKET_CLOB_API_SECRET").ok().filter(|v| !v.is_empty()));
    let passphrase = std::env::var("POLYMARKET_API_PASSPHRASE")
        .ok()
        .filter(|v| !v.is_empty())
        .or_else(|| std::env::var("POLYROCKET_CLOB_API_PASSPHRASE").ok().filter(|v| !v.is_empty()));

    let state_str = match (api_key, api_secret, passphrase) {
        (Some(k), Some(s), Some(p))
            if !k.is_empty() && !s.is_empty() && !p.is_empty() =>
        {
            // v0.51+ —— 待 WebSocket 监听器接入后,
            // 这里会变为 "connected" / "reconnecting"。
            // 当前即使有凭据,仍返回 "configured"。
            "configured"
        }
        _ => "not_configured",
    };
    // 汇总统计。
    let total: i64 = sqlx::query_scalar(
        "SELECT COUNT(DISTINCT (market_id, captured_at)) FROM clob_snapshots",
    )
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);
    let markets: i64 = sqlx::query_scalar(
        "SELECT COUNT(DISTINCT market_id) FROM clob_snapshots",
    )
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);
    Ok(ClobFeedStatus {
        state: state_str,
        total_snapshots: total,
        markets_with_snapshots: markets,
    })
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct RecordClobSnapshotArgs {
    pub market_id: String,
    pub captured_at: i64,
    pub bids: Vec<(f64, f64)>,
    pub asks: Vec<(f64, f64)>,
}

/// v0.51a —— 手动插入。记录一条 snapshot
/// （同一时间戳下的 bids + asks 批次）,
/// 返回写入的条数。供测试和 L1 的「粘贴 snapshot」
/// 工作流使用。
#[tauri::command]
pub async fn record_clob_snapshot_now(
    state: State<'_, AppState>,
    args: RecordClobSnapshotArgs,
) -> AppResult<usize> {
    clob_snapshots::record_clob_snapshot(
        &state.db,
        &args.market_id,
        args.captured_at,
        &args.bids,
        &args.asks,
    )
    .await
    .map_err(|e| crate::AppError::Internal(format!("record_clob_snapshot: {e}")))?;
    Ok(args.bids.len() + args.asks.len())
}

/// v0.51a —— 返回某市场最近的 snapshot。
/// 该市场无 snapshot 时返回 None。
/// L1 据此渲染订单簿梯形列表。
#[tauri::command]
pub async fn latest_clob_snapshot(
    state: State<'_, AppState>,
    market_id: String,
) -> AppResult<Option<ClobSnapshot>> {
    clob_snapshots::latest_clob_snapshot(&state.db, &market_id)
        .await
        .map_err(|e| crate::AppError::Internal(format!("latest_clob_snapshot: {e}")))
}
