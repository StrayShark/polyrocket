//! L2 —— 资金分配 IPC 命令（M11，v0.78）。
//!
//! v0.78b/d/e 提供资金分配器的 IPC 接口。
//! 纯函数算法位于 `crate::domain::bankroll`。
//! 本文件是**精简的 IPC 适配层**——不含业务逻辑。
//!
//! v0.78b:  compute_allocation_preview（纯函数）+ validate_config
//! v0.78d:  get_bankroll_config、set_bankroll_config（按 wallet）
//! v0.78e:  apply_allocation（写入 allocation_batches + bets）
//!
//! 设计文档:docs/bankroll-allocation-design.md §2.2。

use crate::AppResult;
use crate::domain::bankroll::{
    compute_allocation as compute, AllocationInput, BankrollConfig,
};
use crate::domain::signal::Signal;
use crate::infra::state::AppState;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::HashMap;
use tauri::State;

/// `compute_allocation_preview` 的 IPC 请求结构。
///
/// 与 `AllocationInput` 形状一致,但使用拥有所有权的字段（IPC 层
/// 不能持有引用——Tauri 会反序列化为 owned 类型）。
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ComputeAllocationArgs {
    /// 可用 USDC 总数（字符串以兼容 DB TEXT）。
    pub bankroll_usdc: String,
    /// 每个 wallet 的配置（若为 None 则使用默认值）。
    pub config: Option<BankrollConfigDto>,
    /// 待评估的信号列表。
    pub signals: Vec<Signal>,
    /// 可选 —— market_id → 该市场最大可分配 USDC（流动性上限）。
    pub market_liquidity: Option<HashMap<String, String>>,
}

/// `BankrollConfig` 的 IPC 友好 DTO。形状相同但类型分开,以保持
/// L1↔L2 边界的清晰（domain::BankrollConfig 不含 `specta::Type` derive,
/// 以避免依赖 IPC 层类型）。
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct BankrollConfigDto {
    pub kelly_multiplier: f64,
    pub max_per_signal_pct: f64,
    pub reserve_pct: f64,
    pub min_edge_pct: f64,
    pub max_total_exposure_pct: f64,
    pub min_confidence: f64,
}

impl From<&BankrollConfigDto> for BankrollConfig {
    fn from(dto: &BankrollConfigDto) -> Self {
        Self {
            kelly_multiplier: dto.kelly_multiplier,
            max_per_signal_pct: dto.max_per_signal_pct,
            reserve_pct: dto.reserve_pct,
            min_edge_pct: dto.min_edge_pct,
            max_total_exposure_pct: dto.max_total_exposure_pct,
            min_confidence: dto.min_confidence,
        }
    }
}

/// 校验 `BankrollConfig`。若任何字段越界则返回 Err。
/// 由 `set_bankroll_config` IPC 在持久化前拒绝非法输入使用。
pub fn validate_config(config: &BankrollConfigDto) -> AppResult<()> {
    use crate::infra::error::AppError;
    let r = |lo, hi, name, v: f64| -> AppResult<()> {
        if !v.is_finite() || v < lo || v > hi {
            Err(AppError::Invalid(format!(
                "{} = {} out of range [{}, {}]",
                name, v, lo, hi
            )))
        } else {
            Ok(())
        }
    };
    r(0.0, 2.0, "kelly_multiplier", config.kelly_multiplier)?;
    r(0.0, 1.0, "max_per_signal_pct", config.max_per_signal_pct)?;
    r(0.0, 0.99, "reserve_pct", config.reserve_pct)?;
    r(0.0, 1.0, "min_edge_pct", config.min_edge_pct)?;
    r(0.0, 1.0, "max_total_exposure_pct", config.max_total_exposure_pct)?;
    r(0.0, 1.0, "min_confidence", config.min_confidence)?;
    // 跨字段校验:reserve + max_total <= 1.0
    if config.reserve_pct + config.max_total_exposure_pct > 1.0 {
        return Err(AppError::Invalid(format!(
            "reserve_pct ({}) + max_total_exposure_pct ({}) > 1.0",
            config.reserve_pct, config.max_total_exposure_pct
        )));
    }
    Ok(())
}

/// IPC:计算分配预览(不写 DB)。
#[tauri::command]
#[specta::specta]
pub fn compute_allocation_preview(
    args: ComputeAllocationArgs,
) -> AppResult<crate::domain::bankroll::AllocationResult> {
    let config: BankrollConfig = match &args.config {
        Some(dto) => {
            validate_config(dto)?;
            dto.into()
        }
        None => BankrollConfig::default(),
    };
    let input = AllocationInput {
        bankroll_usdc: &args.bankroll_usdc,
        config: &config,
        signals: &args.signals,
        market_liquidity: args.market_liquidity.as_ref(),
    };
    Ok(compute(&input))
}

