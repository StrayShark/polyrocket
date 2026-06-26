//! L3 — 尖峰检测（P0-3）。
//!
//! 通过比较同一市场连续 `price_snapshots` 行的方式来检测
//! 剧烈价格波动。当两个连续快照之间 `mid_price` 的百分比变化
//! 超过阈值（默认 5%）时,标记为尖峰。
//!
//! 检测逻辑是**纯函数**：接收 `PriceSnapshotRow` 切片并
//! 返回 `Vec<SpikeAlert>`。所有数据库访问（读快照、写告警）
//! 都位于 L2 命令（`commands::spike`）。

use serde::{Deserialize, Serialize};
use specta::Type;

/// 由 `detect_spikes` 消费的扁平化价格快照行。
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct PriceSnapshotRow {
    pub id: i64,
    pub market_id: String,
    pub captured_at: i64,
    pub mid_price: f64,
}

/// 已检测到的价格尖峰。镜像 SQLite `spike_alerts` 表。
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SpikeAlert {
    pub id: i64,
    pub market_id: String,
    pub old_price: f64,
    pub new_price: f64,
    pub change_pct: f64,
    pub detected_at: i64,
    pub market_question: Option<String>,
}

/// 默认价格异动阈值：5%。
pub const DEFAULT_SPIKE_THRESHOLD_PCT: f64 = 5.0;

/// 当 `old` 到 `new` 的百分比变化超过 `threshold`（以百分点计，例如 `5.0` = 5%）时返回 `true`。
///
/// 防止 `old == 0`（视为无尖峰，避免除零）。
pub fn is_significant_spike(old: f64, new: f64, threshold: f64) -> bool {
    if old <= 0.0 {
        return false;
    }
    let change_pct = ((new - old) / old).abs() * 100.0;
    change_pct > threshold
}

/// 比较连续的快照（按 `captured_at` 升序排列）,标记
/// 任何 `mid_price` 变动超过 `threshold_pct` 的配对。
///
/// **假设** `snapshots` 已经按 `captured_at` 升序排序
/// 并过滤到单一市场（命令层负责 `ORDER BY`）。
/// 跨市场比较没有意义,因为价格是独立的。
///
/// 返回的告警 `id` 从 0 开始单调递增;命令层在
/// 插入时重新分配真正的 DB id。
pub fn detect_spikes(snapshots: &[PriceSnapshotRow], threshold_pct: f64) -> Vec<SpikeAlert> {
    if snapshots.len() < 2 {
        return vec![];
    }

    let mut alerts = Vec::new();
    let mut id = 0i64;

    for window in snapshots.windows(2) {
        let prev = &window[0];
        let curr = &window[1];

        if prev.market_id != curr.market_id {
            continue;
        }
        if !is_significant_spike(prev.mid_price, curr.mid_price, threshold_pct) {
            continue;
        }

        let change_pct = if prev.mid_price > 0.0 {
            ((curr.mid_price - prev.mid_price) / prev.mid_price) * 100.0
        } else {
            0.0
        };

        alerts.push(SpikeAlert {
            id,
            market_id: curr.market_id.clone(),
            old_price: prev.mid_price,
            new_price: curr.mid_price,
            change_pct,
            detected_at: curr.captured_at,
            market_question: None,
        });
        id += 1;
    }

    alerts
}

/// 统计尖峰检测的默认 z-score 阈值。
pub const DEFAULT_Z_THRESHOLD: f64 = 2.0;

/// 统计基线的默认滚动窗口大小。
pub const DEFAULT_STAT_WINDOW: usize = 20;

