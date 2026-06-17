//! L3 — Python sidecar protocol (M7).
//!
//! Defines the JSON-line protocol that polyrocket uses to talk to
//! the optional Python ML sidecar (a separate process spawned via
//! `std::process::Command`). All pure helpers — no IO here.
//!
//! Spec: docs/polyrocket-modules.md §2.M7
//!
//! Protocol (one JSON object per line, both directions):
//!
//! request:  { "id": "uuid", "method": "name", "params": {...} }
//! response: { "id": "uuid", "ok": true, "result": {...} }
//!            | { "id": "uuid", "ok": false, "error": "msg" }
//!
//! Methods (polyrocket → python):
//!   - "ping"            → { "pong": true }
//!   - "predict"         → { "predictions": [{ market_id, prob, confidence }] }
//!   - "train_job"       → { "job_id": "..." }
//!   - "promote_model"   → { "promoted": true }
//!
//! Methods (python → polyrocket, for callbacks):
//!   - "log"             → append to sidecar log
//!   - "metric"          → live metric update

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// One JSON-line request from polyrocket → python.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SidecarRequest {
    pub id: String,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

/// One JSON-line response from python → polyrocket.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SidecarResponse {
    pub id: String,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl SidecarResponse {
    pub fn ok(id: impl Into<String>, result: Value) -> Self {
        Self { id: id.into(), ok: true, result: Some(result), error: None }
    }
    pub fn err(id: impl Into<String>, msg: impl Into<String>) -> Self {
        Self { id: id.into(), ok: false, result: None, error: Some(msg.into()) }
    }
}

/// One prediction row returned by the `predict` method.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Prediction {
    pub market_id: String,
    /// 0..1
    pub prob: f64,
    /// 0..1
    pub confidence: f64,
    pub rationale: Option<String>,
}

/// v0.12a — full predict response (the result block + each row).
/// The `predictions` field mirrors the per-row data; `model_version`
/// is hoisted to the top level so the L1 ModelLab page can show
/// "scoring with logistic-train-..." without iterating rows.
/// v0.13b — also carries `brier_score` from the active model so
/// the L1 ModelVersionPill can show calibration in the tooltip.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PredictResult {
    pub predictions: Vec<Prediction>,
    /// `logistic-0.1.0` (inline fallback) or
    /// `logistic-train-441c352b` (active promoted model).
    /// `None` if the sidecar didn't include it (back-compat).
    pub model_version: Option<String>,
    /// Brier score from the most recent train run (the score of
    /// the "best" trial that was promoted to active.json). `None`
    /// if no model has been promoted (using inline fallback) or
    /// the sidecar didn't include it.
    pub brier_score: Option<f64>,
}

/// Methods enum for type-safe dispatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SidecarMethod {
    Ping,
    Predict,
    TrainJob,
    PromoteModel,
    /// v0.19a — read-only audit of past promotions. Returns
    /// the `promotion_history` array from active.json (capped
    /// at 20 most-recent entries).
    ListPromoteHistory,
    /// v0.20a — roll the active model back to a previous
    /// version. Looks up the entry in promotion_history by
    /// `model_version` and restores its `weights` as the
    /// new active model. The entry must include weights
    /// (v0.20a+); older v0.19 entries without weights are
    /// refused with a clear error.
    RollbackModel,
    /// v0.23a — auto-promote guard. Promotes the candidate
    /// only if it's meaningfully better than the active
    /// model (Brier margin). If not, no-op + clear "skipped"
    /// reason. One-click action triggered by the user.
    AutoPromoteIfBetter,
}

impl SidecarMethod {
    pub fn as_str(self) -> &'static str {
        match self {
            SidecarMethod::Ping => "ping",
            SidecarMethod::Predict => "predict",
            SidecarMethod::TrainJob => "train_job",
            SidecarMethod::PromoteModel => "promote_model",
            SidecarMethod::ListPromoteHistory => "list_promote_history",
            SidecarMethod::RollbackModel => "rollback_model",
            SidecarMethod::AutoPromoteIfBetter => "auto_promote_if_better",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "ping" => Some(SidecarMethod::Ping),
            "predict" => Some(SidecarMethod::Predict),
            "train_job" => Some(SidecarMethod::TrainJob),
            "promote_model" => Some(SidecarMethod::PromoteModel),
            "list_promote_history" => Some(SidecarMethod::ListPromoteHistory),
            "rollback_model" => Some(SidecarMethod::RollbackModel),
            "auto_promote_if_better" => Some(SidecarMethod::AutoPromoteIfBetter),
            _ => None,
        }
    }
}

/// Try to parse one JSON line as either a request or response.
/// Returns Ok(parsed) or Err with the raw line for inspection.
pub fn parse_line(line: &str) -> Result<ParseResult, String> {
    let v: Value = serde_json::from_str(line.trim())
        .map_err(|e| format!("invalid JSON: {e}"))?;
    // Response has `ok`; request has `method`.
    if v.get("ok").is_some() {
        let r: SidecarResponse = serde_json::from_value(v)
            .map_err(|e| format!("response parse: {e}"))?;
        Ok(ParseResult::Response(r))
    } else if v.get("method").is_some() {
        let r: SidecarRequest = serde_json::from_value(v)
            .map_err(|e| format!("request parse: {e}"))?;
        Ok(ParseResult::Request(r))
    } else {
        Err("line is neither a request nor a response".into())
    }
}

pub enum ParseResult {
    Request(SidecarRequest),
    Response(SidecarResponse),
}

impl std::fmt::Debug for ParseResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseResult::Request(r) => f.debug_tuple("Request").field(r).finish(),
            ParseResult::Response(r) => f.debug_tuple("Response").field(r).finish(),
        }
    }
}

