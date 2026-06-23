# polyrocket — Rust-only Architecture (v0.122+)

> **Status**: design (v0.122a, prep step)
> **Owner**: Mavis
> **Last update**: 2026-06-23

This document is the architecture blueprint for **removing the Python sidecar** from
polyrocket and consolidating all logic — UI calls, scheduled tasks, ML inference,
HTTP to external APIs — into a single Tauri Rust binary. It captures the *why*,
*what*, *how*, and the sub-version schedule.

It does **not** freeze every implementation detail; sub-version commits (v0.122b,
v0.122c, ...) update the design as we learn. The schedule and risk register are the
stable parts.

---

## 1. Why — current state, problems, motivation

### 1.1 What exists today (v0.121)

```
┌────────────┐   invoke()    ┌──────────────┐  spawn   ┌──────────────────┐
│  React 18  ├──────────────►│ Tauri Rust   ├─────────►│ Python sidecar   │
│  (webview) │               │ (single bin) │  stdin   │  polyrocket-     │
│            │◄──────────────┤              │◄─────────┤  sidecar (sub-   │
│  ipc.ts    │   events()    │  5,715 LOC   │ stdout   │  process)        │
└────────────┘               │  sidecar     │          │                  │
                             │  mirror code │          │  2,615 LOC       │
                             │              │          │  predict/train/  │
                             │              │          │  shap/active/... │
                             └──────────────┘          └──────────────────┘
```

The Tauri Rust core is a **thin protocol mirror** + process manager. The actual ML
algorithm lives in the Python sidecar (`polyrocket_sidecar/`). The Rust side:
- marshals JSON-RPC lines through `stdin` / `stdout`
- tracks child process lifecycle (`SidecarState`)
- exposes 11 methods (predict, train_job, promote_model, etc.) over Tauri commands

**Problem 1 — dual source of truth for ML math.**
The Rust scheduler already has `fallback_predict()` (mirrors Python's `predict_logic`)
with a parity test `fallback_predict_matches_python_baseline`. This is a **smoking
gun** for the dual-source problem: if the inline Python weights change, the Rust
test breaks. We cannot make one side authoritative without breaking the other.

**Problem 2 — protocol mirror bloat.**
`src-tauri/src/domain/lab/sidecar.rs` is 2,252 LOC of `SidecarRequest` /
`SidecarResponse` DTOs whose only job is to round-trip the same shape that
`sidecar/polyrocket_sidecar/protocol.py` (108 LOC of dataclasses) already defines.
The two implementations have to stay in sync forever.

**Problem 3 — 2 deployment artifacts.**
The desktop app currently ships:
- a compiled Tauri binary (~80 MB stripped)
- a Python wheel + `polyrocket-sidecar` CLI on PATH (or an embedded copy)

This doubles the release surface, requires `python3` on the user's machine, and
makes platform support (Windows, Linux) harder than it has to be.

**Problem 4 — already mostly Rust anyway.**
- All HTTP to external APIs (Polymarket CLOB, LLM providers) goes from Rust
  (`infra/http/mod.rs`, `domain/llm/*.rs`).
- All scheduled tasks (health probe, daily brief, anomaly detect, mirror executor,
  audit purge, paper_fills_reconcile, degradation_check) are `tokio::spawn` loops
  in `infra/scheduler/mod.rs`.
- The sidecar is called **only** from ModelLab + auto-promote cron; everything else
  is already Rust.

**Conclusion**: the sidecar is the last Python holdout. Removing it is the next
logical step of the v0.119 "Rust-only secret handling" consolidation.

### 1.2 What this document is NOT arguing

- **It is not arguing for a rewrite of the working system.** The sidecar works
  today; users have trained models, predictions are flowing, paper trades fire. We
  do not throw that away — we port it incrementally, one method at a time, with
  parity tests.
- **It is not arguing for a different ML framework.** The sidecar's "logistic
  regression on (price, market_age_hours)" is intentional simplicity (v0.10b
  comment: *"a working toy, not production ML"*). Rust's stdlib is sufficient.
- **It is not arguing for new external dependencies.** We do not add `linfa` or
  `smartcore` for this migration. The 3-feature logistic model is 6 lines of
  arithmetic.

---

## 2. What — target architecture

### 2.1 Target shape (post v0.122g)

