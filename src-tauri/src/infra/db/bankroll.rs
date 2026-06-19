//! L4 — Bankroll allocation DB layer (v0.78).
//!
//! Two tables:
//!   - `bankroll_config` (per-wallet) — the 6-field `BankrollConfig`
//!   - `allocation_batches` — one row per `apply_allocation` call
//!
//! Plus an extension to `bets` (handled in a separate migration in
//! v0.78e; v0.78d stops at the bankroll-specific tables).
//!
//! Spec: docs/bankroll-allocation-design.md §3.

use crate::domain::bankroll::BankrollConfig;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

/// v0.78 — create the bankroll tables. **Idempotent** (IF NOT EXISTS).
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

/// Per-batch allocation audit record. Stored as JSON in the
/// `config_json` column + a flat row for query convenience.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllocationBatch {
    pub id: String,
    pub wallet_id: String,
    pub bankroll_usdc: String,
    pub config_json: String,
    pub total_allocated_usdc: String,
    pub applied_at: i64,
}

/// Get the bankroll config for a wallet. Returns `None` if not set
/// (caller should fall back to `BankrollConfig::default()`).
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

/// Upsert a per-wallet bankroll config. Always touches `updated_at`.
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

/// Record an applied allocation batch. The `id` is a UUID generated
/// by the caller. Returns the inserted `AllocationBatch`.
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

/// List all batches for a wallet, newest first. Used for the
/// "history" view of applied allocations.
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