/// Build a `predict` request from market ids + market context.
/// Pure function: serializes to JSON-line string.
pub fn build_predict_request(
    id: impl Into<String>,
    markets: &[(String, f64)], // (market_id, current_price)
) -> String {
    let params = serde_json::json!({
        "markets": markets.iter().map(|(id, price)| {
            serde_json::json!({ "market_id": id, "price": price })
        }).collect::<Vec<_>>()
    });
    let req = SidecarRequest {
        id: id.into(),
        method: SidecarMethod::Predict.as_str().to_string(),
        params,
    };
    serde_json::to_string(&req).unwrap_or_default()
}

/// Parse a `predict` response into a `PredictResult`.
/// Returns Err if the response is `ok=false`.
///
/// v0.12a — now returns the full `PredictResult` (predictions +
/// model_version) instead of just `Vec<Prediction>`. The model
/// version is hoisted from the response's top-level `model_version`
/// field, falling back to `None` if the sidecar didn't send it
/// (back-compat with v0.11c and earlier).
pub fn parse_predict_response(
    resp: &SidecarResponse,
) -> Result<PredictResult, String> {
    if !resp.ok {
        return Err(resp.error.clone().unwrap_or_else(|| "unknown error".into()));
    }
    let v = resp.result.clone().unwrap_or(Value::Null);
    let arr = v.get("predictions")
        .and_then(|x| x.as_array())
        .ok_or_else(|| "missing 'predictions' array".to_string())?;
    let mut out = Vec::with_capacity(arr.len());
    for item in arr {
        let p: Prediction = serde_json::from_value(item.clone())
            .map_err(|e| format!("prediction row: {e}"))?;
        out.push(p);
    }
    let model_version = v.get("model_version")
        .and_then(|x| x.as_str())
        .map(String::from);
    let brier_score = v.get("brier_score").and_then(|x| x.as_f64());
    Ok(PredictResult { predictions: out, model_version, brier_score })
}

// =================================================================
// ============== v0.17a — train_job wire format ====================
// =================================================================

/// Build a `train_job` request. Pure function: serializes to a
/// JSON-line string. The Python sidecar accepts these params
/// (see `sidecar/polyrocket_sidecar/dispatch.py::train_job`):
///
///   - n_trials: int (default 4, max 4)
///   - epochs:   int (default 80)
///   - job_id:   str (server-generated; client-side is ignored)
///
/// The Rust side generates a fresh `job_id` and passes it back
/// in the started event so the L1 can correlate.
pub fn build_train_request(
    id: impl Into<String>,
    n_trials: Option<u32>,
    epochs: Option<u32>,
) -> String {
    let mut params = serde_json::Map::new();
    if let Some(n) = n_trials {
        params.insert("n_trials".into(), serde_json::json!(n));
    }
    if let Some(e) = epochs {
        params.insert("epochs".into(), serde_json::json!(e));
    }
    let req = SidecarRequest {
        id: id.into(),
        method: SidecarMethod::TrainJob.as_str().to_string(),
        params: serde_json::Value::Object(params),
    };
    serde_json::to_string(&req).unwrap_or_default()
}

/// One trial's result in a `train_job` response. v0.17a
/// mirrors the Python sidecar's `trials[]` array.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainTrial {
    pub lr: f64,
    pub reg: f64,
    pub brier: f64,
    /// `{"w0": ..., "w1": ..., "w2": ...}` — the trained weights
    /// for this trial. Maps to the Python `weights` dict.
    pub weights: serde_json::Value,
}

/// Wire-format mirror of the Python `run_train_job` return value.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainResult {
    /// Server-generated job id (e.g. `"train-441c352b"`).
    pub job_id: String,
    /// "completed" | "failed" (mirrors the Python return value).
    pub status: String,
    /// Best trial's Brier score (lower is better). Null on failure.
    pub best_brier: Option<f64>,
    /// Best trial's weights: `{"w0", "w1", "w2"}`. Null on failure.
    pub best_params: Option<serde_json::Value>,
    /// Per-trial stats. Length 0 if the train failed before any
    /// trial finished.
    pub trials: Vec<TrainTrial>,
    /// Wall-clock time in milliseconds (Python's `duration_ms`).
    pub duration_ms: i64,
    /// Absolute path of the candidate JSON the Python sidecar
    /// wrote (e.g. `~/.polyrocket/sidecar/models/candidate.json`).
    /// Null on failure.
    pub candidate_path: Option<String>,
    /// Human-readable error message if `status == "failed"`.
    /// None on success.
    pub message: Option<String>,
}

/// Parse a `train_job` response into a `TrainResult`. Returns
/// Err if the response is `ok=false`.
///
/// The Python sidecar's `run_train_job` returns a dict with
/// the fields above; some are optional on failure paths
/// (e.g. `best_brier` may be null even on partial success).
pub fn parse_train_response(resp: &SidecarResponse) -> Result<TrainResult, String> {
    if !resp.ok {
        return Err(resp.error.clone().unwrap_or_else(|| "unknown error".into()));
    }
    let v = resp.result.clone().unwrap_or(Value::Null);
    Ok(TrainResult {
        job_id: v.get("job_id")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string(),
        status: v.get("status")
            .and_then(|x| x.as_str())
            .unwrap_or("unknown")
            .to_string(),
        best_brier: v.get("best_brier").and_then(|x| x.as_f64()),
        best_params: v.get("best_params").cloned(),
        trials: v.get("trials")
            .and_then(|x| x.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|item| serde_json::from_value::<TrainTrial>(item.clone()).ok())
                    .collect()
            })
            .unwrap_or_default(),
        duration_ms: v.get("duration_ms").and_then(|x| x.as_i64()).unwrap_or(0),
        candidate_path: v.get("candidate_path")
            .and_then(|x| x.as_str())
            .map(String::from),
        message: v.get("message")
            .and_then(|x| x.as_str())
            .map(String::from),
    })
}