```
┌────────────┐   invoke()    ┌──────────────────────────────────┐
│  React 18  ├──────────────►│ Tauri Rust Core (single binary)  │
│  (webview) │               │                                  │
│            │◄──────────────┤  commands/lab.rs                 │
│  ipc.ts    │   events()    │    ├─ train_job                  │
│            │               │    ├─ promote_model              │
│            │               │    ├─ backtest_model             │
│            │               │    ├─ explain_model              │
│            │               │    └─ shap_explain               │
│            │               │                                  │
│            │               │  domain/lab/inference.rs         │
│            │               │    ├─ predict()  (sigmoid)       │
│            │               │    ├─ train()    (sweep)         │
│            │               │    └─ ...                        │
│            │               │                                  │
│            │               │  infra/scheduler/mod.rs          │
│            │               │    └─ auto_promote_loop  (NEW)   │
│            │               │                                  │
│            │               │  infra/http/mod.rs (reqwest)     │
│            │               └─────────────┬────────────────────┘
└────────────┘                             │ HTTPS
                                           ▼
                  ┌────────────────────────────────────────┐
                  │ External APIs (all from Rust)          │
                  │  ├─ Polymarket CLOB / Gamma            │
                  │  ├─ LLM providers (MiniMax, OpenAI,..) │
                  │  └─ Sports data APIs                   │
                  └────────────────────────────────────────┘
```

### 2.2 Module layout

| Current (v0.121) | Target (v0.122g) | Notes |
|---|---|---|
| `sidecar/polyrocket_sidecar/*.py` (2,615 LOC) | **DELETED** | All algorithms move to Rust |
| `src-tauri/src/commands/sidecar.rs` (2,360 LOC) | `src-tauri/src/commands/lab.rs` (~500 LOC) | subprocess + JSON-RPC marshal removed |
| `src-tauri/src/commands/sidecar_health.rs` (28 LOC) | **DELETED** | degradation_check loop absorbs it |
| `src-tauri/src/commands/active_model.rs` (247 LOC) | `src-tauri/src/commands/active_model.rs` (unchanged) | Already reads active.json; no subprocess |
| `src-tauri/src/domain/lab/sidecar.rs` (2,252 LOC) | `src-tauri/src/domain/lab/inference.rs` (~400 LOC) | Protocol DTOs gone; just math + I/O |
| `src-tauri/src/domain/lab/train_progress.rs` (141 LOC) | unchanged | UI event stream already pure Rust |
| `src-tauri/src/infra/db/sidecar_health.rs` | **DELETED** | DB table dropped |
| `src-tauri/tests/sidecar_e2e.rs` (464 LOC) | `src-tauri/src/domain/lab/inference.rs` tests (~300 LOC) | Tests move closer to code |
| `sidecar/tests/*.py` (~600 LOC) | **DELETED** | Replaced by Rust parity tests |

**Net change**: approximately **−8,400 LOC** (algorithm token one source of truth +
protocol layer gone + tests consolidate).

### 2.3 IPC surface (post v0.122g)

| Tauri command | Rust impl | Caller | Old Python method |
|---|---|---|---|
| `train_job` | `domain::lab::inference::train()` | ModelLab | `train_job` |
| `promote_model` | `domain::lab::inference::promote()` | ModelLab | `promote_model` |
| `rollback_model` | `domain::lab::inference::rollback()` | PromoteHistory | `rollback_model` |
| `auto_promote_if_better` | `domain::lab::inference::auto_promote()` | auto_promote_loop cron | `auto_promote_if_better` |
| `backtest_model` | `domain::lab::inference::backtest()` | ModelLab | `backtest_model` |
| `explain_model` | `domain::lab::inference::explain()` | ModelLab | `explain_model` |
| `shap_explain` | `domain::lab::inference::shap()` | ModelLab | `shap_explain` |
| `list_promote_history` | `domain::lab::inference::history()` | PromoteHistory | `list_promote_history` |
| `promote_all_trials` | `domain::lab::inference::promote_all()` | ModelLab | `promote_all_trials` |
| `predict` (internal) | `domain::lab::inference::predict()` | signal recompute | `predict` |
| `get_active_model` | unchanged | Settings, dashboard | (already pure Rust) |
| `sidecar_status`, `start_sidecar`, `stop_sidecar` | **DELETED** | (removed from UI) | (no replacement) |
| `sidecar_predict`, `sidecar_predict_async` | **DELETED** | (callers switch to new path) | (replaced by `predict` IPC) |
| `sidecar_request` | **DELETED** | (internal generic dispatcher) | (no replacement) |

### 2.4 State management

`SidecarState` (`infra/state.rs`) currently holds:
- `child: Mutex<Option<Child>>` — child process handle
- `stdin: Mutex<Option<ChildStdin>>` — write end of pipe
- `stdout: Mutex<Option<ChildStdout>>` — read end of pipe
- `status: Mutex<SidecarStatus>` — health metadata

After v0.122g, the only state we still need is the **active model metadata cache**
(read from `active.json` with mtime invalidation). This already lives in
`active_model.rs` and is independent of the sidecar. We can drop `SidecarState`
entirely.

### 2.5 On-disk format

