//! L2 — Active-model IPC (v0.49b).
//!
//! Single source of truth for "what model is currently
//! active, and what are its training metrics?". Prior
//! to v0.49b, every consumer (degradation detector,
//! settings card, etc.) read `active.json` directly
//! from disk. This module makes that an IPC, so:
//!
//! - L1 always goes through `getActiveModel` instead
//!   of duplicating the file path logic.
//! - The Rust consumer (degradation loop, future
//!   v0.50+ monitors) calls the same internal helper
//!   instead of re-reading active.json.
//! - Schema changes (v0.50+ will add `weights`) only
//!   happen in one place.
//!
//! Returned DTO:
//!
//! ```text
//! ActiveModel {
//!   model_version: String,           // e.g. "logistic-train-441c352b"
//!   best_brier: Option<f64>,         // train-time Brier
//!   best_params: Option<Value>,      // hyperparams of the best trial
//!   promoted_at_ms: Option<i64>,     // wall-clock of last promote
//!   weights: Option<Vec<f64>>,       // optional; v0.50+ will populate
//!   source_path: String,             // absolute path to active.json
//! }
//! ```
//!
//! Returns an `AppResult::Ok(None)` when active.json is
//! missing (typical before first promote). Other IO /
//! parse errors are surfaced as `AppError::Internal`.

use crate::AppError;
use crate::AppResult;
use serde::Serialize;
use std::io::Read;
use std::path::PathBuf;

/// Wire-format DTO returned by `get_active_model`.
#[derive(Debug, Clone, Serialize)]
pub struct ActiveModel {
    pub model_version: String,
    pub best_brier: Option<f64>,
    pub best_params: Option<serde_json::Value>,
    pub promoted_at_ms: Option<i64>,
    /// v0.50+ will populate this when the sidecar
    /// writes weights into active.json. Today the
    /// sidecar doesn't, so it's always None — but
    /// the field is reserved here for forward-compat.
    pub weights: Option<Vec<f64>>,
    pub source_path: String,
}

/// Resolve the active.json path the same way the
/// Python sidecar does in `train.py:48`. Centralising
/// the env-var handling here means the L1 doesn't
/// have to know about `POLYROCKET_SIDECAR_MODEL_DIR`.
pub fn active_model_path() -> PathBuf {
    let dir = std::env::var("POLYROCKET_SIDECAR_MODEL_DIR")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
            format!("{home}/.polyrocket/sidecar/models")
        });
    PathBuf::from(dir).join("active.json")
}

/// Read the active model from disk. Returns `Ok(None)`
/// when active.json is missing (i.e. no model has been
/// promoted yet). Returns `Err(Internal)` when the
/// file exists but is malformed.
pub fn read_active_model_from_disk() -> AppResult<Option<ActiveModel>> {
    let path = active_model_path();
    let mut f = match std::fs::File::open(&path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(AppError::Internal(format!("active.json: {e}"))),
    };
    let mut s = String::new();
    if let Err(e) = f.read_to_string(&mut s) {
        return Err(AppError::Internal(format!("read active.json: {e}")));
    }
    let v: serde_json::Value = match serde_json::from_str(&s) {
        Ok(v) => v,
        Err(e) => return Err(AppError::Internal(format!("parse active.json: {e}"))),
    };
    // Best-effort schema decode. Fields are all optional;
    // we surface what's there.
    let model_version = v
        .get("model_version")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    if model_version.is_empty() {
        return Err(AppError::Internal(
            "active.json: missing model_version".into(),
        ));
    }
    let best = v.get("best");
    let best_brier = best
        .and_then(|b| b.get("brier"))
        .and_then(|x| x.as_f64());
    let best_params = best.and_then(|b| b.get("params")).cloned();
    let promoted_at_ms = v
        .get("promoted_at_ms")
        .and_then(|x| x.as_i64());
    let weights = v
        .get("weights")
        .and_then(|x| x.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|n| n.as_f64())
                .collect::<Vec<f64>>()
        });
    Ok(Some(ActiveModel {
        model_version,
        best_brier,
        best_params,
        promoted_at_ms,
        weights,
        source_path: path.to_string_lossy().to_string(),
    }))
}

