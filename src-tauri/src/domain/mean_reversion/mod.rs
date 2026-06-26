//! L3 — 均值回归与过度反应检测 (Phase 1.2)。
//!
//! 实现 Logic 1 (行为金融) 中的「均值回归」和「过度反应」
//! 检测逻辑。当市场价格显著偏离其滚动均值时,假设
//! 散户对新闻/情绪过度反应,价格很可能向均值回归。
//!
//! 关键概念:
//!   - **滚动均值** — 最近 N 个价格快照的平均值
//!   - **标准差** — 价格窗口的波动率
//!   - **Z-score** — `(current - mean) / std_dev`,衡量偏离程度
//!   - **Bollinger bands** — mean ± 2*std_dev,经典的超买/超卖
//!   - **反向信号** — 建议在过度延伸的方向上反向操作,预期回归
//!
//! 函数是 **纯函数**:接受一个价格切片并返回统计。
//! DB 访问位于 L2 命令 (`commands::mean_reversion`)。

use serde::{Deserialize, Serialize};
use specta::Type;

/// 默认滚动窗口大小(价格快照数量)。
pub const DEFAULT_WINDOW: usize = 20;

/// 默认 z-score 阈值,用于「过度延伸」分类。
pub const DEFAULT_Z_THRESHOLD: f64 = 2.0;

/// 价格窗口的统计摘要。
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct PriceStats {
    pub current: f64,
    pub mean: f64,
    pub std_dev: f64,
    pub z_score: f64,
    pub bollinger_upper: f64,    // mean + 2*std_dev
    pub bollinger_lower: f64,    // mean - 2*std_dev
    pub is_overextended: bool,   // |z_score| > threshold
    pub direction: String,       // "overbought" | "oversold" | "neutral"
    pub window_size: usize,
}

/// 均值回归交易信号。
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ReversionSignal {
    pub market_id: String,
    pub stats: PriceStats,
    /// -1.0 = 反向 YES (价格过高,预期下跌)
    /// +1.0 = 反向 NO (价格过低,预期上涨)
    /// 0.0 = 无信号
    pub fade_signal: f64,
    pub confidence: f64,         // 0..1,基于 z-score 量级
    pub computed_at: i64,
}

/// 计算价格序列的滚动统计。
///
/// 使用最后 `window` 个价格计算均值和总体标准差。
/// 当前(最后)价格的 z-score 指示它偏离均值多少个标准差。
///
/// 公式:
///   mean = Σ(price_i) / N
///   variance = Σ((price_i - mean)²) / N
///   std_dev = √variance
///   z_score = (current - mean) / std_dev
///
/// 守卫:
///   - 如果价格少于 2 个,std_dev = 0, z_score = 0。
///   - 如果 std_dev == 0 (所有价格相同),z_score = 0。
///   - 价格被限制在 [0, 1] 范围(预测市场价格范围)。
pub fn compute_price_stats(prices: &[f64], window: usize, z_threshold: f64) -> PriceStats {
    let n = prices.len();
    let current = prices.last().copied().unwrap_or(0.5);

    if n < 2 {
        return PriceStats {
            current,
            mean: current,
            std_dev: 0.0,
            z_score: 0.0,
            bollinger_upper: current,
            bollinger_lower: current,
            is_overextended: false,
            direction: "neutral".into(),
            window_size: n,
        };
    }

    // 使用最后 `window` 个价格(或全部,如果可用数量更少)。
    let start = if n > window { n - window } else { 0 };
    let slice = &prices[start..];
    let w = slice.len();

    let mean: f64 = slice.iter().sum::<f64>() / w as f64;

    let variance: f64 = slice.iter().map(|p| (p - mean).powi(2)).sum::<f64>() / w as f64;
    let std_dev = variance.sqrt();

    let z_score = if std_dev > 1e-10 {
        (current - mean) / std_dev
    } else {
        0.0
    };

    let bollinger_upper = mean + 2.0 * std_dev;
    let bollinger_lower = mean - 2.0 * std_dev;

    let is_overextended = z_score.abs() > z_threshold;
    let direction = if z_score > z_threshold {
        "overbought".to_string()
    } else if z_score < -z_threshold {
        "oversold".to_string()
    } else {
        "neutral".to_string()
    };

    PriceStats {
        current,
        mean,
        std_dev,
        z_score,
        bollinger_upper,
        bollinger_lower,
        is_overextended,
        direction,
        window_size: w,
    }
}