`active.json` (and the per-trial JSONs in `<sidecar model dir>/`) **do not
change format**. The Rust port reads and writes the same JSON. Existing
`train-XXX` artifacts stay valid.

The only on-disk change: `POLYROCKET_SIDECAR_MODEL_DIR` env var is renamed to
`POLYROCKET_LAB_MODEL_DIR` (semantic: it's no longer the *sidecar's* model dir,
it's the *lab's* model dir). Migration path: v0.122c reads both names; v0.122g
deletes the alias.

---

## 3. How — incremental migration plan

### 3.1 Sub-version schedule

| Sub-version | Scope | LOC delta | Risk |
|---|---|---|---|
| **v0.122a** | Prep: `POLYROCKET_DISABLE_SIDECAR` flag + this doc | +200 / −0 | none (flag is opt-in, default = current behavior) |
| **v0.122b** | Port `ping` + `predict` (the hot path used by signal recompute) | +200 / −150 | low — `fallback_predict` already exists, port is parity-only |
| **v0.122c** | Port `train_job` + `promote_model` + `rollback_model` | +600 / −0 | medium — synthetic data gen is non-trivial |
| **v0.122d** | Port `auto_promote_if_better` + new cron `auto_promote_loop` | +300 / −50 | low — pure file I/O + Brier compare |
| **v0.122e** | Port `backtest_model` + `explain_model` (reuses predict) | +200 / −0 | low |
| **v0.122f** | Port `shap_explain` (KernelExplainer coalition sampling) | +400 / −0 | medium — algorithm port needs care |
| **v0.122g** | **Delete** `sidecar/` dir + `commands/sidecar*.rs` + `domain/lab/sidecar.rs` + `infra/db/sidecar_health.rs` + `tests/sidecar_e2e.rs` + Tauri config | +0 / **−8,400** | high — only after all methods are ported + tested |
| **v0.122h** | CI cleanup (remove Python matrix, simplify workflows) | workflow −15 | low |

**Total**: ~8 sub-versions, ~2-3 weeks of focused work.

### 3.2 Per-method port checklist

For each method (`predict`, `train_job`, etc.):
1. **Read** the Python implementation end-to-end.
2. **Write** Rust port as a pure function in `domain::lab::inference` (no I/O, no
   `AppError`).
3. **Test** parity against Python: feed the same inputs, assert output identical
   to 1e-9.
4. **Wire** into Tauri command: replace subprocess + JSON-RPC parse with direct
   function call.
