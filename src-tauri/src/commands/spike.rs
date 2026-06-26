//! L2 —— 异动检测（P0-3）。
//!
//! IPC：
//!   - `list_spike_alerts` —— 从 `spike_alerts` 读取已持久化的告警
//!   - `run_spike_scan` —— 扫描 `price_snapshots` 寻找剧烈价格变动，
//!     持久化新告警，返回新增数量。

use crate::AppResult;
use crate::domain::spike::{
    DEFAULT_SPIKE_THRESHOLD_PCT, PriceSnapshotRow, SpikeAlert, detect_spikes,
};
use crate::infra::state::AppState;
use serde::{Deserialize, Serialize};
use specta::Type;
use sqlx::FromRow;
use tauri::State;

/// 价格异动告警的传输 DTO，镜像 `spike_alerts` 表。
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, Type)]
pub struct SpikeAlertDto {
    pub id: i64,
    pub market_id: String,
    pub old_price: f64,
    pub new_price: f64,
    pub change_pct: f64,
    pub detected_at: i64,
    pub market_question: Option<String>,
}

/// 检测流程使用的原始价格快照行。
#[derive(Debug, Clone, FromRow)]
struct SnapshotRow {
    id: i64,
    market_id: String,
    captured_at: i64,
    mid_price: f64,
}

/// `run_spike_scan` 阶段用于去重的已存在告警行。
#[derive(Debug, Clone, FromRow)]
struct ExistingAlertKey {
    market_id: String,
    detected_at: i64,
}

/// IPC:list_spike_alerts —— 返回最近的异动告警，按时间倒序排列。
/// `limit` 默认 50。
#[tauri::command]
pub async fn list_spike_alerts(
    state: State<'_, AppState>,
    limit: Option<i64>,
) -> AppResult<Vec<SpikeAlertDto>> {
    let limit = limit.unwrap_or(50);

    let rows = sqlx::query_as::<_, SpikeAlertDto>(
        "SELECT sa.id, sa.market_id, sa.old_price, sa.new_price,
                sa.change_pct, sa.detected_at,
                m.question AS market_question
         FROM spike_alerts sa
         LEFT JOIN markets m ON m.id = sa.market_id
         ORDER BY sa.detected_at DESC
         LIMIT ?",
    )
    .bind(limit)
    .fetch_all(&state.db)
    .await?;

    Ok(rows)
}

/// IPC:run_spike_scan —— 扫描 price_snapshots，寻找
/// 相邻采样间超过默认阈值（5%）的价格变动，把新告警持久化
/// 到 spike_alerts 表，并返回新增的行数。
///
/// 去重：在插入前加载已有的 `(market_id, detected_at)` 集合，
/// 任何已存在的 spike 一律跳过。这样扫描是幂等的 —— 对相同
/// snapshots 跑两次不会产生重复告警。
#[tauri::command]
pub async fn run_spike_scan(state: State<'_, AppState>) -> AppResult<i64> {
    // 1) 拉取所有 price_snapshots，按 market + 时间排序，
    //    这样领域函数可以逐市场比较相邻行。
    let snapshots: Vec<SnapshotRow> = sqlx::query_as::<_, SnapshotRow>(
        "SELECT id, market_id, captured_at, mid_price
         FROM price_snapshots
         ORDER BY market_id ASC, captured_at ASC",
    )
    .fetch_all(&state.db)
    .await?;

    if snapshots.len() < 2 {
        return Ok(0);
    }

    // 2) 按 market_id 对 snapshots 分组（它们已经排好序），
    //    然后对每组调用 detect_spikes。
    let mut all_alerts: Vec<SpikeAlert> = Vec::new();
    let mut start = 0usize;
    while start < snapshots.len() {
        let market_id = &snapshots[start].market_id;
        let mut end = start + 1;
        while end < snapshots.len() && snapshots[end].market_id == *market_id {
            end += 1;
        }
        let group: Vec<PriceSnapshotRow> = snapshots[start..end]
            .iter()
            .map(|s| PriceSnapshotRow {
                id: s.id,
                market_id: s.market_id.clone(),
                captured_at: s.captured_at,
                mid_price: s.mid_price,
            })
            .collect();
        let mut alerts = detect_spikes(&group, DEFAULT_SPIKE_THRESHOLD_PCT);
        all_alerts.append(&mut alerts);
        start = end;
    }

    if all_alerts.is_empty() {
        return Ok(0);
    }

    // 3) 加载已有的 (market_id, detected_at) 对，避免在
    //    重复扫描时插入重复告警。
    let existing: Vec<ExistingAlertKey> =
        sqlx::query_as::<_, ExistingAlertKey>("SELECT market_id, detected_at FROM spike_alerts")
            .fetch_all(&state.db)
            .await?;

    let mut existing_set: std::collections::HashSet<(String, i64)> =
        std::collections::HashSet::with_capacity(existing.len());
    for e in existing {
        existing_set.insert((e.market_id, e.detected_at));
    }

    // 4) 仅插入新 alert。
    let mut new_count = 0i64;
    for alert in &all_alerts {
        let key = (alert.market_id.clone(), alert.detected_at);
        if existing_set.contains(&key) {
            continue;
        }
        sqlx::query(
            "INSERT INTO spike_alerts
                (market_id, old_price, new_price, change_pct, detected_at, notified)
             VALUES (?, ?, ?, ?, ?, 0)",
        )
        .bind(&alert.market_id)
        .bind(alert.old_price)
        .bind(alert.new_price)
        .bind(alert.change_pct)
        .bind(alert.detected_at)
        .execute(&state.db)
        .await?;
        existing_set.insert(key);
        new_count += 1;
    }

    // 5) 审计 log —— 记录一次 spike 扫描的执行。
    if new_count > 0 {
        sqlx::query(
            "INSERT INTO audit_log (actor, action, target, payload, result)
             VALUES ('system', 'spike.scan', 'spike_alerts', ?, 'ok')",
        )
        .bind(serde_json::json!({ "new_alerts": new_count }))
        .execute(&state.db)
        .await?;
    }

    Ok(new_count)
}
