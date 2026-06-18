# polyrocket v0.48 — final

**Branch**: main (local-only, not pushed)
**Commits**: `b17c2fe` v0.48a/b → `<this commit>` v0.48c
**Released**: 2026-06-18 (local, awaiting user push)

## What changed since v0.47

| sub-version | hash       | one-liner                                                | tests at landing |
|-------------|------------|----------------------------------------------------------|------------------|
| v0.48a/b    | `b17c2fe`  | Model degradation detector + alert toggle               | 745              |
| v0.48c      | this file  | Ship log + tally                                         | 745              |

**Test totals at v0.48 final**: cargo 239/239, vitest 362/362, python 77/77, script 31/31. **Total 678 (vitest+cargo+python) + 31 script = 709 total.** (Up from 704 in v0.47 — +3 cargo + +2 vitest = +5 net.)

## Highlights

### Model degradation detector (v0.48a-b)

The 7th scheduler loop runs every hour, computes
the **live Brier** of the FALLBACK model on
recently-resolved markets, and emits a telemetry
event with an `alert` flag. The L1 listens and
fires an OS notification when the alert is on.

**The flow**:
1. `run_degradation_check_loop` ticks every
   3600s (1 hour)
2. `compute_live_brier` joins markets +
   `price_snapshots` (the v0.47 LEFT JOIN
   pattern). For each resolved market, runs
   the FALLBACK prediction logic and computes
   per-sample Brier. Returns mean Brier.
3. `read_active_train_brier` reads the
   active.json's `best.brier` for the
   train-time baseline.
4. `drift = live_brier - train_brier`. Alert
   fires when `drift > 0.05` AND
   `n_samples >= 10` (avoid small-sample
   noise).
5. `telemetry::Event::ModelDegradation`
   carries all the data; the L1 listener
   optionally fires an OS notification
   (gated by a new Settings pref).

**Why this matters**:
- v0.17a train gives you a Brier number on
  the synthetic train fold. v0.43a backtest
  lets you check on demand. v0.48 is the
  "always-on" check: every hour, the system
  measures how the platform's prediction
  quality is tracking against train-time
  expectations.
- For new installs, the loop ticks emitting
  `n_samples=0` until markets sync and
  start resolving. The L1 user can
  manually trigger via `degradationCheckNow`
  to verify the integration end-to-end.

**v0.48 monitors the FALLBACK model, not a
specific trained model**: reading the active
model's weights requires the v0.50+ CLOB
integration. The detector's purpose is
platform-wide signal drift, not per-model
performance. v0.50+ can extend this to
monitor a user-selected model.

## Files changed in v0.48

```
src-tauri/src/infra/telemetry.rs                | +ModelDegradation event
src-tauri/src/infra/scheduler/mod.rs            | +7th loop + 3 cargo tests
src-tauri/src/commands/scheduler.rs             | +degradation_check_now IPC
src-tauri/src/lib.rs                            | +1 invoke_handler entry
src/ipc.ts                                      | +degradationCheckNow wrapper
src/stores/prefs-store.ts                      | +degradationAlertNotify pref
src/lib/prefs-io.ts                            | +12 fields (was 11)
src/lib/prefs-io.test.ts                       | updated for 12 fields
src/routes/Settings.tsx                        | +DegradationAlertCard
src/routes/Settings.test.tsx                   | +2 tests
src/lib/i18n.ts                                | +4 keys × 2 locales
docs/polyrocket-v0.48-final.md                  | (this file)
docs/overview.md                                | new row in doc-sync table
```

**Net change**: 12 files, +500 LOC.

## Migration / back-compat

- **No DB schema changes.**
- **No new sidecar methods.**
- **No new env vars** (existing defaults: 1h tick,
  0.05 threshold, 50 sample size, all
  env-overridable for power users).
- **prefs imports**: 12 fields now, defaults
  forward-compat for older exports (missing
  field → false / true default).
- **L1↔Tauri guard**: 90 commands (was 89, +1).
  No new "missing wrapper" entries.

## What's next

After user push:
- **v0.50 milestone** — trading-side: real
  order-book feed (so v0.47a placeholders
  become real, and the v0.48 detector can
  monitor a user-selected model), advanced
  order types, conditional orders, fill
  analytics.

The 7-loop scheduler + telemetry + L1
preference system is now mature enough to
power any future "check periodically and
alert" feature. v0.49 candidates (hygiene
/ next small features):
- v0.49: enforce a single source of truth
  for "the active model" (v0.48 reads
  active.json from disk; v0.50+ should
  surface this through a proper IPC)
- v0.49: 5-loop scheduler self-test (verify
  all 7 loops are running on boot)
- v0.49: telemetry retention — currently
  NDJSON to stderr is lost on restart;
  a per-session JSONL file would persist
  events for offline analysis
