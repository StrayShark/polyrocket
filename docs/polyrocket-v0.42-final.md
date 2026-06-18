# polyrocket v0.42 — final

**Branch**: main (local-only, not pushed)
**Commits**: `f68a90d` v0.42a → ... → `76cbc9a` v0.42e-3 → `<this commit>` v0.42f
**Released**: 2026-06-18 (local, awaiting user push)

## What changed since v0.41

| sub-version | hash       | one-liner                                                | tests at landing |
|-------------|------------|----------------------------------------------------------|------------------|
| v0.42a      | `f68a90d`  | Telemetry module (infra/telemetry.rs) + 5 tests          | 707              |
| v0.42b      | `c91eaf8`  | Wire telemetry into 5 schedulers + train/promote IPCs    | 707              |
| v0.42c      | `a5be107`  | L1 Settings toggle + set_telemetry_enabled IPC           | 711              |
| v0.42d      | `3159245`  | overview.md refresh (IPC 39→83, schedulers 3→5)          | 711              |
| v0.42e-1    | `53a79e0`  | CI: .github/workflows/l1-tauri-guard.yml                 | 711              |
| v0.42e-2    | `03fbd0d`  | OS notification on skipped auto-promote (opt-in)         | 714              |
| v0.42e-3    | `76cbc9a`  | ModelComparison with weights from archive                | 718              |
| v0.42f      | this file  | Ship log + tally                                         | 718              |

**Test totals at v0.42 final**: cargo 264/264, vitest 351/351, python 70/70, script 31/31. **Total 685 (vitest+cargo+python) + 31 script = 716 total.** (Up from 702 in v0.41 — 7 new telemetry + 3 new skipped-notify + 2 new ModelComparison weights + 1 archive filter.)

## Highlights

### Direction A — Telemetry + governance (v0.42a–d)

A long-overdue opt-in telemetry path. Default OFF;
turn on with `POLYROCKET_TELEMETRY=1` in the env,
or via the Settings telemetry toggle (runtime
override).

**Events captured** (NDJSON to stderr):

- `scheduler_tick` / `scheduler_error` — per loop
  (5 loops, errors only)
- `train_started` / `train_completed` / `train_failed`
- `promote_completed` (on success only)
- `auto_promote_fired` / `auto_promote_skipped` — both branches
- `sidecar_connected` / `sidecar_disconnected` — transitions only
- `daily_brief_generated` — with duration + size
- `anomaly_detected` — primary kind, severity 1-3
- `llm_health_probe` — per provider, per attempt
- `mirror_executor_tick` — intents / executed / errors
- `audit_purged` — only when n > 0

**Wire format**: stable, snake_case, via serde's
`tag = "name" + rename_all = "snake_case"`. Future
sinks (file, Sentry) are a trait swap, not a redesign.

**What this is NOT**: a privacy violation. No PII,
no model weights, no secrets. Just `job_id`,
`loop_name`, `latency_ms`, `error`. The L1 toggle
default is OFF.

**Bonus — overview.md refresh**: the doc had drifted
significantly (IPC count 39 → 83, scheduler loops
3 → 5). v0.42d is the bookend for that debt. The
L1↔Tauri guard is the source of truth going forward.

### Direction A bonus — deferred 三件套 (v0.42e-1/2/3)

The three items deferred from prior finals that
fit cleanly into v0.42:

1. **CI integration of L1↔Tauri guard** (v0.32a
   script → v0.42e-1 GitHub Action). Pure Node, <2s
   runtime. Closes the gap where new IPCs without L1
   wrappers (or vice versa) wouldn't be caught by CI.
2. **OS notification on skipped auto-promote**
   (v0.42e-2). New `autoPromoteSkippedNotify` pref,
   default OFF (most users don't want "no
   improvement" pings every train). Mirrors the
   existing promoted-branch pattern.
3. **ModelComparison with weights from archive**
   (v0.42e-3). The deferred item from v0.40 final.
   list_promote_history_archive gains a `job_ids`
   whitelist filter. ModelComparison renders
   `w0/w1/w2` next to the existing `best_params`.
   Caveat: in-memory entries that haven't fallen off
   the 20-cap show "(no archive entry yet)" — same
   pattern as the existing pre-v0.18 best_params
   fallback.

## Files changed in v0.42

```
src-tauri/src/infra/telemetry.rs       | new (~280 LOC)
src-tauri/src/infra/scheduler/mod.rs   | 5 loops + 1 LLM probe
src-tauri/src/commands/sidecar.rs      | train/promote/auto-promote + telemetry IPCs + archive job_ids
src-tauri/src/domain/lab/sidecar.rs    | (unchanged; minor)
src-tauri/src/platform/env.rs          | env_str helper
src-tauri/src/lib.rs                   | telemetry::init_from_env + IPC registration
src/ipc.ts                             | setTelemetryEnabled + archive job_ids
src/lib/i18n.ts                        | +12 keys × 2 locales = 24 strings
src/lib/prefs-io.ts                    | telemetryEnabled + autoPromoteSkippedNotify
src/lib/prefs-io.test.ts               | updated for 2 new fields
src/stores/prefs-store.ts              | telemetryEnabled + autoPromoteSkippedNotify
src/routes/Settings.tsx                | TelemetryCard + skipped-notify toggle
src/routes/Settings.test.tsx           | +5 tests (3 telemetry + 2 skipped)
src/routes/ModelLab.tsx                | OS notif skipped branch + weights query
src/components/feedback/ModelComparison.tsx     | weights prop + UI
src/components/feedback/ModelComparison.test.tsx | +2 tests
docs/overview.md                       | refresh + new row
docs/polyrocket-v0.42-final.md         | (this file)
.github/workflows/l1-tauri-guard.yml   | new CI workflow
```

**Net change**: 18 files, +850 LOC.

## Migration / back-compat

- **No DB schema changes.**
- **No new sidecar method** (telemetry is Rust-internal;
  archive job_ids is a Rust-side filter on the existing
  Python file).
- **Pre-v0.42 entries**: no telemetry, no behavior
  change. The IPC + Settings toggle is opt-in.
- **prefs imports**: `parsePrefsFromString` defaults
  `telemetryEnabled` and `autoPromoteSkippedNotify`
  to false if missing (forward-compat with older
  export envelopes).
- **Rust feature flag**: none needed; telemetry is
  zero-cost when off (`AtomicBool` load + early
  return).
- **Backwards-compatible with v0.41 active.json /
  archive.jsonl** — both are forward-compatible
  formats; the v0.42 code reads them unchanged.

## What's next

After user push:
- **v0.43** — Direction B: backtest engine. Closes
  the "did this model actually beat the last one"
  gap. Replays historical data through a saved
  model, surfaces Brier / calibration / top winners
  + losers.
- **v0.44** — Direction C: paper trading mode. New
  `MirrorExecutorConfig.paper_mode` flag, new
  `paper_fills` table, Settings toggle, [PAPER]
  badges in Copy / PnL.
- **v0.50 milestone** — model lifecycle + trading
  parity. Trading-side features (advanced order
  types, conditional orders, post-only enforcement,
  fill analytics) start here.
