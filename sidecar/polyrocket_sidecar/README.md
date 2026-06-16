# polyrocket-sidecar

Real Python implementation of the `domain::lab::sidecar` JSON-RPC protocol.

The polyrocket Tauri app spawns this process (via `commands::sidecar`) and
exchanges one JSON object per line over stdio. The protocol is symmetric
on both sides: any future Rust change to `SidecarRequest` / `SidecarResponse`
MUST be matched by an update here.

## Methods (v0.7b)

| method        | purpose                                       | impl status       |
|---------------|-----------------------------------------------|-------------------|
| `Ping`        | health check, returns `pong`                  | ✅ implemented    |
| `Predict`     | single signal → win probability               | ✅ logistic       |
| `TrainJob`    | (placeholder) hyperparameter sweep            | ⏳ stub           |
| `PromoteModel`| (placeholder) flip a model to active          | ⏳ stub           |

The `Predict` method currently uses a deterministic logistic regression over
the signal's edge + horizon + confidence, trained on synthetic data. This is
intentionally simple — the Tauri app treats it as a black box; the goal of
v0.7b is to verify the JSON-RPC plumbing end-to-end, not to ship a strong
model. Replace the body of `predict_logic()` in a later release.

## Wire protocol

```
→ {"id": 1, "method": "Ping", "params": {}}
← {"id": 1, "result": "pong"}
→ {"id": 2, "method": "Predict", "params": {"features": {"edge": 0.12, "horizon": 24, "confidence": 0.8}}}
← {"id": 2, "result": {"prob": 0.61, "model_version": "logistic-0.1.0"}}
→ {"id": 3, "method": "Predict", "params": {"features": {"edge": 9.9}}}
← {"id": 3, "error": {"code": -32602, "message": "feature 'horizon' missing"}}
```

JSON-RPC 2.0-flavoured but informal: no batch support, no `jsonrpc: "2.0"`
field required. Both sides are tolerant.

## Run locally

```bash
# interactive REPL — type JSON lines, get JSON lines
python -m polyrocket_sidecar

# or invoke the smoke
python scripts/smoke.py

# or test directly
pytest tests/
```

## Run under Tauri

The Rust side spawns `python3 -m polyrocket_sidecar` (overridable via
`POLYROCKET_SIDECAR_CMD`). Communication is line-delimited JSON over
stdin/stdout, exactly as in the local REPL.
