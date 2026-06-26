//! L4 —— 资金分配数据库层（v0.78）。
//!
//! 两张表:
//!   - `bankroll_config`（按钱包）—— 6 字段的 `BankrollConfig`
//!   - `allocation_batches` —— 每次 `apply_allocation` 调用对应一行
//!
//! 另含对 `bets` 表的扩展（在 v0.78e 的独立迁移中处理;
//! v0.78d 仅停留在资金分配相关表）。
//!
//! 规格:docs/bankroll-allocation-design.md §3。

use crate::domain::bankroll::BankrollConfig;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

/// v0.78 —— 创建 bankroll 表。**幂等**（IF NOT EXISTS）。
pub async fn ensure_tables(pool: &SqlitePool) -> sqlx::Result<()> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS bankroll_config (
            wallet_id TEXT PRIMARY KEY,
            kelly_multiplier REAL NOT NULL,
            max_per_signal_pct REAL NOT NULL,
            reserve_pct REAL NOT NULL,
            min_edge_pct REAL NOT NULL,
            max_total_exposure_pct REAL NOT NULL,
            min_confidence REAL NOT NULL,
            updated_at INTEGER NOT NULL DEFAULT (unixepoch() * 1000)
        )",
    )
    .execute(pool)
    .await?;
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS allocation_batches (
            id TEXT PRIMARY KEY,
            wallet_id TEXT NOT NULL,
            bankroll_usdc TEXT NOT NULL,
            config_json TEXT NOT NULL,
            total_allocated_usdc TEXT NOT NULL,
            applied_at INTEGER NOT NULL DEFAULT (unixepoch() * 1000)
        )",
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// 每批分配的审计记录。以 JSON 存储于 `config_json` 列,
/// 外加一行扁平化记录以便查询。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllocationBatch {
    pub id: String,
    pub wallet_id: String,
    pub bankroll_usdc: String,
    pub config_json: String,
    pub total_allocated_usdc: String,
    pub applied_at: i64,
}

/// 获取某钱包的资金配置。若未设置则返回 `None`
///（调用方应回退到 `BankrollConfig::default()`）。
pub async fn get_config(
    pool: &SqlitePool,
    wallet_id: &str,
) -> sqlx::Result<Option<BankrollConfig>> {
    let row: Option<(f64, f64, f64, f64, f64, f64)> = sqlx::query_as(
        "SELECT kelly_multiplier, max_per_signal_pct, reserve_pct,
                min_edge_pct, max_total_exposure_pct, min_confidence
           FROM bankroll_config
          WHERE wallet_id = ?",
    )
    .bind(wallet_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|(km, mps, rp, me, mte, mc)| BankrollConfig {
        kelly_multiplier: km,
        max_per_signal_pct: mps,
        reserve_pct: rp,
        min_edge_pct: me,
        max_total_exposure_pct: mte,
        min_confidence: mc,
    }))
}

/// 写入或更新某钱包的资金配置。总会更新 `updated_at`。
pub async fn set_config(
    pool: &SqlitePool,
    wallet_id: &str,
    config: &BankrollConfig,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO bankroll_config
            (wallet_id, kelly_multiplier, max_per_signal_pct, reserve_pct,
             min_edge_pct, max_total_exposure_pct, min_confidence, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, unixepoch() * 1000)
         ON CONFLICT(wallet_id) DO UPDATE SET
            kelly_multiplier = excluded.kelly_multiplier,
            max_per_signal_pct = excluded.max_per_signal_pct,
            reserve_pct = excluded.reserve_pct,
            min_edge_pct = excluded.min_edge_pct,
            max_total_exposure_pct = excluded.max_total_exposure_pct,
            min_confidence = excluded.min_confidence,
            updated_at = unixepoch() * 1000",
    )
    .bind(wallet_id)
    .bind(config.kelly_multiplier)
    .bind(config.max_per_signal_pct)
    .bind(config.reserve_pct)
    .bind(config.min_edge_pct)
    .bind(config.max_total_exposure_pct)
    .bind(config.min_confidence)
    .execute(pool)
    .await?;
    Ok(())
}

/// 记录一笔已应用的分配批次。`id` 是由调用方
/// 生成的 UUID。返回插入的 `AllocationBatch`。
pub async fn insert_batch(
    pool: &SqlitePool,
    batch: &AllocationBatch,
) -> sqlx::Result<AllocationBatch> {
    let now = chrono::Utc::now().timestamp_millis();
    sqlx::query(
        "INSERT INTO allocation_batches
            (id, wallet_id, bankroll_usdc, config_json, total_allocated_usdc, applied_at)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&batch.id)
    .bind(&batch.wallet_id)
    .bind(&batch.bankroll_usdc)
    .bind(&batch.config_json)
    .bind(&batch.total_allocated_usdc)
    .bind(batch.applied_at)
    .execute(pool)
    .await?;
    Ok(AllocationBatch {
        applied_at: if batch.applied_at == 0 { now } else { batch.applied_at },
        ..batch.clone()
    })
}

/// 列出某钱包的所有批次,按时间倒序。用于已应用分配的
/// "历史" 视图。
pub async fn list_batches(
    pool: &SqlitePool,
    wallet_id: &str,
    limit: i64,
) -> sqlx::Result<Vec<AllocationBatch>> {
    let rows: Vec<(String, String, String, String, String, i64)> = sqlx::query_as(
        "SELECT id, wallet_id, bankroll_usdc, config_json, total_allocated_usdc, applied_at
           FROM allocation_batches
          WHERE wallet_id = ?
          ORDER BY applied_at DESC
          LIMIT ?",
    )
    .bind(wallet_id)
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(id, wallet_id, bankroll_usdc, config_json, total_allocated_usdc, applied_at)| {
            AllocationBatch {
                id,
                wallet_id,
                bankroll_usdc,
                config_json,
                total_allocated_usdc,
                applied_at,
            }
        })
        .collect())
}