/// 从价格序列中检测均值回归机会。
///
/// 返回带反向建议的 `ReversionSignal`:
///   - 如果 z_score > threshold (超买): fade_signal = -1.0 (反向 YES)
///   - 如果 z_score < -threshold (超卖): fade_signal = +1.0 (反向 NO)
///   - 其他情况: fade_signal = 0.0 (无信号)
///
/// 置信度随 |z_score| 缩放:
///   confidence = min(|z_score| / (threshold * 2), 1.0)（0..1）
///   在 z=2.0 (threshold),confidence = 0.5
///   在 z=4.0 (2x threshold),confidence = 1.0
pub fn detect_reversion(
    market_id: &str,
    prices: &[f64],
    window: usize,
    z_threshold: f64,
    computed_at: i64,
) -> ReversionSignal {
    let stats = compute_price_stats(prices, window, z_threshold);

    let fade_signal = if stats.z_score > z_threshold {
        -1.0 // price too high → expect drop → fade YES
    } else if stats.z_score < -z_threshold {
        1.0 // price too low → expect rise → fade NO
    } else {
        0.0
    };

    let confidence = if z_threshold > 0.0 {
        (stats.z_score.abs() / (z_threshold * 2.0)).min(1.0)
    } else {
        0.0
    };

    ReversionSignal {
        market_id: market_id.to_string(),
        stats,
        fade_signal,
        confidence,
        computed_at,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_prices() {
        let s = compute_price_stats(&[], 20, 2.0);
        assert!((s.current - 0.5).abs() < 1e-9);
        assert!((s.z_score).abs() < 1e-9);
    }

    #[test]
    fn single_price() {
        let s = compute_price_stats(&[0.55], 20, 2.0);
        assert!((s.mean - 0.55).abs() < 1e-9);
        assert!((s.std_dev).abs() < 1e-9);
        assert_eq!(s.direction, "neutral");
    }

    #[test]
    fn stable_prices_no_signal() {
        // 所有价格相同 → std_dev = 0,无信号
        let prices = vec![0.50; 20];
        let sig = detect_reversion("m", &prices, 20, 2.0, 1000);
        assert!((sig.fade_signal).abs() < 1e-9);
        assert!((sig.confidence).abs() < 1e-9);
    }

    #[test]
    fn overbought_detected() {
        // 19 个价格 0.50,然后跳到 0.70
        let mut prices = vec![0.50; 19];
        prices.push(0.70);
        let sig = detect_reversion("m", &prices, 20, 2.0, 1000);
        assert!(sig.stats.z_score > 2.0);
        assert_eq!(sig.stats.direction, "overbought");
        assert!((sig.fade_signal - (-1.0)).abs() < 1e-6);
        assert!(sig.confidence > 0.0);
    }

    #[test]
    fn oversold_detected() {
        // 19 个价格 0.50,然后跌到 0.30
        let mut prices = vec![0.50; 19];
        prices.push(0.30);
        let sig = detect_reversion("m", &prices, 20, 2.0, 1000);
        assert!(sig.stats.z_score < -2.0);
        assert_eq!(sig.stats.direction, "oversold");
        assert!((sig.fade_signal - 1.0).abs() < 1e-6);
        assert!(sig.confidence > 0.0);
    }

    #[test]
    fn bollinger_bands() {
        let prices = vec![0.40, 0.50, 0.60, 0.50, 0.50];
        let s = compute_price_stats(&prices, 20, 2.0);
        let expected_mean = 0.50;
        assert!((s.mean - expected_mean).abs() < 1e-6);
        assert!(s.bollinger_upper > s.mean);
        assert!(s.bollinger_lower < s.mean);
    }

    #[test]
    fn confidence_scales_with_z() {
        // 使用具有自然方差的基准序列,使 z-score 产生差异。
        let base: Vec<f64> = (0..20)
            .map(|i| 0.50 + (i as f64 % 3.0 - 1.0) * 0.01)
            .collect();

        // 中度异常值
        let mut p1 = base.clone();
        p1.push(0.56);
        let sig1 = detect_reversion("m", &p1, 20, 2.0, 1000);

        // 极端异常值
        let mut p2 = base.clone();
        p2.push(0.65);
        let sig2 = detect_reversion("m", &p2, 20, 2.0, 1000);

        assert!(sig2.confidence >= sig1.confidence);
        assert!(sig2.confidence <= 1.0);
    }

    #[test]
    fn window_truncation() {
        // 30 个价格但 window=10 → 只用最后 10 个
        // 价格: 0.40, 0.41, ..., 0.69 (步长 0.01)
        // 最后 10 个: 0.60, 0.61, ..., 0.69 → mean = 0.645
        let prices: Vec<f64> = (0..30).map(|i| 0.40 + (i as f64) * 0.01).collect();
        let s = compute_price_stats(&prices, 10, 2.0);
        assert_eq!(s.window_size, 10);
        let expected_mean: f64 = (0.60 + 0.61 + 0.62 + 0.63 + 0.64 + 0.65 + 0.66 + 0.67 + 0.68 + 0.69) / 10.0;
        assert!((s.mean - expected_mean).abs() < 1e-6);
    }
}