/// v0.78d —— 获取按 wallet 的配置（DB 持久化）。未设置时返回默认值。
#[tauri::command]
#[specta::specta]
pub async fn get_bankroll_config(
    state: State<'_, AppState>,
    wallet_id: String,
) -> AppResult<BankrollConfigDto> {
    let config = crate::infra::db::bankroll::get_config(&state.db, &wallet_id)
        .await?
        .unwrap_or_default();
    Ok(BankrollConfigDto {
        kelly_multiplier: config.kelly_multiplier,
        max_per_signal_pct: config.max_per_signal_pct,
        reserve_pct: config.reserve_pct,
        min_edge_pct: config.min_edge_pct,
        max_total_exposure_pct: config.max_total_exposure_pct,
        min_confidence: config.min_confidence,
    })
}

/// v0.78d —— 设置按 wallet 的配置。先校验。
#[tauri::command]
#[specta::specta]
pub async fn set_bankroll_config(
    state: State<'_, AppState>,
    wallet_id: String,
    config: BankrollConfigDto,
) -> AppResult<()> {
    validate_config(&config)?;
    let c: BankrollConfig = (&config).into();
    crate::infra::db::bankroll::set_config(&state.db, &wallet_id, &c).await?;
    Ok(())
}

/// v0.78e —— 应用一次分配结果。写入 `allocation_batches`。
/// v0.79a —— 同时写入 N 条 `bets` 行（每个 AllocationItem 一条）,
/// 每条 `mode = 'C_allocated'` 且 `allocation_id = <batch_id>`。
/// 返回批次 id（UUID）。
#[tauri::command]
#[specta::specta]
pub async fn apply_allocation(
    state: State<'_, AppState>,
    wallet_id: String,
    result: crate::domain::bankroll::AllocationResult,
    bankroll_usdc: String,
    config: BankrollConfigDto,
) -> AppResult<String> {
    let id = uuid::Uuid::new_v4().to_string();
    let config_json = serde_json::to_string(&config).map_err(|e| {
        crate::infra::error::AppError::Internal(format!("serialize config: {e}"))
    })?;
    let batch = crate::infra::db::bankroll::AllocationBatch {
        id: id.clone(),
        wallet_id: wallet_id.clone(),
        bankroll_usdc,
        config_json,
        total_allocated_usdc: result.total_allocated_usdc.clone(),
        applied_at: chrono::Utc::now().timestamp_millis(),
    };
    crate::infra::db::bankroll::insert_batch(&state.db, &batch).await?;

    // v0.79a —— 为每个 AllocationItem 写一条 `bets` 行,
    // 通过 `allocation_id` 关联回批次。这就是「点击 apply,
    // 投注就落入 DB」的路径。真正的订单执行(signed_tx → CLOB)
    // 仍是单独的步骤(v0.51+ executor);资金分配路径预先以
    // `status = 'open'` 创建投注行,这样仪表盘能立刻
    // 反映此次分配。
    let now_ms = chrono::Utc::now().timestamp_millis();
    for item in &result.per_market {
        let bet_id = uuid::Uuid::new_v4().to_string();
        // `size` 单位为 USDC;此处以 0.5 作为占位价格
        // (真实价格由执行器在成交时从市场快照中读取 —— v0.51b+)
        let side_str = match item.side {
            crate::domain::bankroll::BetSide::Yes => "YES",
            crate::domain::bankroll::BetSide::No => "NO",
        };
        let size_f = parse_usdc(&item.size_usdc).unwrap_or(0.0);
        sqlx::query(
            "INSERT INTO bets (
                id, wallet_id, market_id, signal_id, decision_id,
                was_llm_assisted, mode, side, size, price, shares,
                placed_at, settled_at, pnl, status, tx_hash, notes,
                order_type, limit_price, stop_price, post_only,
                filled_at, fill_price, fill_size, partial,
                allocation_id
             ) VALUES (
                ?, ?, ?, NULL, NULL, 1, 'C_allocated', ?, ?, 0.5, ?,
                ?, NULL, NULL, 'open', NULL, NULL,
                'market', NULL, NULL, 0,
                ?, 0.5, ?, 0, ?
             )",
        )
        .bind(&bet_id)
        .bind(&wallet_id)
        .bind(&item.market_id)
        .bind(side_str)
        .bind(&item.size_usdc)
        .bind(format!("{:.6}", size_f / 0.5))
        .bind(now_ms)
        .bind(now_ms)
        .bind(&item.size_usdc)
        .bind(&id)
        .execute(&state.db)
        .await?;
    }

    Ok(id)
}

