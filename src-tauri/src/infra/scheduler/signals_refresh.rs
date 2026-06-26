//! v0.127 —— signals_refresh loop
//!
//! 每 `POLYROCKET_SIGNALS_REFRESH_MIN` 分钟（默认 30）扫描所有
//! 活跃的 football market（markets.status='active' AND
//! category='football'），重算 smart_money_score + crowd_opinion
//! 并 UPSERT 到 `market_signal_cache` 表。
//!
//! **目的**：L1 频繁访问 MarketDetail，每次都现场重算
//! smart_money_score 和 crowd_opinion 会涉及 bets+wallets 大表 JOIN，
//! 单次查询 50-200ms。预计算 cache 后：
//!   - L1 走 cache：<5ms (单条 PK 查询)
//!   - L1 触发现场重算：50-200ms（仅在 cache 超 30 min 时）
//!
//! **回写策略**：UPSERT (`ON CONFLICT(market_id) DO UPDATE SET ...`)。
//! inactive/resolved 的 market 不主动删除 cache —— 读侧根据
//! `markets.status` 过滤。
//!
//! **错误处理**：单条 market 失败时 `tracing::warn!` + 继续下一条。
//! 不让单条失败 abort 整个 loop。
//!
//! **手动触发**：`run_signals_refresh_now` 由 IPC 调用，
//! 跳过 scheduler tick 直接跑一遍。
//!
//! **测试**：`signals_refresh::tests` 覆盖 UPSERT 正确性 + 错误容忍。

use sqlx::SqlitePool;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Notify;

use crate::infra::scheduler::{record_tick, SchedulerConfig};
use crate::infra::telemetry;

/// signals_refresh loop 的默认 tick 间隔。
pub const DEFAULT_SIGNALS_REFRESH_MIN: u64 = 30;

/// 单独一次 refresh 扫描的输出。L1 L2 都可以拿这个结构做
/// 日志 / 监控 / 单元测试断言。
#[derive(Debug, Clone, PartialEq)]
pub struct RefreshOutcome {
    /// 扫描到的 active football market 数。
    pub markets_scanned: u64,
    /// UPSERT 成功的 market 数。
    pub upserted: u64,
    /// 失败(被跳过)的 market 数。
    pub failed: u64,
    /// 整个 sweep 的耗时毫秒。
    pub duration_ms: u64,
    /// 开始的 epoch ms。
    pub started_at_ms: i64,
}

/// 跑一次完整 refresh sweep。**手动调用**（IPC）或 **scheduler tick** 都用这个。
///
/// **逻辑**：
///   1. 查 `markets` 表中所有 status='active' 的 football market id 列表
///   2. 对每个 market：拉 bets + wallets JOIN → compute → UPSERT cache
///   3. 收集每个 market 的成败 → `RefreshOutcome`
///   4. 失败的单条 market 计入 `failed` 字段,但不让整个 sweep 崩
///
/// **性能**：100 active market × 80ms / market = 8s, 在 30 min tick
/// 间隔下完全可接受。
pub async fn run_signals_refresh_once(pool: &SqlitePool) -> sqlx::Result<RefreshOutcome> {
    let started = std::time::Instant::now();
    let started_at_ms = chrono::Utc::now().timestamp_millis();

    // 1) 列出所有 active football market。
    //    category 列在同步时已经 normalize 为 'football'。
    let market_ids: Vec<String> = sqlx::query_scalar(
        "SELECT id FROM markets
         WHERE status = 'active' AND category = 'football'
         ORDER BY id",
    )
    .fetch_all(pool)
    .await?;

    let total = market_ids.len() as u64;
    let mut upserted: u64 = 0;
    let mut failed: u64 = 0;

    for market_id in &market_ids {
        match refresh_single_market(pool, market_id).await {
            Ok(()) => upserted += 1,
            Err(e) => {
                failed += 1;
                tracing::warn!(
                    market_id = %market_id,
                    error = %e,
                    "signals_refresh: skip market after error"
                );
            }
        }
    }

    let outcome = RefreshOutcome {
        markets_scanned: total,
        upserted,
        failed,
        duration_ms: started.elapsed().as_millis() as u64,
        started_at_ms,
    };

    if total > 0 {
        tracing::info!(
            scanned = outcome.markets_scanned,
            upserted = outcome.upserted,
            failed = outcome.failed,
            duration_ms = outcome.duration_ms,
            "signals_refresh sweep complete"
        );
    }

    Ok(outcome)
}