/// 使用滚动统计基线的增强型尖峰检测。
///
/// 与使用固定百分比阈值不同,这里在先前的 `window` 个
/// 快照上计算滚动均值与标准差,然后将任何 z-score 超过
/// `z_threshold` 的快照标记为尖峰。
///
/// 这比 `detect_spikes` 更具自适应性:在高波动性市场中,
/// 10% 的变动可能属于正常范围（不构成尖峰）;而在
/// 平静市场中,3% 的变动可能就是 3-sigma 离群点。
///
/// **假设** `snapshots` 已按 `captured_at` 升序排序
/// 并过滤到单一市场。
pub fn detect_spikes_statistical(
    snapshots: &[PriceSnapshotRow],
    window: usize,
    z_threshold: f64,
) -> Vec<SpikeAlert> {
    if snapshots.len() < 3 {
        return vec![];
    }

    let mut alerts = Vec::new();
    let mut id = 0i64;

    for i in 2..snapshots.len() {
        let curr = &snapshots[i];
        // 基于前置 `window` 个快照（或不足时全部）构建基线。
        let start = if i > window { i - window } else { 0 };
        let baseline = &snapshots[start..i];

        // 不同市场则跳过（如果已预过滤则不应发生）。
        if baseline.iter().any(|s| s.market_id != curr.market_id) {
            continue;
        }

        let prices: Vec<f64> = baseline.iter().map(|s| s.mid_price).collect();
        let n = prices.len() as f64;
        if n < 2.0 {
            continue;
        }

        let mean: f64 = prices.iter().sum::<f64>() / n;
        let variance: f64 = prices.iter().map(|p| (p - mean).powi(2)).sum::<f64>() / n;
        let std_dev = variance.sqrt();

        if std_dev < 1e-10 || mean <= 0.0 {
            continue;
        }

        let z_score = (curr.mid_price - mean) / std_dev;

        if z_score.abs() > z_threshold {
            let change_pct = if mean > 0.0 {
                ((curr.mid_price - mean) / mean) * 100.0
            } else {
                0.0
            };

            alerts.push(SpikeAlert {
                id,
                market_id: curr.market_id.clone(),
                old_price: mean,
                new_price: curr.mid_price,
                change_pct,
                detected_at: curr.captured_at,
                market_question: None,
            });
            id += 1;
        }
    }

    alerts
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(id: i64, market: &str, at: i64, mid: f64) -> PriceSnapshotRow {
        PriceSnapshotRow {
            id,
            market_id: market.into(),
            captured_at: at,
            mid_price: mid,
        }
    }

    #[test]
    fn is_significant_spike_basic() {
        assert!(is_significant_spike(0.50, 0.55, 5.0));  // +10%
        assert!(is_significant_spike(0.50, 0.45, 5.0));  // -10%
        assert!(!is_significant_spike(0.50, 0.52, 5.0)); // +4%
        assert!(!is_significant_spike(0.0, 1.0, 5.0));   // div-by-zero guard
    }

    #[test]
    fn detect_spikes_empty() {
        assert!(detect_spikes(&[], 5.0).is_empty());
        assert!(detect_spikes(&[snap(1, "m", 100, 0.5)], 5.0).is_empty());
    }

    #[test]
    fn detect_spikes_finds_movement() {
        let snaps = vec![
            snap(1, "m", 100, 0.50),
            snap(2, "m", 200, 0.55), // +10% → spike
            snap(3, "m", 300, 0.56), // +2% → no spike
            snap(4, "m", 400, 0.40), // -28.6% → spike
        ];
        let alerts = detect_spikes(&snaps, 5.0);
        assert_eq!(alerts.len(), 2);
        assert_eq!(alerts[0].market_id, "m");
        assert!((alerts[0].change_pct - 10.0).abs() < 1e-9);
        assert!(alerts[1].change_pct < 0.0);
    }

    #[test]
    fn detect_spikes_ignores_cross_market() {
        let snaps = vec![
            snap(1, "a", 100, 0.50),
            snap(2, "b", 200, 0.90), // 不同市场,变动大但被忽略
        ];
        let alerts = detect_spikes(&snaps, 5.0);
        assert!(alerts.is_empty());
    }

    #[test]
    fn detect_spikes_threshold_respected() {
        let snaps = vec![
            snap(1, "m", 100, 0.50),
            snap(2, "m", 200, 0.52), // +4% → below 5%
        ];
        let alerts = detect_spikes(&snaps, 5.0);
        assert!(alerts.is_empty());
    }

    // -- 统计尖峰检测测试 --

    #[test]
    fn stat_detect_too_few_snapshots() {
        assert!(detect_spikes_statistical(&[], 20, 2.0).is_empty());
        assert!(detect_spikes_statistical(&[snap(1, "m", 100, 0.5)], 20, 2.0).is_empty());
        assert!(detect_spikes_statistical(&[snap(1, "m", 100, 0.5), snap(2, "m", 200, 0.5)], 20, 2.0).is_empty());
    }

    #[test]
    fn stat_detect_finds_outlier() {
        // 20 个稳定价格为 0.50,然后尖峰到 0.70
        let mut snaps: Vec<PriceSnapshotRow> = (0..20)
            .map(|i| snap(i, "m", i * 100, 0.50))
            .collect();
        snaps.push(snap(20, "m", 2000, 0.70));
        let alerts = detect_spikes_statistical(&snaps, 20, 2.0);
        assert_eq!(alerts.len(), 1);
        assert!((alerts[0].new_price - 0.70).abs() < 1e-6);
    }

    #[test]
    fn stat_detect_ignores_normal_volatility() {
        // 价格在 0.50 附近小幅波动 —— 不构成尖峰
        let snaps: Vec<PriceSnapshotRow> = (0..25)
            .map(|i| snap(i, "m", i * 100, 0.50 + (i as f64 % 3.0) * 0.005))
            .collect();
        let alerts = detect_spikes_statistical(&snaps, 20, 2.0);
        assert!(alerts.is_empty());
    }

    #[test]
    fn stat_detect_adaptive_threshold() {
        // 高波动性市场:0.30、0.70、0.30、0.70、…… 然后 0.75
        // 固定 5% 的阈值会标记 0.75,但 z-score 不会,
        // 因为 std_dev 已经非常高。
        let mut snaps: Vec<PriceSnapshotRow> = (0..20)
            .map(|i| snap(i, "m", i * 100, if i % 2 == 0 { 0.30 } else { 0.70 }))
            .collect();
        snaps.push(snap(20, "m", 2000, 0.75)); // 在正常范围内
        let alerts = detect_spikes_statistical(&snaps, 20, 2.0);
        // 鉴于高方差,0.75 的 z_score 应 < 2.0
        assert!(alerts.is_empty());
    }
}
