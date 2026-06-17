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
}

impl SidecarMethod {
    pub fn as_str(self) -> &'static str {
        match self {
            SidecarMethod::Ping => "ping",
            SidecarMethod::Predict => "predict",
            SidecarMethod::TrainJob => "train_job",
            SidecarMethod::PromoteModel => "promote_model",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "ping" => Some(SidecarMethod::Ping),
            "predict" => Some(SidecarMethod::Predict),
            "train_job" => Some(SidecarMethod::TrainJob),
            "promote_model" => Some(SidecarMethod::PromoteModel),
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
pub fn build_promote_request(id: impl Into<String>, job_id: Option<&str>) -> String {
    let params = match job_id {
        Some(j) => serde_json::json!({ "job_id": j }),
        None => serde_json::json!({}),
    };
    let req = SidecarRequest {
        id: id.into(),
        method: SidecarMethod::PromoteModel.as_str().to_string(),
        params,
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
    /// New model version string (e.g. `logistic-train-441c352b`).
    /// Empty string on failure.
    pub model_version: String,
    /// Human-readable error message on failure; `None` on success.
    pub message: Option<String>,
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
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_round_trip() {
        for m in [SidecarMethod::Ping, SidecarMethod::Predict,
                  SidecarMethod::TrainJob, SidecarMethod::PromoteModel] {
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
        let line = build_promote_request("promote-123", None);
        let v: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(v["method"], "promote_model");
        assert_eq!(v["id"], "promote-123");
        assert!(v["params"].as_object().unwrap().is_empty());
    }

    #[test]
    fn build_promote_request_with_job_id() {
        // v0.18a — job_id is included
        let line = build_promote_request("promote-456", Some("train-abc"));
        let v: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(v["params"]["job_id"], "train-abc");
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