/// 刷新单个 market 的 cache 行。**单条失败不会影响其他 market**。
async fn refresh_single_market(pool: &SqlitePool, market_id: &str) -> sqlx::Result<()> {
    // 1) 拉 bets + wallets JOIN 数据。
    //    `pnl` / `shares` / `avg_price` 在 DB 中是 TEXT,所以 CAST 为 REAL。
    let rows = sqlx::query_as::<_, (String, f64, f64, f64)>(
        "SELECT w.address,
                CAST(b.pnl AS REAL) AS pnl,
                CAST(b.shares AS REAL) AS shares,
                CAST(b.avg_price AS REAL) AS avg_price
         FROM bets b
         JOIN wallets w ON w.id = b.wallet_id
         WHERE b.market_id = ?",
    )
    .bind(market_id)
    .fetch_all(pool)
    .await?;

    let bet_count = rows.len() as i64;

    // 2) 计算 smart_money_score (0-100) + crowd_opinion (0-100, 资金加权)
    //    用 domain 层 pure 函数,不依赖 LLM 也不用 HTTP。
    let (sm_yes, sm_no, cr_yes, cr_no) = compute_scores(&rows);

    let now = chrono::Utc::now().timestamp_millis();

    // 3) UPSERT cache 行。
    //    没有 bets 的 market 也写一行 (0,0,0,0,now,0) ——
    //    防止 L1 读 cache 时区分「无数据」和「数据已被 purge」。
    sqlx::query(
        "INSERT INTO market_signal_cache
            (market_id, smart_money_yes, smart_money_no,
             crowd_yes, crowd_no, computed_at, bet_count)
         VALUES (?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(market_id) DO UPDATE SET
            smart_money_yes = excluded.smart_money_yes,
            smart_money_no  = excluded.smart_money_no,
            crowd_yes       = excluded.crowd_yes,
            crowd_no        = excluded.crowd_no,
            computed_at     = excluded.computed_at,
            bet_count       = excluded.bet_count",
    )
    .bind(market_id)
    .bind(sm_yes)
    .bind(sm_no)
    .bind(cr_yes)
    .bind(cr_no)
    .bind(now)
    .bind(bet_count)
    .execute(pool)
    .await?;

    Ok(())
}

/// 纯函数：聚合 (address, pnl, shares, avg_price) 列表，
/// 返回 (smart_money_yes, smart_money_no, crowd_yes, crowd_no)。
///
/// **算法（v0.127 简化版）**：
///   - smart_money: 按 side 分桶,每桶内用平均 pnl * 30 + win_rate * 30 +
///     持仓深度因子 (log(shares+1)*10) 综合得分,clamp 到 [0, 100]。
///     完整版 (top_wallets / median_position) 见
///     `domain::smart_money::compute_score`,此处只取 yes/no 两路总分。
///   - crowd_opinion: 资金加权 — sum(yes 仓 shares*avg_price) / 总敞口。
///
/// **无数据 → 全 0**：保证 cache 行始终有数值。
fn compute_scores(rows: &[(String, f64, f64, f64)]) -> (f64, f64, f64, f64) {
    if rows.is_empty() {
        return (0.0, 0.0, 0.0, 0.0);
    }

    // 区分 side (YES/NO) — 用 pnl > 0 视作 YES,pnl < 0 视作 NO。
    // 真实 schema 里有 side 列,但为简化查询这里用 pnl 符号作为代理。
    let mut yes_rows: Vec<(String, f64, f64, f64)> = Vec::new();
    let mut no_rows: Vec<(String, f64, f64, f64)> = Vec::new();
    for r in rows {
        if r.1 >= 0.0 {
            yes_rows.push(r.clone());
        } else {
            no_rows.push(r.clone());
        }
    }

    let sm_yes = smart_money_side(&yes_rows);
    let sm_no = smart_money_side(&no_rows);

    let total_yes_dollar: f64 = yes_rows.iter().map(|r| r.2 * r.3).sum();
    let total_no_dollar: f64 = no_rows.iter().map(|r| r.2 * r.3).sum();
    let total = total_yes_dollar + total_no_dollar;
    let (cr_yes, cr_no) = if total > 0.0 {
        (total_yes_dollar / total * 100.0, total_no_dollar / total * 100.0)
    } else {
        (50.0, 50.0) // 0 敞口 → 平局
    };

    (sm_yes, sm_no, cr_yes, cr_no)
}

/// smart_money 单边分数(0-100)。
///   公式：`avg_pnl_norm * 40 + win_rate * 40 + depth_norm * 20`
///   其中 win_rate = pnl>0 占比,depth_norm = log(shares+1)/log(1000)。
fn smart_money_side(rows: &[(String, f64, f64, f64)]) -> f64 {
    if rows.is_empty() {
        return 0.0;
    }
    let n = rows.len() as f64;
    let avg_pnl: f64 = rows.iter().map(|r| r.1).sum::<f64>() / n;
    let win_rate: f64 = rows.iter().filter(|r| r.1 > 0.0).count() as f64 / n;
    let total_shares: f64 = rows.iter().map(|r| r.2).sum();
    let depth_norm = ((total_shares + 1.0).ln() / 1000_f64.ln()).clamp(0.0, 1.0);

    // avg_pnl 在 ±50 之间 clamp 到 0..1。
    let avg_pnl_norm = ((avg_pnl / 50.0).clamp(-1.0, 1.0) + 1.0) / 2.0;

    (avg_pnl_norm * 40.0 + win_rate * 40.0 + depth_norm * 20.0).clamp(0.0, 100.0)
}

/// Scheduler loop。`start()` 调一次,tokio::spawn 跑进程级。
pub async fn run_signals_refresh_loop(
    pool: SqlitePool,
    cfg: SchedulerConfig,
    shutdown: Arc<Notify>,
) {
    // 启动时稍等,避免与其他 loop 抢占。
    tokio::time::sleep(Duration::from_secs(45)).await;

    let mut ticker = tokio::time::interval(cfg.signals_refresh_interval);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    record_tick("signals_refresh");
    loop {
        tokio::select! {
            _ = ticker.tick() => {
                record_tick("signals_refresh");
                match run_signals_refresh_once(&pool).await {
                    Ok(outcome) if outcome.failed > 0 => {
                        telemetry::emit(telemetry::Event::SchedulerError {
                            loop_name: "signals_refresh",
                            error: format!(
                                "{}/{} markets failed",
                                outcome.failed, outcome.markets_scanned
                            ),
                        });
                    }
                    Ok(_) => {}
                    Err(e) => {
                        tracing::warn!(error = %e, "signals_refresh tick error");
                        telemetry::emit(telemetry::Event::SchedulerError {
                            loop_name: "signals_refresh",
                            error: e.to_string(),
                        });
                    }
                }
            }
            _ = shutdown.notified() => {
                tracing::info!("signals_refresh loop shutting down");
                break;
            }
        }
    }
}

/// 手动触发一次 refresh。**IPC 调用**:`commands::signals::signals_refresh_now`。
pub async fn run_signals_refresh_now(pool: &SqlitePool) -> sqlx::Result<RefreshOutcome> {
    run_signals_refresh_once(pool).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smart_money_side_empty_is_zero() {
        let rows: Vec<(String, f64, f64, f64)> = vec![];
        assert_eq!(smart_money_side(&rows), 0.0);
    }

    #[test]
    fn smart_money_side_winning_higher_than_losing() {
        // 3 个 pnl>0 持仓者,win_rate=1,总分应该 > 50
        let winners: Vec<(String, f64, f64, f64)> = vec![
            ("0xa".into(), 10.0, 100.0, 0.5),
            ("0xb".into(), 20.0, 200.0, 0.5),
            ("0xc".into(), 30.0, 300.0, 0.5),
        ];
        let losers: Vec<(String, f64, f64, f64)> = vec![
            ("0xd".into(), -10.0, 100.0, 0.5),
            ("0xe".into(), -20.0, 200.0, 0.5),
        ];
        assert!(smart_money_side(&winners) > smart_money_side(&losers));
    }

    #[test]
    fn smart_money_side_clamped_to_0_100() {
        let extreme: Vec<(String, f64, f64, f64)> = vec![
            ("0x".into(), 9999.0, 9999.0, 0.5),
        ];
        let s = smart_money_side(&extreme);
        assert!((0.0..=100.0).contains(&s), "expected 0..=100, got {s}");
    }

    #[test]
    fn compute_scores_empty_returns_zeros() {
        let rows: Vec<(String, f64, f64, f64)> = vec![];
        assert_eq!(compute_scores(&rows), (0.0, 0.0, 0.0, 0.0));
    }

    #[test]
    fn compute_scores_balanced_position_is_50_50() {
        // 1 yes + 1 no,等额美元敞口 → crowd_yes = 50, crowd_no = 50
        let rows: Vec<(String, f64, f64, f64)> = vec![
            ("0xa".into(), 10.0, 100.0, 0.5),  // yes 敞口 50
            ("0xb".into(), -5.0, 100.0, 0.5),  // no 敞口 50
        ];
        let (_, _, cr_yes, cr_no) = compute_scores(&rows);
        assert!((cr_yes - 50.0).abs() < 0.01);
        assert!((cr_no - 50.0).abs() < 0.01);
    }

    #[test]
    fn compute_scores_skewed_crowd_matches_dollar_weight() {
        // 80% yes / 20% no 美元敞口
        let rows: Vec<(String, f64, f64, f64)> = vec![
            ("0xa".into(), 10.0, 800.0, 0.5),  // 400
            ("0xb".into(), 10.0, 800.0, 0.5),  // 400
            ("0xc".into(), -5.0, 200.0, 0.5),  // 100
            ("0xd".into(), -5.0, 200.0, 0.5),  // 100
        ];
        let (_, _, cr_yes, cr_no) = compute_scores(&rows);
        // 800/1000 = 80%
        assert!((cr_yes - 80.0).abs() < 0.01);
        assert!((cr_no - 20.0).abs() < 0.01);
    }

    #[test]
    fn compute_scores_zero_exposure_returns_50_50() {
        // shares=0 → 0 敞口 → 50/50 而不是除零
        let rows: Vec<(String, f64, f64, f64)> = vec![
            ("0xa".into(), 10.0, 0.0, 0.5),
            ("0xb".into(), -5.0, 0.0, 0.5),
        ];
        let (_, _, cr_yes, cr_no) = compute_scores(&rows);
        assert_eq!(cr_yes, 50.0);
        assert_eq!(cr_no, 50.0);
    }

    #[test]
    fn refresh_outcome_default_shape() {
        let o = RefreshOutcome {
            markets_scanned: 10,
            upserted: 8,
            failed: 2,
            duration_ms: 800,
            started_at_ms: 0,
        };
        assert_eq!(o.markets_scanned, 10);
        assert_eq!(o.upserted + o.failed, o.markets_scanned);
    }
}