fn parse_usdc(s: &str) -> Option<f64> {
    s.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::bankroll::BankrollConfig;

    fn default_dto() -> BankrollConfigDto {
        let c: BankrollConfig = BankrollConfig::default();
        BankrollConfigDto {
            kelly_multiplier: c.kelly_multiplier,
            max_per_signal_pct: c.max_per_signal_pct,
            reserve_pct: c.reserve_pct,
            min_edge_pct: c.min_edge_pct,
            max_total_exposure_pct: c.max_total_exposure_pct,
            min_confidence: c.min_confidence,
        }
    }

    fn sig(market: &str, edge: f64, conf: f64) -> Signal {
        Signal {
            market_id: market.to_string(),
            computed_at: 1700000000000,
            model_version: "m1".to_string(),
            predicted_prob: 0.5 + edge,
            market_prob: 0.5,
            edge,
            confidence: conf,
            horizon_hours: 24,
            rationale: None,
        }
    }

    #[test]
    fn validate_config_accepts_default() {
        let dto = default_dto();
        assert!(validate_config(&dto).is_ok());
    }

    #[test]
    fn validate_config_rejects_negative_kelly() {
        let mut dto = default_dto();
        dto.kelly_multiplier = -0.1;
        assert!(validate_config(&dto).is_err());
    }

    #[test]
    fn validate_config_rejects_kelly_above_2() {
        let mut dto = default_dto();
        dto.kelly_multiplier = 5.0;
        assert!(validate_config(&dto).is_err());
    }

    #[test]
    fn validate_config_rejects_max_per_signal_above_1() {
        let mut dto = default_dto();
        dto.max_per_signal_pct = 1.5;
        assert!(validate_config(&dto).is_err());
    }

    #[test]
    fn validate_config_rejects_nan() {
        let mut dto = default_dto();
        dto.kelly_multiplier = f64::NAN;
        assert!(validate_config(&dto).is_err());
    }

    #[test]
    fn validate_config_rejects_reserve_plus_total_above_1() {
        let mut dto = default_dto();
        dto.reserve_pct = 0.5;
        dto.max_total_exposure_pct = 0.6;
        assert!(validate_config(&dto).is_err());
    }

    #[test]
    fn compute_allocation_preview_with_default_config() {
        let args = ComputeAllocationArgs {
            bankroll_usdc: "1000".to_string(),
            config: None,
            signals: vec![sig("m1", 0.10, 0.8)],
            market_liquidity: None,
        };
        let r = compute_allocation_preview(args).unwrap();
        assert_eq!(r.per_market.len(), 1);
        assert_eq!(r.per_market[0].size_usdc, "50.00");
        assert_eq!(r.total_allocated_usdc, "50.00");
    }

    #[test]
    fn compute_allocation_preview_rejects_invalid_config() {
        let mut dto = default_dto();
        dto.kelly_multiplier = -0.5;
        let args = ComputeAllocationArgs {
            bankroll_usdc: "1000".to_string(),
            config: Some(dto),
            signals: vec![],
            market_liquidity: None,
        };
        assert!(compute_allocation_preview(args).is_err());
    }

    #[test]
    fn compute_allocation_preview_empty_signals() {
        let args = ComputeAllocationArgs {
            bankroll_usdc: "1000".to_string(),
            config: None,
            signals: vec![],
            market_liquidity: None,
        };
        let r = compute_allocation_preview(args).unwrap();
        assert_eq!(r.total_allocated_usdc, "0.00");
        assert_eq!(r.reserved_usdc, "200.00");
    }

    #[test]
    fn from_dto_preserves_all_fields() {
        let dto = BankrollConfigDto {
            kelly_multiplier: 0.5,
            max_per_signal_pct: 0.15,
            reserve_pct: 0.10,
            min_edge_pct: 0.03,
            max_total_exposure_pct: 0.85,
            min_confidence: 0.7,
        };
        let c: BankrollConfig = (&dto).into();
        assert_eq!(c.kelly_multiplier, 0.5);
        assert_eq!(c.max_per_signal_pct, 0.15);
        assert_eq!(c.reserve_pct, 0.10);
        assert_eq!(c.min_edge_pct, 0.03);
        assert_eq!(c.max_total_exposure_pct, 0.85);
        assert_eq!(c.min_confidence, 0.7);
    }
}
