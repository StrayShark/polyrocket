//! L3 — Sidecar health snapshot (pure).
//!
//! Tracks a rolling "last 20 probes" history so the L1 UI can show
//! a small sparkline of uptime. Newer = better, fewer errors = better.
//!
//! Pure functions only — the L4 module does the actual IPC ping.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SidecarHealthKind {
    /// Probe returned `pong` in time.
    Ok,
    /// Probe returned an error (timeout, parse failure, non-200).
    Failed,
    /// No probe has been recorded yet.
    Unknown,
}

impl SidecarHealthKind {
    pub fn as_str(self) -> &'static str {
        match self {
            SidecarHealthKind::Ok => "ok",
            SidecarHealthKind::Failed => "failed",
            SidecarHealthKind::Unknown => "unknown",
        }
    }
    pub fn parse(s: &str) -> Self {
        match s {
            "ok" => SidecarHealthKind::Ok,
            "failed" => SidecarHealthKind::Failed,
            _ => SidecarHealthKind::Unknown,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SidecarHealthRow {
    pub at_ms: i64,
    pub kind: SidecarHealthKind,
    pub latency_ms: Option<i64>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SidecarHealthSnapshot {
    pub last_24h: Vec<SidecarHealthRow>,  // most-recent first
    pub success_count: i64,
    pub failure_count: i64,
    pub last_success_at_ms: Option<i64>,
    pub last_failure_at_ms: Option<i64>,
}

impl SidecarHealthSnapshot {
    pub fn from_rows(rows: &[SidecarHealthRow]) -> Self {
        let mut success = 0i64;
        let mut failure = 0i64;
        let mut last_ok: Option<i64> = None;
        let mut last_fail: Option<i64> = None;
        for r in rows {
            match r.kind {
                SidecarHealthKind::Ok => {
                    success += 1;
                    if last_ok.is_none() || r.at_ms > last_ok.unwrap() {
                        last_ok = Some(r.at_ms);
                    }
                }
                SidecarHealthKind::Failed => {
                    failure += 1;
                    if last_fail.is_none() || r.at_ms > last_fail.unwrap() {
                        last_fail = Some(r.at_ms);
                    }
                }
                SidecarHealthKind::Unknown => {}
            }
        }
        Self {
            last_24h: rows.to_vec(),
            success_count: success,
            failure_count: failure,
            last_success_at_ms: last_ok,
            last_failure_at_ms: last_fail,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn as_str_round_trip() {
        for k in [SidecarHealthKind::Ok, SidecarHealthKind::Failed, SidecarHealthKind::Unknown] {
            assert_eq!(SidecarHealthKind::parse(k.as_str()), k);
        }
    }

    #[test]
    fn empty_snapshot_is_all_zeros() {
        let snap = SidecarHealthSnapshot::from_rows(&[]);
        assert_eq!(snap.success_count, 0);
        assert_eq!(snap.failure_count, 0);
        assert!(snap.last_success_at_ms.is_none());
        assert!(snap.last_failure_at_ms.is_none());
    }

    #[test]
    fn snapshot_counts_ok_and_failed() {
        let rows = vec![
            SidecarHealthRow { at_ms: 100, kind: SidecarHealthKind::Ok, latency_ms: Some(5), error: None },
            SidecarHealthRow { at_ms: 200, kind: SidecarHealthKind::Failed, latency_ms: None, error: Some("timeout".into()) },
            SidecarHealthRow { at_ms: 300, kind: SidecarHealthKind::Ok, latency_ms: Some(3), error: None },
        ];
        let snap = SidecarHealthSnapshot::from_rows(&rows);
        assert_eq!(snap.success_count, 2);
        assert_eq!(snap.failure_count, 1);
        assert_eq!(snap.last_success_at_ms, Some(300));
        assert_eq!(snap.last_failure_at_ms, Some(200));
    }

    #[test]
    fn parse_handles_unknown_string() {
        assert_eq!(SidecarHealthKind::parse("nonsense"), SidecarHealthKind::Unknown);
    }
}