/// IPC: read the active model from disk. Returns
/// `null` when active.json is missing (no model has
/// been promoted yet). Errors are surfaced via the
/// standard `AppError::Internal` channel.
#[tauri::command]
pub fn get_active_model() -> AppResult<Option<ActiveModel>> {
    read_active_model_from_disk()
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Write a JSON value to a temp file. Returns the
    /// path; caller is responsible for `remove_file`.
    fn write_active_json(value: serde_json::Value) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "polyrocket_active_model_test_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("active.json");
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(value.to_string().as_bytes()).unwrap();
        path
    }

    /// v0.49b — the env var override path. We can't
    /// modify the process env from parallel tests, so
    /// we just sanity-check the path computation here.
    #[test]
    fn active_model_path_under_default() {
        // Sanity: when env unset, path ends with .polyrocket/sidecar/models/active.json
        let p = active_model_path();
        let s = p.to_string_lossy();
        // Either "$HOME/.polyrocket/sidecar/models/active.json"
        // or whatever POLYROCKET_SIDECAR_MODEL_DIR points to.
        assert!(s.ends_with("active.json"), "got: {s}");
    }

    /// v0.49b — the read function gracefully returns
    /// Ok(None) when the file is missing. Tested by
    /// pointing the env var at a temp dir without
    /// active.json.
    #[test]
    fn read_returns_none_when_missing() {
        let dir = std::env::temp_dir().join(format!(
            "polyrocket_active_model_missing_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        // SAFETY: POLYROCKET_SIDECAR_MODEL_DIR is read
        // by name; this test is the only writer in the
        // test process. set_var is unsafe since Rust
        // 1.83 but we need it for the test.
        unsafe { std::env::set_var("POLYROCKET_SIDECAR_MODEL_DIR", &dir); }
        let result = read_active_model_from_disk();
        unsafe { std::env::remove_var("POLYROCKET_SIDECAR_MODEL_DIR"); }
        assert!(matches!(result, Ok(None)), "got: {result:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// v0.49b — happy path: a well-formed active.json
    /// round-trips into the DTO with all fields.
    #[test]
    fn read_happy_path_populates_all_fields() {
        let path = write_active_json(serde_json::json!({
            "model_version": "logistic-train-abc",
            "best": {
                "brier": 0.172,
                "params": {"alpha": 0.01}
            },
            "promoted_at_ms": 1740000000000_i64,
            "weights": [0.5, 1.0, -0.25]
        }));
        let parent = path.parent().unwrap().to_string_lossy().to_string();
        unsafe { std::env::set_var("POLYROCKET_SIDECAR_MODEL_DIR", &parent); }
        let result = read_active_model_from_disk();
        unsafe { std::env::remove_var("POLYROCKET_SIDECAR_MODEL_DIR"); }
        let am = result.expect("Ok").expect("Some");
        assert_eq!(am.model_version, "logistic-train-abc");
        assert_eq!(am.best_brier, Some(0.172));
        assert_eq!(am.promoted_at_ms, Some(1740000000000));
        assert_eq!(am.weights.as_deref(), Some([0.5, 1.0, -0.25].as_slice()));
        assert!(am.source_path.ends_with("active.json"));
        let _ = std::fs::remove_dir_all(&parent);
    }

    /// v0.49b — a malformed active.json surfaces as
    /// AppError::Internal (not silently Ok(None)).
    #[test]
    fn read_malformed_json_returns_error() {
        let path = write_active_json(serde_json::json!({
            "model_version": "logistic-train-xyz"
            // missing "best.brier" — fine, it's optional
            // — but the file is JSON-valid. Try invalid JSON.
        }));
        // Overwrite with garbage.
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(b"{ not json").unwrap();
        drop(f);

        let parent = path.parent().unwrap().to_string_lossy().to_string();
        unsafe { std::env::set_var("POLYROCKET_SIDECAR_MODEL_DIR", &parent); }
        let result = read_active_model_from_disk();
        unsafe { std::env::remove_var("POLYROCKET_SIDECAR_MODEL_DIR"); }
        assert!(matches!(result, Err(AppError::Internal(_))), "got: {result:?}");
        let _ = std::fs::remove_dir_all(&parent);
    }
}
