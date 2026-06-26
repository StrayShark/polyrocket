//! L3 — Poisson 比分矩阵（P2-1）。
//!
//! 为足球比赛比分计算 Dixon-Coles Poisson 模型。
//! 给定主队和客队的预期进球数（lambda），生成 5x5 比分概率矩阵
//! 以及最可能的单场比分。
//!
//! 规范：P2-1 Poisson Score Matrix

use serde::{Deserialize, Serialize};
use specta::Type;

/// Poisson 比分矩阵计算结果。
///
/// `matrix[i][j]` = P(主队=i, 客队=j)，i,j ∈ 0..=4。
/// `most_likely` 是按概率降序排列的前 N 个比分，每项形如 `("2-1", 0.083)`。
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ScoreMatrixResult {
    pub lambda_h: f64,
    pub lambda_a: f64,
    pub matrix: [[f64; 5]; 5],
    pub most_likely: Vec<(String, f64)>,
}

/// Poisson 概率质量函数:P(X=k) = (lambda^k * e^-lambda) / k!
pub fn poisson_pmf(k: usize, lambda: f64) -> f64 {
    // e^-lambda（指数项）
    let exp_term = (-lambda).exp();
    // lambda^k / k!（阶乘项）
    let mut pow_term = 1.0;
    for i in 1..=k {
        pow_term *= lambda / (i as f64);
    }
    exp_term * pow_term
}

/// 计算给定 lambda 下完整的 5x5 Poisson 比分矩阵。
///
/// P(主队=i, 客队=j) = poisson_pmf(i, lambda_h) * poisson_pmf(j, lambda_a)，
/// i, j ∈ 0..=4。返回矩阵以及概率最高的 5 个比分。
pub fn compute_poisson_matrix(lambda_h: f64, lambda_a: f64) -> ScoreMatrixResult {
    let mut matrix = [[0.0f64; 5]; 5];
    let home_probs: Vec<f64> = (0..5).map(|i| poisson_pmf(i, lambda_h)).collect();
    let away_probs: Vec<f64> = (0..5).map(|j| poisson_pmf(j, lambda_a)).collect();

    for i in 0..5 {
        for j in 0..5 {
            matrix[i][j] = home_probs[i] * away_probs[j];
        }
    }

    let most_likely = most_likely_scores(&matrix, 5);

    ScoreMatrixResult {
        lambda_h,
        lambda_a,
        matrix,
        most_likely,
    }
}

/// 应用 Dixon-Coles 低分调整。
///
/// Dixon-Coles 模型针对低比分结果（0-0、1-1、0-1、1-0）
/// 对 Poisson 独立性假设进行修正,通过基于 `rho` 的因子
/// 乘以它们的概率:
///
/// - P(0,0) *= 1 - (lambda_h * lambda_a * rho)
/// - P(0,1) *= 1 + (lambda_h * rho)
/// - P(1,0) *= 1 + (lambda_a * rho)
/// - P(1,1) *= 1 - rho
///
/// 调用方传入矩阵与 rho 参数（通常在 -0.1..0.1 区间）。
pub fn apply_dixon_coles_adjustment(matrix: &mut [[f64; 5]; 5], rho: f64, lambda_h: f64, lambda_a: f64) {
    // P(0,0) *= 1 - lambda_h * lambda_a * rho
    matrix[0][0] *= 1.0 - (lambda_h * lambda_a * rho);
    // P(1,1) *= 1 - rho
    matrix[1][1] *= 1.0 - rho;
    // P(0,1) *= 1 + lambda_h * rho
    matrix[0][1] *= 1.0 + (lambda_h * rho);
    // P(1,0) *= 1 + lambda_a * rho
    matrix[1][0] *= 1.0 + (lambda_a * rho);
}

/// 从 5x5 概率矩阵中返回概率最高的前 `n` 个比分。
///
/// 每项为 `("i-j", probability)`,按概率降序排列。
pub fn most_likely_scores(matrix: &[[f64; 5]; 5], n: usize) -> Vec<(String, f64)> {
    let mut all: Vec<(String, f64)> = Vec::with_capacity(25);
    for i in 0..5 {
        for j in 0..5 {
            all.push((format!("{}-{}", i, j), matrix[i][j]));
        }
    }
    all.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    all.truncate(n);
    all
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn poisson_pmf_basic() {
        // P(X=0) = e^-lambda（Poisson 质量函数）
        let p0 = poisson_pmf(0, 1.0);
        assert!((p0 - std::f64::consts::E.powf(-1.0)).abs() < 1e-9);
    }

    #[test]
    fn poisson_pmf_sums_near_one() {
        // 在 0..100 上对 Poisson 求和应近似为 1
        let lambda = 1.5;
        let sum: f64 = (0..100).map(|k| poisson_pmf(k, lambda)).sum();
        assert!((sum - 1.0).abs() < 1e-6);
    }

    #[test]
    fn matrix_sums_near_one() {
        let result = compute_poisson_matrix(1.5, 1.0);
        let total: f64 = result.matrix.iter().flat_map(|r| r.iter()).sum();
        // 5x5 捕获了大部分概率（但不包括 4 球以上的尾部）
        assert!(total > 0.85 && total < 1.0);
    }

    #[test]
    fn most_likely_sorted_desc() {
        let result = compute_poisson_matrix(1.5, 1.0);
        for w in result.most_likely.windows(2) {
            assert!(w[0].1 >= w[1].1);
        }
    }

    #[test]
    fn dixon_coles_adjusts_low_scores() {
        let mut m = [[0.1f64; 5]; 5];
        let p00_before = m[0][0];
        let p11_before = m[1][1];
        let lambda_h = 1.5;
        let lambda_a = 1.0;
        apply_dixon_coles_adjustment(&mut m, 0.1, lambda_h, lambda_a);
        // rho=0.1: P(0,0) *= 1 - (1.5*1.0*0.1) = 0.85; P(1,1) *= 0.9（数值校核）
        assert!((m[0][0] - p00_before * 0.85).abs() < 1e-9);
        assert!((m[1][1] - p11_before * 0.9).abs() < 1e-9);
    }
}
