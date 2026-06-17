//! Integration test: spawn the real `polyrocket-sidecar` Python process and
//! round-trip a few requests. This proves the JSON-RPC wire format works
//! end-to-end. Skipped if `python3` is not on PATH.
//!
//! Run with:
//!     cargo test --test sidecar_e2e
//!
//! Requires the sidecar/ directory at the repo root.

use polyrocket_lib::domain::lab::sidecar::{
    build_predict_request, parse_line, parse_predict_response, ParseResult, SidecarMethod,
    SidecarRequest, SidecarResponse,
};
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

fn python_bin() -> Option<String> {
    // Honor explicit override, fall back to python3
    std::env::var("POLYROCKET_SIDECAR_PY").ok().or_else(|| {
        // Try a few names; cargo test may not have python3 on PATH on Windows CI.
        for name in ["python3", "python"] {
            if Command::new(name).arg("--version").output().is_ok() {
                return Some(name.to_string());
            }
        }
        None
    })
}

fn sidecar_repo_dir() -> Option<std::path::PathBuf> {
    // The sidecar/ directory lives at <repo>/sidecar/. When cargo runs the
    // test from src-tauri, the manifest dir is src-tauri/. So we go up one.
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let candidate = manifest_dir.parent()?.join("sidecar");
    if candidate.exists() {
        Some(candidate)
    } else {
        None
    }
}

/// Per-test temporary model dir so parallel tests don't clobber each
/// other's `~/.polyrocket/sidecar/models/`.
fn make_tmp_model_dir(test_name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "polyrocket-sidecar-test-{}-{}",
        test_name,
        std::process::id()
    ));
    let _ = std::fs::create_dir_all(&dir);
    dir
}

fn spawn_sidecar() -> Option<(std::process::Child, std::path::PathBuf, String)> {
    spawn_sidecar_in("default")
}