// =================================================================
// ============== v0.18a — promote_model wire format ================
// =================================================================

/// Build a `promote_model` request. Pure function: serializes
/// to a JSON-line string. The Python sidecar accepts:
///
///   - job_id: str (optional; if set, refuses to promote a
///     candidate from a different job — protects against
///     race conditions where another train finishes between
///     the user's intent to promote and the promote call)
///
/// v0.18a — promote is a fast synchronous file move (~10ms).
/// No progress events. The IPC returns the full result.
/// v0.21a — `trial_index` is an optional bulk-promote param.
/// If `Some(n)`, promotes the n-th trial from `all_trials[]`
/// instead of the best. The model version gets a `-t{n}`
/// suffix so the user can distinguish bulk-promoted trials
/// in the history panel.
pub fn build_promote_request(
    id: impl Into<String>,
    job_id: Option<&str>,
    trial_index: Option<usize>,
) -> String {
    let mut params = serde_json::Map::new();
    if let Some(j) = job_id {
        params.insert("job_id".into(), serde_json::Value::String(j.to_string()));
    }
    if let Some(t) = trial_index {
        params.insert("trial_index".into(), serde_json::Value::from(t));
    }
    let req = SidecarRequest {
        id: id.into(),
        method: SidecarMethod::PromoteModel.as_str().to_string(),
        params: serde_json::Value::Object(params),
    };
    serde_json::to_string(&req).unwrap_or_default()
}

/// Wire-format mirror of the Python `run_promote_model`
/// return value. v0.18a — `promoted: bool` is the primary
/// success indicator; `status: "ok" | "failed"` is the
/// Python's own string for backward compat.
///
/// All fields are nullable because the Python sidecar
/// returns a partially-populated dict on failure paths
/// (e.g. when the candidate file is missing).
///
/// v0.21a — `trial_index: Option<usize>` records which
/// trial was promoted. `None` means the best (default);
/// `Some(n)` means the n-th trial of the train sweep
/// (bulk promote).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromoteResult {
    /// `true` on success, `false` on failure.
    pub promoted: bool,
    /// "ok" | "failed" — mirrors the Python's return string.
    pub status: String,
    /// Path of the previous active.json (or null on first promote).
    pub previous_path: Option<String>,
    /// Path of the new active.json (the candidate was renamed to this).
    pub active_path: Option<String>,
    /// Wall-clock time of the promote in milliseconds.
    pub promoted_at_ms: Option<i64>,
    /// New model version string (e.g. `logistic-train-441c352b`
    /// for the best, or `logistic-train-441c352b-t2` for bulk).
    /// Empty string on failure.
    pub model_version: String,
    /// Human-readable error message on failure; `None` on success.
    pub message: Option<String>,
    /// v0.21a — which trial was promoted. `None` = best;
    /// `Some(n)` = trial n of the train sweep.
    pub trial_index: Option<usize>,
}

/// Parse a `promote_model` response into a `PromoteResult`.
/// Returns Err if the response is `ok=false`.
pub fn parse_promote_response(resp: &SidecarResponse) -> Result<PromoteResult, String> {
    if !resp.ok {
        return Err(resp.error.clone().unwrap_or_else(|| "unknown error".into()));
    }
    let v = resp.result.clone().unwrap_or(Value::Null);
    Ok(PromoteResult {
        promoted: v.get("promoted").and_then(|x| x.as_bool()).unwrap_or(false),
        status: v.get("status")
            .and_then(|x| x.as_str())
            .unwrap_or("unknown")
            .to_string(),
        previous_path: v.get("previous_path")
            .and_then(|x| x.as_str())
            .map(String::from),
        active_path: v.get("active_path")
            .and_then(|x| x.as_str())
            .map(String::from),
        promoted_at_ms: v.get("promoted_at_ms").and_then(|x| x.as_i64()),
        model_version: v.get("model_version")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string(),
        message: v.get("message")
            .and_then(|x| x.as_str())
            .map(String::from),
        trial_index: v.get("trial_index").and_then(|x| x.as_u64()).map(|n| n as usize),
    })
}

// =================================================================
// ============ v0.19a — list_promote_history wire format ===========
// =================================================================

/// Build a `list_promote_history` request. Pure function.
/// v0.19a — read-only audit, no params. The request just
/// carries the id for correlation.
pub fn build_list_promote_history_request(id: impl Into<String>) -> String {
    let req = SidecarRequest {
        id: id.into(),
        method: SidecarMethod::ListPromoteHistory.as_str().to_string(),
        params: serde_json::json!({}),
    };
    serde_json::to_string(&req).unwrap_or_default()
}

/// Wire-format mirror of a single entry in
/// `active.json.promotion_history[]`. v0.19a — one entry per
/// successful promote. Oldest first, newest last (the last
/// entry is the one that was just superseded when a new
/// promote happened; if you want the currently active model
/// use `model_version` from `PredictResult` instead).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromoteHistoryEntry {
    /// Train job_id, e.g. "train-441c352b".
    pub job_id: String,
    /// Derived model version, e.g. "logistic-train-441c352b".
    pub model_version: String,
    /// Wall-clock time of the promote in milliseconds.
    pub promoted_at_ms: i64,
    /// Best Brier score from the train sweep (lower is better).
    pub best_brier: Option<f64>,
    /// Hyperparameters of the best trial.
    #[serde(default)]
    pub best_params: Option<Value>,
}