5. **Update** `commands::lab` so the IPC signature stays identical (so the UI
   doesn't have to change).
6. **Test** end-to-end from UI: ModelLab click "Train" → verify trial results →
   click "Promote" → verify `active.json` updated.
7. **Only then** mark the Python method as deprecated in the dispatch table
   (keeps both paths live until v0.122g).
8. **Delete** the Python method in v0.122g.

### 3.3 Test strategy

Three layers of tests, mirroring the v0.121 coverage ramp discipline:

1. **Unit** (Rust): `domain::lab::inference` tests. Pure function in/out, parity
   against recorded Python outputs. ≥90% branch coverage.
2. **Integration** (Rust): `commands::lab` tests with a temp `active.json` and
   in-memory DB. Asserts IPC return shapes match `src/types/generated/index.ts`.
3. **End-to-end** (UI): `ModelLab.e2e` Playwright test (out of scope for v0.122,
   pre-existing Playwright setup can be extended). Asserts click → result.

**Parity test pattern** (per port):
```rust
#[test]
fn predict_matches_python_v121() {
    // Recorded from sidecar/polyrocket_sidecar/predict.py
    // at git SHA bbfc2c4 (last commit before v0.122a).
    let cases: &[(f64, f64, f64)] = &[
        // (price, market_age_hours, expected_prob)
        (0.5, 24.0,  0.6357),  // computed by hand from sigmoid
        (0.7, 12.0,  0.5763),
        (0.3, 168.0, 0.7823),
    ];
    for &(price, age, expected) in cases {
        let p = predict_logic(price, age);
        assert!((p - expected).abs() < 1e-3,
            "predict({price}, {age}) = {p}, expected {expected}");
    }
}
```

### 3.4 Rollback strategy

The `POLYROCKET_DISABLE_SIDECAR=1` flag (v0.122a) is the **kill switch** for
the whole migration:
- If v0.122b breaks `predict` → unset the flag, app still works via Python.
- If v0.122c breaks `train_job` → unset the flag.
- Per-method granularity not possible (all-or-nothing today). v0.122g is the
  point of no return.

The flag is checked at the top of every sidecar IPC entry point in
`commands/sidecar.rs`. When set, the IPC returns
`AppError::Internal("sidecar disabled by POLYROCKET_DISABLE_SIDECAR=1 (v0.122+ migration in progress)")`.

After v0.122b, the flag's semantics flip: it becomes **opt-out** of the
Python sidecar. New Rust impls run by default; the flag falls back to the
deprecated Python path. This dual-path mode is the steady state from v0.122b
to v0.122g.

### 3.5 `POLYROCKET_DISABLE_SIDECAR` semantics (timeline)

| Sub-version | Flag unset (default) | Flag set |
|---|---|---|
| v0.122a (now) | Python sidecar (current) | All sidecar IPCs return "disabled" error |
| v0.122b-v0.122f | **New Rust impl** runs (port per method) | Falls back to Python for unported methods |
| v0.122g+ | New Rust impl only | New Rust impl only (Python deleted) |

---

## 4. Risks and mitigations

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Parity test fails for a method (Rust ≠ Python) | medium | medium | Log diff to JSON, debug with realistic data. If unfixable, keep Python for that method until next attempt |
| User has v0.121-era `train-XXX` artifacts that don't read with new Rust | low | low | JSON format unchanged. Test by loading one before/after v0.122g |
| `auto_promote_loop` cron over-fires after v0.122d | medium | medium | Reuse `set_auto_promote_config` prefs (already exists). Add backoff if last attempt failed |
| `shap_explain` numerical drift in Rust | medium | low | Tolerance: assert parity to 1e-6 (KernelExplainer is approximate anyway) |
| Tauri build CI breaks when we remove sidecar in v0.122g | low | medium | Test on macOS first, then Linux, then Windows. CI matrix already exists |
| `scripts/snapshot_pages.py` (playwright docs tool) accidentally deleted | medium | low | It's a docs tool, not part of the runtime. Move to `docs/preview-renderer/` subdir in v0.122i (separate concern) |
| Brier score regression: Rust compute differs from Python's by > 0.001 | low | high | Add `compute_live_brier` to parity test set. Block on test pass before promoting |
| E2E Playwright test flakes (click before render) | medium | low | Reuse v0.121's `data-testid` pattern; explicit waits; CI retries |

---

## 5. Success criteria

v0.122 is "done" when:

- [ ] `sidecar/` directory deleted from repo
- [ ] `src-tauri/src/commands/sidecar.rs` and `commands/sidecar_health.rs` deleted
- [ ] `src-tauri/src/domain/lab/sidecar.rs` renamed to `inference.rs`
- [ ] `src-tauri/src/infra/db/sidecar_health.rs` deleted; DB migration applied
- [ ] `src-tauri/tests/sidecar_e2e.rs` deleted; coverage re-allocated to `inference.rs` tests
- [ ] Tauri config no longer declares a sidecar binary
- [ ] All 11 methods ported with parity tests (≥90% branch coverage on `inference.rs`)
- [ ] `cargo test` green; `pnpm test:coverage` green
- [ ] `pnpm tauri build` produces a single binary on macOS, Linux, Windows
- [ ] No `python3` or `polyrocket-sidecar` references in the built app
- [ ] User can: launch app → click Train → see trial results → click Promote →
      see model version bump → trigger backtest → see SHAP explanation. All
      without subprocess, without Python.

---

## 6. Open questions

1. **Do we need a `linfa` integration at all?** Currently no. The 3-feature
   logistic model is 6 lines of arithmetic. If a future vertical needs gradient
   boosting, revisit. (Decision: **no** for v0.122.)
2. **Should `compute_live_brier` move into `domain::lab::inference`?** It's
   currently in `infra/scheduler/mod.rs` for cron. After v0.122d, the cron uses
   `inference::auto_promote()`, and `compute_live_brier` becomes a private
   helper. (Decision: **yes, move it** in v0.122d.)
3. **Do we keep the `SidecarState` struct around for backward compat?** No.
   v0.122g deletes it. The Tauri commands stop taking `State<'_, SidecarState>`
   as an argument.
4. **What about `infra/db/sidecar_health.rs`?** This is a DB table tracking
   sidecar health over time. After v0.122g, the table is dead. v0.122g includes
   a SQL migration to drop it.

---

## 7. References

- v0.121 last commit with sidecar: `bbfc2c4` (v0.121a) — parity tests should
  freeze inputs at this SHA.
- `src-tauri/src/infra/scheduler/mod.rs::fallback_predict` — the smoking gun
  for dual-source drift.
- `docs/coding-spec.md §15.5` — cron design; v0.122d adds auto_promote_loop
  following the same pattern.
- `docs/polyrocket-llm-management.md` — LLM provider dispatch (the
  sidecar's "other cousin" in the architecture).

---

## 8. Revision history

| Date | Version | Author | Change |
|---|---|---|---|
| 2026-06-23 | 0.1 (v0.122a) | Mavis | Initial design |