fn spawn_sidecar_in(test_name: &str) -> Option<(std::process::Child, std::path::PathBuf, String)> {
    let py = python_bin()?;
    let repo = sidecar_repo_dir()?;
    let model_dir = make_tmp_model_dir(test_name);
    let mut child = Command::new(&py)
        .arg("-m")
        .arg("polyrocket_sidecar")
        .current_dir(&repo)
        .env("POLYROCKET_SIDECAR_MODEL_DIR", &model_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .ok()?;
    Some((child, repo, py))
}

fn send_line<R: BufRead, W: Write>(
    stdin: &mut W,
    stdout: &mut R,
    payload: &str,
) -> Result<String, String> {
    writeln!(stdin, "{payload}").map_err(|e| format!("write: {e}"))?;
    stdin.flush().map_err(|e| format!("flush: {e}"))?;
    let mut response = String::new();
    stdout
        .read_line(&mut response)
        .map_err(|e| format!("read: {e}"))?;
    Ok(response)
}

#[test]
fn sidecar_ping_round_trip() {
    let Some((mut child, _repo, _py)) = spawn_sidecar() else {
        eprintln!("[skip] python3 or sidecar/ not available");
        return;
    };
    let mut stdin = child.stdin.take().expect("stdin");
    let stdout = child.stdout.take().expect("stdout");
    let mut stdout = BufReader::new(stdout);

    let req = SidecarRequest {
        id: "rt-1".into(),
        method: SidecarMethod::Ping.as_str().into(),
        params: serde_json::json!({}),
    };
    let payload = serde_json::to_string(&req).unwrap();
    let response = send_line(&mut stdin, &mut stdout, &payload).expect("round trip");
    let parsed = parse_line(&response).expect("parse");
    let resp = match parsed {
        ParseResult::Response(r) => r,
        _ => panic!("expected response, got: {parsed:?}"),
    };
    assert_eq!(resp.id, "rt-1");
    assert!(resp.ok, "ok should be true, got: {resp:?}");
    let result = resp.result.expect("result");
    assert!(result.get("pong").and_then(|v| v.as_bool()).unwrap_or(false));
    let _ = child.kill();
}

#[test]
fn sidecar_predict_round_trip() {
    let Some((mut child, _repo, _py)) = spawn_sidecar() else {
        eprintln!("[skip] python3 or sidecar/ not available");
        return;
    };
    let mut stdin = child.stdin.take().expect("stdin");
    let stdout = child.stdout.take().expect("stdout");
    let mut stdout = BufReader::new(stdout);

    let markets = vec![
        ("m1".to_string(), 0.2),
        ("m2".to_string(), 0.8),
        ("m3".to_string(), 0.5),
    ];
    let payload = build_predict_request("rt-2", &markets);
    let response = send_line(&mut stdin, &mut stdout, &payload).expect("round trip");
    let parsed = parse_line(&response).expect("parse");
    let resp = match parsed {
        ParseResult::Response(r) => r,
        _ => panic!("expected response, got: {parsed:?}"),
    };
    assert_eq!(resp.id, "rt-2");
    assert!(resp.ok, "ok should be true: {resp:?}");
    let preds = parse_predict_response(&resp).expect("decode predictions");
    assert_eq!(preds.len(), 3);
    // m1 (cheap) should score higher than m2 (expensive)
    let m1 = preds.iter().find(|p| p.market_id == "m1").expect("m1");
    let m2 = preds.iter().find(|p| p.market_id == "m2").expect("m2");
    assert!(
        m1.prob > m2.prob,
        "m1 should have higher prob than m2 (cheap YES > expensive YES): m1={} m2={}",
        m1.prob, m2.prob
    );
    for p in &preds {
        assert!(p.prob >= 0.0 && p.prob <= 1.0);
        assert!(p.confidence >= 0.0 && p.confidence <= 1.0);
    }
    let _ = child.kill();
}

#[test]
fn sidecar_unknown_method_returns_ok_false() {
    let Some((mut child, _repo, _py)) = spawn_sidecar() else {
        eprintln!("[skip] python3 or sidecar/ not available");
        return;
    };
    let mut stdin = child.stdin.take().expect("stdin");
    let stdout = child.stdout.take().expect("stdout");
    let mut stdout = BufReader::new(stdout);

    let req = SidecarRequest {
        id: "rt-3".into(),
        method: "what_is_this".into(),
        params: serde_json::json!({}),
    };
    let payload = serde_json::to_string(&req).unwrap();
    let response = send_line(&mut stdin, &mut stdout, &payload).expect("round trip");
    let parsed = parse_line(&response).expect("parse");
    let resp = match parsed {
        ParseResult::Response(r) => r,
        _ => panic!("expected response"),
    };
    assert_eq!(resp.id, "rt-3");
    assert!(!resp.ok, "should be ok=false for unknown method");
    assert!(resp.error.is_some(), "should have error message");
    let _ = child.kill();
}

#[test]
fn sidecar_train_job_runs_real_sweep() {
    let Some((mut child, _repo, _py)) = spawn_sidecar_in("train_sweep") else {
        eprintln!("[skip] python3 or sidecar/ not available");
        return;
    };
    let mut stdin = child.stdin.take().expect("stdin");
    let stdout = child.stdout.take().expect("stdout");
    let mut stdout = BufReader::new(stdout);

    // v0.10b — train_job is no longer a stub. It runs a hyperparameter
    // sweep and writes a candidate file. We just verify the response
    // shape here (a real e2e would inspect the candidate file too).
    let req = SidecarRequest {
        id: "rt-4".into(),
        method: SidecarMethod::TrainJob.as_str().into(),
        params: serde_json::json!({ "n_trials": 1, "epochs": 5 }),
    };
    let payload = serde_json::to_string(&req).unwrap();
    let response = send_line(&mut stdin, &mut stdout, &payload).expect("round trip");
    let parsed = parse_line(&response).expect("parse");
    let resp = match parsed {
        ParseResult::Response(r) => r,
        _ => panic!("expected response"),
    };
    assert_eq!(resp.id, "rt-4");
    assert!(resp.ok, "train_job should return ok=true: {resp:?}");
    // Result should have a job_id and best_brier from the sweep
    let result = resp.result.expect("train_job result");
    assert!(result.get("job_id").is_some(), "expected job_id in result");
    assert!(result.get("best_brier").is_some(), "expected best_brier");
    let _ = child.kill();
}

#[test]
fn sidecar_promote_model_round_trip() {
    let Some((mut child, _repo, _py)) = spawn_sidecar_in("promote") else {
        eprintln!("[skip] python3 or sidecar/ not available");
        return;
    };
    let mut stdin = child.stdin.take().expect("stdin");
    let stdout = child.stdout.take().expect("stdout");
    let mut stdout = BufReader::new(stdout);

    // First train (writes candidate), then promote (copies to active).
    // Both should return ok=true.
    let train_req = SidecarRequest {
        id: "rt-5a".into(),
        method: SidecarMethod::TrainJob.as_str().into(),
        params: serde_json::json!({ "n_trials": 1, "epochs": 3 }),
    };
    let train_payload = serde_json::to_string(&train_req).unwrap();
    let train_resp = send_line(&mut stdin, &mut stdout, &train_payload).expect("train round trip");
    let train_parsed = parse_line(&train_resp).expect("parse");
    let train_resp = match train_parsed {
        ParseResult::Response(r) => r,
        _ => panic!("expected response"),
    };
    assert!(train_resp.ok, "train should be ok: {train_resp:?}");

    let promote_req = SidecarRequest {
        id: "rt-5b".into(),
        method: SidecarMethod::PromoteModel.as_str().into(),
        params: serde_json::json!({}),
    };
    let promote_payload = serde_json::to_string(&promote_req).unwrap();
    let promote_resp = send_line(&mut stdin, &mut stdout, &promote_payload).expect("promote round trip");
    let promote_parsed = parse_line(&promote_resp).expect("parse");
    let promote_resp = match promote_parsed {
        ParseResult::Response(r) => r,
        _ => panic!("expected response"),
    };
    assert!(promote_resp.ok, "promote should be ok: {promote_resp:?}");
    let result = promote_resp.result.expect("promote result");
    assert_eq!(result.get("promoted").and_then(|v| v.as_bool()), Some(true));
    assert!(result.get("active_path").is_some());
    let _ = child.kill();
}

#[test]
fn sidecar_ping_blocking_returns_pong() {
    // v0.11b — exercise SidecarState::ping_blocking end-to-end
    // against a real Python sidecar process. Verifies the lock
    // discipline (write under stdin lock, read under stdout lock)
    // and the JSON-RPC round-trip.
    use polyrocket_lib::SidecarState;
    use polyrocket_lib::SidecarStatus;
    use std::process::{Command, Stdio};
    let Some((_child_ignored, repo, py)) = spawn_sidecar() else {
        eprintln!("[skip] python3 or sidecar/ not available");
        return;
    };
    let mut child = Command::new(&py)
        .arg("-m").arg("polyrocket_sidecar")
        .current_dir(&repo)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn sidecar");
    let stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let state = SidecarState::new();
    *state.stdin.lock().unwrap() = Some(stdin);
    *state.stdout.lock().unwrap() = Some(stdout);
    state.set_status(SidecarStatus {
        running: true, pid: Some(child.id()), command: "polyrocket-sidecar".into(), last_error: None,
    });
    *state.child.lock().unwrap() = Some(child);

    let started = std::time::Instant::now();
    let result = state.ping_blocking(2000);
    let elapsed = started.elapsed();
    let latency = result.expect("ping should succeed against running sidecar");
    assert!(latency < 2000, "ping took too long: {}ms", latency);
    assert!(elapsed.as_millis() < 2500, "wall clock over 2.5s: {:?}", elapsed);
    eprintln!("ping_blocking latency: {}ms", latency);
}