/// Wire-format mirror of the `list_promote_history` response.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PromoteHistoryResult {
    /// `true` on success, `false` if active.json is missing
    /// or malformed. (The Python sidecar returns ok=false
    /// only in pathological cases; missing file is `ok=true
    /// with count=0` because it's the natural "no history
    /// yet" state.)
    pub ok: bool,
    /// History entries, oldest first.
    pub entries: Vec<PromoteHistoryEntry>,
    /// `len(entries)` for convenience.
    pub count: usize,
    /// Human-readable error message on failure; `None` on success.
    pub message: Option<String>,
}

/// Parse a `list_promote_history` response into a
/// `PromoteHistoryResult`. Returns Err only if the sidecar
/// returned `ok=false` at the envelope level (transport
/// error). An ok=true envelope with ok=false at the result
/// level (file is malformed) returns Ok with the message.
pub fn parse_list_promote_history_response(
    resp: &SidecarResponse,
) -> Result<PromoteHistoryResult, String> {
    if !resp.ok {
        return Err(resp.error.clone().unwrap_or_else(|| "unknown error".into()));
    }
    let v = resp.result.clone().unwrap_or(Value::Null);
    let entries: Vec<PromoteHistoryEntry> = v
        .get("entries")
        .and_then(|x| x.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|item| serde_json::from_value::<PromoteHistoryEntry>(item.clone()).ok())
                .collect()
        })
        .unwrap_or_default();
    Ok(PromoteHistoryResult {
        ok: v.get("ok").and_then(|x| x.as_bool()).unwrap_or(false),
        count: v.get("count").and_then(|x| x.as_u64()).unwrap_or(entries.len() as u64) as usize,
        entries,
        message: v.get("message")
            .and_then(|x| x.as_str())
            .map(String::from),
    })
}

// =================================================================
// ============== v0.20a — rollback_model wire format ===============
// =================================================================

/// Build a `rollback_model` request. Pure function.
/// v0.20a — rolls the active model back to a previous
/// version (looked up by `model_version` in the
/// promotion_history).
pub fn build_rollback_request(
    id: impl Into<String>,
    model_version: &str,
) -> String {
    let req = SidecarRequest {
        id: id.into(),
        method: SidecarMethod::RollbackModel.as_str().to_string(),
        params: serde_json::json!({ "model_version": model_version }),
    };
    serde_json::to_string(&req).unwrap_or_default()
}

/// Wire-format mirror of the Python `run_rollback_model`
/// return value. v0.20a — `rolled_back: bool` is the
/// primary success indicator.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackResult {
    /// `true` on success, `false` on failure.
    pub rolled_back: bool,
    /// "ok" | "failed" — mirrors the Python's return string.
    pub status: String,
    /// Path of the previous active.json (always the same
    /// path; rollback is a write to the same file).
    pub previous_path: Option<String>,
    /// Path of the new active.json (same as previous_path).
    pub active_path: Option<String>,
    /// Wall-clock time of the rollback in milliseconds.
    pub rolled_back_at_ms: Option<i64>,
    /// The version that was rolled back to. Empty on failure.
    pub model_version: String,
    /// Human-readable error message on failure.
    pub message: Option<String>,
}

/// Parse a `rollback_model` response into a `RollbackResult`.
/// Returns Err if the response is `ok=false` at the
/// envelope level.
pub fn parse_rollback_response(resp: &SidecarResponse) -> Result<RollbackResult, String> {
    if !resp.ok {
        return Err(resp.error.clone().unwrap_or_else(|| "unknown error".into()));
    }
    let v = resp.result.clone().unwrap_or(Value::Null);
    Ok(RollbackResult {
        rolled_back: v.get("rolled_back").and_then(|x| x.as_bool()).unwrap_or(false),
        status: v.get("status")
            .and_then(|x| x.as_str())
            .unwrap_or("unknown")
            .to_string(),
        previous_path: v.get("previous_path")
            .and_then(|x| x.as_str())
            .map(String::from),
        active_path: v.get("active_path")
            .and_then(|x| x.as_str())
            .map(String::from),
        rolled_back_at_ms: v.get("rolled_back_at_ms").and_then(|x| x.as_i64()),
        model_version: v.get("model_version")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string(),
        message: v.get("message")
            .and_then(|x| x.as_str())
            .map(String::from),
    })
}

// =================================================================
// ========== v0.23a — auto_promote_if_better wire format ==========
// =================================================================

/// Build a `auto_promote_if_better` request. Pure function.
/// v0.23a — one-click "promote the candidate only if it's
/// meaningfully better than the active model" action.
///
/// `brier_margin` is how much better the candidate must be
/// (lower Brier = better). Default 0.005.
/// `trial_index` is which trial to use (None = best).
pub fn build_auto_promote_if_better_request(
    id: impl Into<String>,
    brier_margin: Option<f64>,
    trial_index: Option<usize>,
) -> String {
    let mut params = serde_json::Map::new();
    if let Some(m) = brier_margin {
        params.insert("brier_margin".into(), serde_json::json!(m));
    }
    if let Some(t) = trial_index {
        params.insert("trial_index".into(), serde_json::Value::from(t));
    }
    let req = SidecarRequest {
        id: id.into(),
        method: SidecarMethod::AutoPromoteIfBetter.as_str().to_string(),
        params: serde_json::Value::Object(params),
    };
    serde_json::to_string(&req).unwrap_or_default()
}

