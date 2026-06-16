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

/// Parse a `predict` response into Vec<Prediction>.
/// Returns Err if the response is `ok=false`.
pub fn parse_predict_response(
    resp: &SidecarResponse,
) -> Result<Vec<Prediction>, String> {
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
    Ok(out)
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
            ]
        }));
        let preds = parse_predict_response(&resp).unwrap();
        assert_eq!(preds.len(), 2);
        assert_eq!(preds[0].market_id, "m1");
        assert!((preds[0].prob - 0.65).abs() < 1e-9);
        assert_eq!(preds[0].rationale.as_deref(), Some("why"));
        assert!(preds[1].rationale.is_none());
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
