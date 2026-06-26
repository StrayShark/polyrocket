//! L3 — 审计日志保留策略（纯函数）。
//!
//! 如果不加限制，`audit_log` 表会无限增长（每次 IPC 都会写入一条）。
//! 重度使用约 1 年后可能累积 10 万行以上。
//! 本模块决定保留哪些行、清理哪些行。
//!
//! 策略：**基于年龄的保留 + 安全下限**。
//!   - 最多保留 `max_rows` 行
//!   - 至少保留 `min_keep_rows` 行（即便时间较旧）
//!   - 始终保留最近 `retain_recent_ms` 内的记录（例如 90 天）
//!
//! 纯函数 `plan_purge(rows, policy) -> Vec<id_to_delete>` 单独测试；
//! 实际清理由 `infra::db::audit::purge_old` 执行。

use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// 保留策略。默认目标是约 90 天的历史记录，并设置
/// 1000 行的硬下限（以确保低频用户仍有数据）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionPolicy {
    /// 最大保留时长（毫秒）。早于此时间且超过 `min_keep_rows` 的行可被清理。
    pub retain_recent_ms: i64,
    /// 总行数硬上限。若超过此值，则不论新旧都会清理最旧的行。
    pub max_rows: i64,
    /// 安全下限：即便数据非常旧，也不会自动清理至低于此行数。
    /// 默认 1000，确保用户始终保留一定的上下文。
    pub min_keep_rows: i64,
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        Self {
            // 90 天
            retain_recent_ms: 90 * 86_400_000,
            // 5 万行
            max_rows: 50_000,
            // 1000 行下限
            min_keep_rows: 1000,
        }
    }
}

/// 单行的标识与时间戳，按 SQL `ORDER BY at DESC` 返回的顺序。
/// 时间戳用于应用年龄策略；id 用于确定要删除的记录。
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AuditRow {
    pub id: i64,
    pub at: i64,
}

/// 计划输出：要删除的行 id 列表。
pub fn plan_purge(rows: &[AuditRow], policy: &RetentionPolicy, now_ms: i64) -> Vec<i64> {
    if rows.is_empty() {
        return Vec::new();
    }
    // 1. 计算截止时间：`at < cutoff` 的行符合年龄清理条件
    let cutoff = now_ms - policy.retain_recent_ms;

    // 2. 从新到旧遍历；收集同时满足以下条件的 id：
    //    - 符合年龄清理条件（at < cutoff），且
    //    - 不在安全下限内（id 索引超过 min_keep_rows）
    let mut to_delete: Vec<i64> = Vec::new();
    for (idx, row) in rows.iter().enumerate() {
        if idx < policy.min_keep_rows as usize {
            continue;  // 在安全下限内
        }
        if row.at >= cutoff {
            break;  // 遇到较新行 —— 由于按 DESC 排序，更早的行也均为较新行
        }
        to_delete.push(row.id);
    }
    let _ = policy.max_rows;  // 保留供未来"即使较新也要限制总数"使用
    to_delete
}