/// Wire-format mirror of the Python `run_auto_promote_if_better`
/// return value. v0.23a — the user clicks "Promote if better"
/// after each train; this DTO carries the result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoPromoteIfBetterResult {
    /// `true` if the candidate was actually promoted.
    pub promoted: bool,
    /// `true` if the candidate was NOT promoted (because
    /// it wasn't meaningfully better). Mutually exclusive
    /// with `promoted=true`.
    pub skipped: bool,
    /// Human-readable reason: "auto-promoted: improvement
    /// X > margin Y" / "candidate brier X is not at least
    /// Y better than active Z" / "no candidate" / etc.
    pub reason: String,
    /// The candidate's brier (or None if no candidate).
    pub candidate_brier: Option<f64>,
    /// The active model's brier (or None if no active).
    pub active_brier: Option<f64>,
    /// The brier_margin used for the comparison.
    pub margin: f64,
    /// The new model version (on success). Empty on skip/fail.
    pub model_version: Option<String>,
    /// The promote timestamp (on success). None on skip/fail.
    pub promoted_at_ms: Option<i64>,
    /// Human-readable message (e.g. the Python's error).
    pub message: Option<String>,
}

/// Parse a `auto_promote_if_better` response into an
/// `AutoPromoteIfBetterResult`. Returns Err if the
/// response is `ok=false` at the envelope level.
pub fn parse_auto_promote_if_better_response(
    resp: &SidecarResponse,
) -> Result<AutoPromoteIfBetterResult, String> {
    if !resp.ok {
        return Err(resp.error.clone().unwrap_or_else(|| "unknown error".into()));
    }
    let v = resp.result.clone().unwrap_or(Value::Null);
    Ok(AutoPromoteIfBetterResult {
        promoted: v.get("promoted").and_then(|x| x.as_bool()).unwrap_or(false),
        skipped: v.get("skipped").and_then(|x| x.as_bool()).unwrap_or(false),
        reason: v.get("reason")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string(),
        candidate_brier: v.get("candidate_brier").and_then(|x| x.as_f64()),
        active_brier: v.get("active_brier").and_then(|x| x.as_f64()),
        margin: v.get("margin").and_then(|x| x.as_f64()).unwrap_or(0.005),
        model_version: v.get("model_version")
            .and_then(|x| x.as_str())
            .map(String::from),
        promoted_at_ms: v.get("promoted_at_ms").and_then(|x| x.as_i64()),
        message: v.get("message")
            .and_then(|x| x.as_str())
            .map(String::from),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_round_trip() {
        for m in [SidecarMethod::Ping, SidecarMethod::Predict,
                  SidecarMethod::TrainJob, SidecarMethod::PromoteModel,
                  SidecarMethod::ListPromoteHistory,
                  SidecarMethod::RollbackModel,
                  SidecarMethod::AutoPromoteIfBetter] {
            assert_eq!(SidecarMethod::parse(m.as_str()), Some(m));
        }
        assert_eq!(SidecarMethod::parse("nope"), None);
    }

    #[test]
    fn parse_promote_response_success() {
        // v0.18a — full success case
        let r = SidecarResponse {
            id: "promote-abc".into(),
            ok: true,
            result: Some(serde_json::json!({
                "promoted": true,
                "status": "ok",
                "previous_path": "/home/x/.polyrocket/sidecar/models/active.json",
                "active_path": "/home/x/.polyrocket/sidecar/models/active.json",
                "promoted_at_ms": 1_700_000_000_000_i64,
                "model_version": "logistic-train-441c352b",
            })),
            error: None,
        };
        let p = parse_promote_response(&r).expect("ok");
        assert!(p.promoted);
        assert_eq!(p.status, "ok");
        assert_eq!(p.model_version, "logistic-train-441c352b");
        assert_eq!(p.previous_path.as_deref(), Some("/home/x/.polyrocket/sidecar/models/active.json"));
        assert_eq!(p.promoted_at_ms, Some(1_700_000_000_000));
        assert!(p.message.is_none());
    }

    #[test]
    fn parse_promote_response_no_candidate() {
        // v0.18a — the user clicked Promote before Train.
        // The Python sidecar returns promoted: false with a
        // descriptive message. The Rust side returns the
        // same shape so the L1 doesn't need a special path.
        let r = SidecarResponse {
            id: "promote-xyz".into(),
            ok: true,
            result: Some(serde_json::json!({
                "promoted": false,
                "status": "failed",
                "message": "no candidate found at /home/x/.polyrocket/sidecar/models/candidate.json; run train_job first",
            })),
            error: None,
        };
        let p = parse_promote_response(&r).expect("ok");
        assert!(!p.promoted);
        assert_eq!(p.status, "failed");
        assert!(p.message.unwrap().contains("run train_job first"));
        assert_eq!(p.model_version, "");
        assert!(p.previous_path.is_none());
        assert!(p.active_path.is_none());
    }

    #[test]
    fn parse_promote_response_job_id_mismatch() {
        // v0.18a — the user passed job_id="X" but the
        // current candidate is from job_id="Y". Refused.
        let r = SidecarResponse {
            id: "promote-mmm".into(),
            ok: true,
            result: Some(serde_json::json!({
                "promoted": false,
                "status": "failed",
                "message": "candidate job_id mismatch: expected X, got Y",
            })),
            error: None,
        };
        let p = parse_promote_response(&r).expect("ok");
        assert!(!p.promoted);
        assert!(p.message.unwrap().contains("mismatch"));
    }

    #[test]
    fn parse_promote_response_not_ok_returns_err() {
        // v0.18a — the Python sidecar returned ok=false.
        // We propagate the error message.
        let r = SidecarResponse {
            id: "promote-eee".into(),
            ok: false,
            result: None,
            error: Some("internal: oops".into()),
        };
        assert!(parse_promote_response(&r).is_err());
    }

    #[test]
    fn build_promote_request_no_job_id() {
        // v0.18a — optional job_id is omitted
        let line = build_promote_request("promote-123", None, None);
        let v: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(v["method"], "promote_model");
        assert_eq!(v["id"], "promote-123");
        assert!(v["params"].as_object().unwrap().is_empty());
    }

    #[test]
    fn build_promote_request_with_job_id() {
        // v0.18a — job_id is included
        let line = build_promote_request("promote-456", Some("train-abc"), None);
        let v: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(v["params"]["job_id"], "train-abc");
    }

    #[test]
    fn build_promote_request_with_trial_index() {
        // v0.21a — bulk promote: trial_index in params
        let line = build_promote_request("promote-789", Some("train-abc"), Some(2));
        let v: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(v["params"]["job_id"], "train-abc");
        assert_eq!(v["params"]["trial_index"], 2);
    }

    #[test]
    fn parse_promote_response_with_trial_index() {
        // v0.21a — bulk promote: response carries trial_index
        let r = SidecarResponse {
            id: "promote-bulk".into(),
            ok: true,
            result: Some(serde_json::json!({
                "promoted": true,
                "status": "ok",
                "previous_path": "/home/x/.polyrocket/sidecar/models/active.json",
                "active_path": "/home/x/.polyrocket/sidecar/models/active.json",
                "promoted_at_ms": 1_700_010_000_000_i64,
                "model_version": "logistic-train-441c352b-t2",
                "trial_index": 2,
            })),
            error: None,
        };
        let p = parse_promote_response(&r).expect("ok");
        assert!(p.promoted);
        assert_eq!(p.model_version, "logistic-train-441c352b-t2");
        assert_eq!(p.trial_index, Some(2));
    }

    #[test]
    fn parse_promote_response_default_trial_index() {
        // v0.21a — backward-compat: when trial_index is missing
        // (Python sidecar didn't return it), the Rust side
        // returns None (best, not bulk).
        let r = SidecarResponse {
            id: "promote-best".into(),
            ok: true,
            result: Some(serde_json::json!({
                "promoted": true,
                "status": "ok",
                "previous_path": "/home/x/.polyrocket/sidecar/models/active.json",
                "active_path": "/home/x/.polyrocket/sidecar/models/active.json",
                "promoted_at_ms": 1_700_011_000_000_i64,
                "model_version": "logistic-train-441c352b",
            })),
            error: None,
        };
        let p = parse_promote_response(&r).expect("ok");
        assert!(p.promoted);
        assert_eq!(p.model_version, "logistic-train-441c352b");
        assert!(p.trial_index.is_none());
    }

    #[test]
    fn build_list_promote_history_request_basic() {
        // v0.19a — read-only audit, no params (empty object)
        let line = build_list_promote_history_request("list-1");
        let v: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(v["method"], "list_promote_history");
        assert_eq!(v["id"], "list-1");
        assert_eq!(v["params"], serde_json::json!({}));
    }

    #[test]
    fn parse_list_promote_history_response_populated() {
        // v0.19a — 2 entries, ok=true
        let r = SidecarResponse {
            id: "list-2".into(),
            ok: true,
            result: Some(serde_json::json!({
                "ok": true,
                "count": 2,
                "entries": [
                    {
                        "job_id": "train-aaa",
                        "model_version": "logistic-train-aaa",
                        "promoted_at_ms": 1_700_000_000_000_i64,
                        "best_brier": 0.184,
                        "best_params": {"lr": 0.01, "reg": 0.1}
                    },
                    {
                        "job_id": "train-bbb",
                        "model_version": "logistic-train-bbb",
                        "promoted_at_ms": 1_700_001_000_000_i64,
                        "best_brier": 0.179,
                        "best_params": null
                    }
                ]
            })),
            error: None,
        };
        let h = parse_list_promote_history_response(&r).expect("ok");
        assert!(h.ok);
        assert_eq!(h.count, 2);
        assert_eq!(h.entries.len(), 2);
        assert_eq!(h.entries[0].job_id, "train-aaa");
        assert_eq!(h.entries[0].model_version, "logistic-train-aaa");
        assert_eq!(h.entries[0].best_brier, Some(0.184));
        assert!(h.entries[0].best_params.is_some());
        assert_eq!(h.entries[1].job_id, "train-bbb");
        assert!(h.entries[1].best_params.is_none());
        assert!(h.message.is_none());
    }

    #[test]
    fn parse_list_promote_history_response_empty() {
        // v0.19a — no active model yet; ok=true with empty entries
        let r = SidecarResponse {
            id: "list-3".into(),
            ok: true,
            result: Some(serde_json::json!({
                "ok": true,
                "count": 0,
                "entries": [],
                "message": "no active model yet; train + promote to start history"
            })),
            error: None,
        };
        let h = parse_list_promote_history_response(&r).expect("ok");
        assert!(h.ok);
        assert_eq!(h.count, 0);
        assert!(h.entries.is_empty());
        assert!(h.message.is_some());
    }

    #[test]
    fn parse_list_promote_history_response_not_ok_returns_err() {
        // v0.19a — envelope-level error
        let r = SidecarResponse {
            id: "list-4".into(),
            ok: false,
            result: None,
            error: Some("internal: oops".into()),
        };
        assert!(parse_list_promote_history_response(&r).is_err());
    }

    #[test]
    fn build_rollback_request_basic() {
        // v0.20a — model_version in params
        let line = build_rollback_request("rollback-1", "logistic-train-441c352b");
        let v: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(v["method"], "rollback_model");
        assert_eq!(v["id"], "rollback-1");
        assert_eq!(v["params"]["model_version"], "logistic-train-441c352b");
    }

    #[test]
    fn parse_rollback_response_success() {
        // v0.20a — full success case
        let r = SidecarResponse {
            id: "rollback-2".into(),
            ok: true,
            result: Some(serde_json::json!({
                "rolled_back": true,
                "status": "ok",
                "previous_path": "/home/x/.polyrocket/sidecar/models/active.json",
                "active_path": "/home/x/.polyrocket/sidecar/models/active.json",
                "rolled_back_at_ms": 1_700_005_000_000_i64,
                "model_version": "logistic-train-441c352b",
            })),
            error: None,
        };
        let rb = parse_rollback_response(&r).expect("ok");
        assert!(rb.rolled_back);
        assert_eq!(rb.status, "ok");
        assert_eq!(rb.model_version, "logistic-train-441c352b");
        assert_eq!(rb.rolled_back_at_ms, Some(1_700_005_000_000));
        assert!(rb.message.is_none());
    }

    #[test]
    fn parse_rollback_response_not_found() {
        // v0.20a — the requested model_version isn't in the
        // history (e.g. user passed a typo). The Python
        // sidecar returns rolled_back=false with a clear
        // diagnostic. The Rust side returns the same shape.
        let r = SidecarResponse {
            id: "rollback-3".into(),
            ok: true,
            result: Some(serde_json::json!({
                "rolled_back": false,
                "status": "failed",
                "message": "model_version 'logistic-train-XYZ' not found in promotion history",
            })),
            error: None,
        };
        let rb = parse_rollback_response(&r).expect("ok");
        assert!(!rb.rolled_back);
        assert_eq!(rb.status, "failed");
        assert!(rb.message.unwrap().contains("not found"));
        assert_eq!(rb.model_version, "");
    }

    #[test]
    fn parse_rollback_response_no_weights() {
        // v0.20a — entry exists but was promoted before
        // v0.20a (no weights stored). The user needs to
        // retrain to roll back to it.
        let r = SidecarResponse {
            id: "rollback-4".into(),
            ok: true,
            result: Some(serde_json::json!({
                "rolled_back": false,
                "status": "failed",
                "message": "model_version 'logistic-train-OLD' has no weights stored (promoted before v0.20); cannot rollback",
            })),
            error: None,
        };
        let rb = parse_rollback_response(&r).expect("ok");
        assert!(!rb.rolled_back);
        assert!(rb.message.unwrap().contains("no weights"));
    }

    #[test]
    fn parse_rollback_response_not_ok_returns_err() {
        // v0.20a — envelope-level error
        let r = SidecarResponse {
            id: "rollback-5".into(),
            ok: false,
            result: None,
            error: Some("internal: oops".into()),
        };
        assert!(parse_rollback_response(&r).is_err());
    }

    #[test]
    fn build_auto_promote_if_better_request_default_margin() {
        // v0.23a — no params → empty params object (Python
        // uses its default 0.005 margin)
        let line = build_auto_promote_if_better_request("ap-1", None, None);
        let v: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(v["method"], "auto_promote_if_better");
        assert_eq!(v["id"], "ap-1");
        assert_eq!(v["params"], serde_json::json!({}));
    }

    #[test]
    fn build_auto_promote_if_better_request_with_margin_and_trial() {
        // v0.23a — both params
        let line = build_auto_promote_if_better_request("ap-2", Some(0.01), Some(2));
        let v: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(v["params"]["brier_margin"], 0.01);
        assert_eq!(v["params"]["trial_index"], 2);
    }

    #[test]
    fn parse_auto_promote_if_better_response_promoted() {
        // v0.23a — promoted case (candidate was meaningfully better)
        let r = SidecarResponse {
            id: "ap-3".into(),
            ok: true,
            result: Some(serde_json::json!({
                "promoted": true,
                "skipped": false,
                "reason": "auto-promoted: improvement 0.0120 > margin 0.005",
                "candidate_brier": 0.180,
                "active_brier": 0.192,
                "margin": 0.005,
                "model_version": "logistic-train-XYZ",
                "promoted_at_ms": 1_700_020_000_000_i64,
            })),
            error: None,
        };
        let a = parse_auto_promote_if_better_response(&r).expect("ok");
        assert!(a.promoted);
        assert!(!a.skipped);
        assert!(a.reason.contains("auto-promoted"));
        assert_eq!(a.candidate_brier, Some(0.180));
        assert_eq!(a.active_brier, Some(0.192));
        assert_eq!(a.margin, 0.005);
        assert_eq!(a.model_version.as_deref(), Some("logistic-train-XYZ"));
    }

    #[test]
    fn parse_auto_promote_if_better_response_skipped() {
        // v0.23a — skipped case (candidate wasn't meaningfully better)
        let r = SidecarResponse {
            id: "ap-4".into(),
            ok: true,
            result: Some(serde_json::json!({
                "promoted": false,
                "skipped": true,
                "reason": "candidate brier 0.1900 is not at least 0.005 better than active 0.1920 (improvement: +0.0020)",
                "candidate_brier": 0.190,
                "active_brier": 0.192,
                "margin": 0.005,
                "model_version": null,
                "promoted_at_ms": null,
            })),
            error: None,
        };
        let a = parse_auto_promote_if_better_response(&r).expect("ok");
        assert!(!a.promoted);
        assert!(a.skipped);
        assert!(a.reason.contains("not at least 0.005 better"));
        assert_eq!(a.candidate_brier, Some(0.190));
        assert_eq!(a.active_brier, Some(0.192));
        assert!(a.model_version.is_none());
    }

    #[test]
    fn parse_auto_promote_if_better_response_no_active() {
        // v0.23a — no active model: auto-promotes the candidate
        let r = SidecarResponse {
            id: "ap-5".into(),
            ok: true,
            result: Some(serde_json::json!({
                "promoted": true,
                "skipped": false,
                "reason": "no active model; auto-promoted the candidate",
                "candidate_brier": null,
                "active_brier": null,
                "margin": 0.005,
                "model_version": "logistic-train-ABC",
                "promoted_at_ms": 1_700_021_000_000_i64,
            })),
            error: None,
        };
        let a = parse_auto_promote_if_better_response(&r).expect("ok");
        assert!(a.promoted);
        assert!(a.candidate_brier.is_none());
        assert!(a.active_brier.is_none());
    }

    #[test]
    fn parse_auto_promote_if_better_response_not_ok_returns_err() {
        // v0.23a — envelope-level error
        let r = SidecarResponse {
            id: "ap-6".into(),
            ok: false,
            result: None,
            error: Some("internal: oops".into()),
        };
        assert!(parse_auto_promote_if_better_response(&r).is_err());
    }

    #[test]
    fn response_ok_constructor() {
        let r = SidecarResponse::ok("r1", serde_json::json!({"x": 1}));
        assert!(r.ok);
        assert_eq!(r.id, "r1");
        assert!(r.result.is_some());
        assert!(r.error.is_none());
    }

    #[test]
    fn response_err_constructor() {
        let r = SidecarResponse::err("r2", "boom");
        assert!(!r.ok);
        assert_eq!(r.error.as_deref(), Some("boom"));
        assert!(r.result.is_none());
    }

    #[test]
    fn parse_line_distinguishes_req_vs_resp() {
        let resp_line = r#"{"id":"r1","ok":true,"result":{"x":1}}"#;
        match parse_line(resp_line).unwrap() {
            ParseResult::Response(r) => {
                assert_eq!(r.id, "r1");
                assert!(r.ok);
            }
            _ => panic!("expected response"),
        }

        let req_line = r#"{"id":"r2","method":"ping","params":{}}"#;
        match parse_line(req_line).unwrap() {
            ParseResult::Request(r) => {
                assert_eq!(r.method, "ping");
            }
            _ => panic!("expected request"),
        }
    }

    #[test]
    fn parse_line_rejects_invalid() {
        assert!(parse_line("not json").is_err());
        assert!(parse_line(r#"{"id":"r1"}"#).is_err()); // neither method nor ok
    }

    #[test]
    fn build_and_parse_predict_round_trip() {
        let line = build_predict_request("r1", &[
            ("m1".to_string(), 0.5),
            ("m2".to_string(), 0.7),
        ]);
        // Parse back as a request
        let parsed = match parse_line(&line).unwrap() {
            ParseResult::Request(r) => r,
            _ => panic!(),
        };
        assert_eq!(parsed.method, "predict");
        assert_eq!(parsed.id, "r1");
        let markets = parsed.params.get("markets").and_then(|v| v.as_array()).unwrap();
        assert_eq!(markets.len(), 2);

        // Build a fake response and round-trip
        let resp = SidecarResponse::ok("r1", serde_json::json!({
            "predictions": [
                { "market_id": "m1", "prob": 0.65, "confidence": 0.7, "rationale": "why" },
                { "market_id": "m2", "prob": 0.3, "confidence": 0.6, "rationale": null },
            ],
            "model_version": "logistic-train-abc123"
        }));
        let result = parse_predict_response(&resp).unwrap();
        assert_eq!(result.predictions.len(), 2);
        assert_eq!(result.predictions[0].market_id, "m1");
        assert!((result.predictions[0].prob - 0.65).abs() < 1e-9);
        assert_eq!(result.predictions[0].rationale.as_deref(), Some("why"));
        assert!(result.predictions[1].rationale.is_none());
        // v0.12a — model_version is hoisted to the top level
        assert_eq!(result.model_version.as_deref(), Some("logistic-train-abc123"));
    }

    #[test]
    fn parse_predict_response_model_version_optional() {
        // Back-compat: if the sidecar doesn't send model_version,
        // we still parse OK with model_version = None.
        let resp = SidecarResponse::ok("r1", serde_json::json!({
            "predictions": [
                { "market_id": "m1", "prob": 0.5, "confidence": 0.5 },
            ]
        }));
        let result = parse_predict_response(&resp).unwrap();
        assert_eq!(result.predictions.len(), 1);
        assert!(result.model_version.is_none());
        assert!(result.brier_score.is_none());
    }

    #[test]
    fn parse_predict_response_brier_round_trip() {
        // v0.13b — brier_score is hoisted from the response.
        let resp = SidecarResponse::ok("r1", serde_json::json!({
            "predictions": [
                { "market_id": "m1", "prob": 0.6, "confidence": 0.5 },
            ],
            "model_version": "logistic-train-abc123",
            "brier_score": 0.220012
        }));
        let result = parse_predict_response(&resp).unwrap();
        assert_eq!(result.predictions.len(), 1);
        assert_eq!(result.model_version.as_deref(), Some("logistic-train-abc123"));
        assert_eq!(result.brier_score, Some(0.220012));
    }

    #[test]
    fn parse_predict_error_response() {
        let resp = SidecarResponse::err("r1", "model not loaded");
        let r = parse_predict_response(&resp);
        assert!(r.is_err());
        assert_eq!(r.unwrap_err(), "model not loaded");
    }

    #[test]
    fn parse_predict_missing_field() {
        let resp = SidecarResponse::ok("r1", serde_json::json!({"other": 1}));
        let r = parse_predict_response(&resp);
        assert!(r.is_err());
    }
}
