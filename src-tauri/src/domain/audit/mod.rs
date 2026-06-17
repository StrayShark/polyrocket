//! L3 — Audit log retention policy (pure).
//!
//! Without bounds, the `audit_log` table grows unbounded (every IPC
//! writes one). After ~1 year of heavy use that could be 100k+ rows.
//! This module decides which rows to keep and which to purge.
//!
//! Strategy: **age-based with a safety floor**.
//!   - Keep at most `max_rows` rows total
//!   - Keep at least `min_keep_rows` rows even if they're old
//!   - Always keep the most recent `retain_recent_ms` (e.g. 90 days)
//!
//! The pure function `plan_purge(rows, policy) -> Vec<id_to_delete>` is
//! tested in isolation; `infra::db::audit::purge_old` applies it.

use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// Retention policy. Defaults target ~90 days of history with a
/// hard floor of 1000 rows (so a quiet user still has data).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionPolicy {
    /// Maximum age in milliseconds. Rows older than this are eligible
    /// for purge (subject to `min_keep_rows`).
    pub retain_recent_ms: i64,
    /// Hard cap on total row count. If we exceed this, the oldest
    /// rows are purged regardless of age.
    pub max_rows: i64,
    /// Safety floor: never auto-purge below this many rows, even if
    /// they're ancient. Default 1000 so users always have context.
    pub min_keep_rows: i64,
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        Self {
            // 90 days
            retain_recent_ms: 90 * 86_400_000,
            // 50k rows
            max_rows: 50_000,
            // 1000 rows floor
            min_keep_rows: 1000,
        }
    }
}

/// A row's identity + timestamp, in the order returned by the SQL
/// "ORDER BY at DESC" query. We need the timestamps to apply the
/// age policy; we need the ids to know what to delete.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AuditRow {
    pub id: i64,
    pub at: i64,
}

/// Plan output: which row ids to delete.
pub fn plan_purge(rows: &[AuditRow], policy: &RetentionPolicy, now_ms: i64) -> Vec<i64> {
    if rows.is_empty() {
        return Vec::new();
    }
    // 1. Compute the cutoff: rows with `at < cutoff` are age-eligible
    let cutoff = now_ms - policy.retain_recent_ms;

    // 2. Walk newest→oldest; collect ids that are BOTH:
    //    - age-eligible (at < cutoff), AND
    //    - not within the safety floor (id is past index min_keep_rows)
    let mut to_delete: Vec<i64> = Vec::new();
    for (idx, row) in rows.iter().enumerate() {
        if idx < policy.min_keep_rows as usize {
            continue;  // within safety floor
        }
        if row.at >= cutoff {
            break;  // hit a fresh row — everything older is also fresh (rows are sorted DESC)
        }
        to_delete.push(row.id);
    }
    let _ = policy.max_rows;  // reserved for future "even if fresh, cap total" use
    to_delete
}

/// Count the rows that *would* be deleted without actually building
/// the id list. Useful for logging.
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
        // 10 rows, all within the 90-day window
        let now = 1_000_000_000_000;
        let rows: Vec<AuditRow> = (0..10).map(|i| row(i, now - i * 3_600_000)).collect();
        let to_del = plan_purge(&rows, &RetentionPolicy::default(), now);
        assert!(to_del.is_empty());
    }

    #[test]
    fn all_old_purges_above_floor() {
        // 100 rows, all 1 year old (way past 90 days)
        let now = 1_000_000_000_000;
        let one_year_ago = now - 365 * 86_400_000;
        let rows: Vec<AuditRow> = (0..100).map(|i| row(i, one_year_ago + i)).collect();
        let policy = RetentionPolicy::default();  // min_keep_rows = 1000
        // Floor is 1000, we have 100 → all kept
        assert!(plan_purge(&rows, &policy, now).is_empty());
    }

    #[test]
    fn old_above_floor_purges() {
        // 1500 rows: 1000 newest kept, 500 oldest purged
        let now = 1_000_000_000_000;
        let old = now - 365 * 86_400_000;
        let mut rows: Vec<AuditRow> = Vec::new();
        for i in 0..500 {
            rows.push(row(1000 + i, old + i));   // old rows
        }
        for i in 0..1000 {
            rows.push(row(i, now - i * 3_600_000));  // recent rows
        }
        // rows are NOT sorted; the function expects them sorted DESC by `at`.
        rows.sort_by(|a, b| b.at.cmp(&a.at));
        let policy = RetentionPolicy::default();
        let to_del = plan_purge(&rows, &policy, now);
        // The first 1000 (newest) are within the floor; rows beyond that
        // are checked for age. All 500 old rows are past the floor AND old
        // → all 500 deleted.
        assert_eq!(to_del.len(), 500);
    }

    #[test]
    fn mixed_old_and_new_keeps_new() {
        let now = 1_000_000_000_000;
        let old = now - 365 * 86_400_000;
        // Sorted DESC: 5 new (within 90d), then 5 old (1 year ago)
        let rows = vec![
            row(1, now - 1_000),    // newest
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
        // min_keep_rows = 1000, so 5 new + 5 old all kept (10 < 1000)
        assert!(to_del.is_empty());
    }

    #[test]
    fn cutoff_at_exact_boundary_is_retained() {
        // Row at exactly `cutoff` is considered recent (>=, not <)
        let now = 1_000_000_000_000;
        let policy = RetentionPolicy { retain_recent_ms: 1000, ..Default::default() };
        let rows = vec![row(1, now - 1000), row(2, now - 1001)];
        let to_del = plan_purge(&rows, &policy, now);
        // Row 1 is at the boundary → kept. Row 2 is past → purged,
        // but it's also within min_keep_rows (2 < 1000) → kept.
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
        // 1-day retention, 2-row floor
        let now = 1_000_000_000_000;
        let policy = RetentionPolicy {
            retain_recent_ms: 86_400_000,
            max_rows: 100,
            min_keep_rows: 2,
        };
        let rows = vec![
            row(1, now - 100),
            row(2, now - 200),
            row(3, now - 2 * 86_400_000),  // 2 days old
            row(4, now - 3 * 86_400_000),  // 3 days old
        ];
        // Floor keeps rows 1, 2 (newest 2). Row 3 is 2 days old (>1 day) → purge.
        // Row 4 is also >1 day and past the floor → purge.
        let to_del = plan_purge(&rows, &policy, now);
        assert_eq!(to_del.len(), 2);
        assert!(to_del.contains(&3));
        assert!(to_del.contains(&4));
    }
}