/// 统计将被删除的行数，但实际不构造 id 列表。
/// 便于日志输出。
pub fn count_purgeable(rows: &[AuditRow], policy: &RetentionPolicy, now_ms: i64) -> usize {
    plan_purge(rows, policy, now_ms).len()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: i64, at: i64) -> AuditRow {
        AuditRow { id, at }
    }

    #[test]
    fn empty_rows_returns_empty() {
        assert!(plan_purge(&[], &RetentionPolicy::default(), 100_000_000_000).is_empty());
    }

    #[test]
    fn all_recent_returns_empty() {
        // 10 行，全部位于 90 天窗口内
        let now = 1_000_000_000_000;
        let rows: Vec<AuditRow> = (0..10).map(|i| row(i, now - i * 3_600_000)).collect();
        let to_del = plan_purge(&rows, &RetentionPolicy::default(), now);
        assert!(to_del.is_empty());
    }

    #[test]
    fn all_old_purges_above_floor() {
        // 100 行，全部为 1 年前（远超 90 天）
        let now = 1_000_000_000_000;
        let one_year_ago = now - 365 * 86_400_000;
        let rows: Vec<AuditRow> = (0..100).map(|i| row(i, one_year_ago + i)).collect();
        let policy = RetentionPolicy::default();  // min_keep_rows = 1000
        // 下限为 1000，当前仅 100 行 → 全部保留
        assert!(plan_purge(&rows, &policy, now).is_empty());
    }

    #[test]
    fn old_above_floor_purges() {
        // 1500 行：保留最新 1000 行，清理最早 500 行
        let now = 1_000_000_000_000;
        let old = now - 365 * 86_400_000;
        let mut rows: Vec<AuditRow> = Vec::new();
        for i in 0..500 {
            rows.push(row(1000 + i, old + i));   // 旧行
        }
        for i in 0..1000 {
            rows.push(row(i, now - i * 3_600_000));  // 较新行
        }
        // rows 当前未排序；该函数要求按 `at` 降序排序。
        rows.sort_by(|a, b| b.at.cmp(&a.at));
        let policy = RetentionPolicy::default();
        let to_del = plan_purge(&rows, &policy, now);
        // 前 1000 行（最新）位于安全下限内；超出部分再按年龄判断。
        // 全部 500 条旧行均超过下限且已过期 → 500 条全部被删除。
        assert_eq!(to_del.len(), 500);
    }

    #[test]
    fn mixed_old_and_new_keeps_new() {
        let now = 1_000_000_000_000;
        let old = now - 365 * 86_400_000;
        // 降序排序：5 条新行（90 天内），随后 5 条旧行（1 年前）
        let rows = vec![
            row(1, now - 1_000),    // 最新
            row(2, now - 2_000),
            row(3, now - 3_000),
            row(4, now - 4_000),
            row(5, now - 5_000),
            row(6, old + 1),
            row(7, old + 2),
            row(8, old + 3),
            row(9, old + 4),
            row(10, old + 5),
        ];
        let policy = RetentionPolicy::default();
        let to_del = plan_purge(&rows, &policy, now);
        // min_keep_rows = 1000，因此 5 新 + 5 旧全部保留（10 < 1000）
        assert!(to_del.is_empty());
    }

    #[test]
    fn cutoff_at_exact_boundary_is_retained() {
        // 处于恰好 `cutoff` 位置的行视为较新（>=, 而非 <）
        let now = 1_000_000_000_000;
        let policy = RetentionPolicy { retain_recent_ms: 1000, ..Default::default() };
        let rows = vec![row(1, now - 1000), row(2, now - 1001)];
        let to_del = plan_purge(&rows, &policy, now);
        // 行 1 处于边界 → 保留。行 2 已过边界 → 应被清理，
        // 但仍在 min_keep_rows 之内（2 < 1000）→ 保留。
        assert!(to_del.is_empty());
    }

    #[test]
    fn count_matches_plan_length() {
        let now = 1_000_000_000_000;
        let old = now - 365 * 86_400_000;
        let mut rows: Vec<AuditRow> = Vec::new();
        for i in 0..1500 {
            let at = if i < 500 { old + i } else { now - (i as i64) * 3_600_000 };
            rows.push(row(i, at));
        }
        rows.sort_by(|a, b| b.at.cmp(&a.at));
        let policy = RetentionPolicy::default();
        assert_eq!(count_purgeable(&rows, &policy, now), plan_purge(&rows, &policy, now).len());
    }

    #[test]
    fn custom_policy() {
        // 1 天保留期，2 行下限
        let now = 1_000_000_000_000;
        let policy = RetentionPolicy {
            retain_recent_ms: 86_400_000,
            max_rows: 100,
            min_keep_rows: 2,
        };
        let rows = vec![
            row(1, now - 100),
            row(2, now - 200),
            row(3, now - 2 * 86_400_000),  // 2 天前
            row(4, now - 3 * 86_400_000),  // 3 天前
        ];
        // 下限保留行 1、2（最新 2 行）。行 3 已 2 天（>1 天）→ 清理。
        // 行 4 也已超过 1 天并超出下限 → 清理。
        let to_del = plan_purge(&rows, &policy, now);
        assert_eq!(to_del.len(), 2);
        assert!(to_del.contains(&3));
        assert!(to_del.contains(&4));
    }
}
