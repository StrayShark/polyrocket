# polyrocket v0.49 — final

**Branch**: main (local-only, not pushed)
**Commits**: `5e2a2a8` v0.49a → `c66beaa` v0.49c → `<this commit>` v0.49d
**Released**: 2026-06-18 (local, awaiting user push)

## What changed since v0.48

| sub-version | hash      | one-liner                                              | tests at landing |
|-------------|-----------|--------------------------------------------------------|------------------|
| v0.49a      | `5e2a2a8` | Telemetry file retention + L1 log inventory            | 747              |
| v0.49b      | `1a97f16` | Active model single-source IPC                         | 752              |
| v0.49c      | `c66beaa` | Scheduler self-test on boot + L1 status card           | 754              |
| v0.49d      | this file | Ship log + tally                                       | 754              |

**Test totals at v0.49 final**: cargo 250/250, vitest 367/367, python 77/77, script 31/31. **Total 694 (vitest+cargo+python) + 31 script = 725 total.** (Up from 709 in v0.48 — +3 cargo telemetry + +4 cargo self-test + +3 cargo active_model + +2 vitest telemetry + +3 vitest active_model + +2 vitest self-test + the actual py/script unchanged.)

## Highlights

### v0.49a — telemetry file retention

Before v0.49a, telemetry was NDJSON-to-stderr-only.
Capture worked (`polyrocket 2> telemetry.log`) but
events were lost across restarts. v0.49a adds a
persistent per-session JSONL file alongside stderr.

- `<app_data_dir>/logs/telemetry/session-<start_unix>.jsonl`
- Lazy file creation on first emit
- Retention sweep on startup: deletes session
  files older than `POLYROCKET_TELEMETRY_RETENTION_DAYS`
  (default 14)
- L1 Settings → Telemetry card has a "Session log
  files" sub-section: refresh button, purge button,
  per-file size/mtime, current-session badge

### v0.49b — active model single-source IPC

Before v0.49b, every consumer of "what model is
active?" read `<sidecar model dir>/active.json`
directly. The v0.48a degradation detector had its
own copy of the env-var + path + parse logic. v0.49b
promotes that read path to a single IPC, so:
- L1 always goes through `getActiveModel`
- The Rust degradation loop uses the same helper
- Future consumers (v0.50+ per-model monitor,
  model-comparison card, etc.) don't duplicate
  the path logic

The DTO is forward-compat for v0.50+: `weights`
field is reserved but currently always `null`.
The sidecar doesn't write weights into active.json
today; v0.50+ will.

L1 Settings card shows the active model summary
(version / train Brier / promoted at / source path).

### v0.49c — scheduler self-test on boot

Each of the 8 background scheduler loops now
records its last-tick timestamp into a process-
global atomic. The new `schedulerSelfTestNow` IPC
returns a snapshot:

```
{
  processStartedAtUnix,
  checkedAtUnixMs,
  allHealthy,
  loops: [{name, lastTickUnixMs, ageMs, healthy}]
}
```

`healthy = ageMs <= 3 * expected_interval_ms`.
Loops that have never ticked (still in their
initial stagger sleep) are unhealthy with
`ageMs = null`.

The L1 Settings card renders this as a row of
green/red/yellow dots per loop, auto-polling
every 30s. Click a loop name to see its last
tick timestamp (data-testid attribute).

Why this matters: before v0.49c, the only way
to know if a scheduler loop was stuck was to
`grep -E 'loop.*shutting down' logs/` after the
fact. v0.49c makes liveness observable.

## Files changed in v0.49

```
src-tauri/src/infra/telemetry.rs              | +FileSink + list_telemetry_logs + purge_telemetry_logs
src-tauri/src/commands/telemetry.rs           | NEW — 2 IPCs
src-tauri/src/commands/active_model.rs        | NEW — 1 IPC + read_active_model_from_disk helper
src-tauri/src/commands/scheduler.rs           | +1 IPC (scheduler_self_test_now)
src-tauri/src/commands/mod.rs                 | +2 new submodules
src-tauri/src/lib.rs                          | +4 invoke_handler entries + set_log_dir setup
src-tauri/src/infra/scheduler/mod.rs          | +record_tick/self_test + 7 call sites + 4 tests
src/ipc.ts                                    | +4 L1 wrappers + 2 DTO types
src/stores/prefs-store.ts                     | (unchanged — telemetryEnabled already wired)
src/lib/i18n.ts                               | +18 keys × 2 locales
src/routes/Settings.tsx                       | +2 cards (ActiveModelCard, SchedulerSelfTestCard)
                                               + 1 sub-component (TelemetryLogList)
src/routes/Settings.test.tsx                  | +3 mocks + 5 tests
docs/polyrocket-v0.49-final.md                | (this file)
docs/overview.md                              | new row in doc-sync table
```

**Net change**: 12 files, +1,600 LOC (excluding tests).

## Migration / back-compat

- **No DB schema changes.**
- **No new sidecar methods.**
- **No breaking IPC renames.** All new IPCs are
  additive; existing handlers unchanged.
- **prefs imports**: unchanged — `telemetryEnabled`
  was already wired in v0.42c; the underlying IPC
  existed too, but only `setTelemetryEnabled` /
  `getTelemetryEnabled`. v0.49a adds 2 more.
- **L1↔Tauri guard**: 92 commands (was 90, +2).

## What's next

After user push:

### v0.50 — trading-side milestone (per v0.48 final doc)

v0.49 closes the "telemetry + governance" track.
v0.50 is the trading-side milestone:
- **v0.50a**: order types — limit / market /
  stop-loss — additively flag-based. Existing
  `place_signed_order` extended; new IPC
  `validate_order_args`. Migration path: old
  call sites default to `market`.
- **v0.50b**: post-only enforcement — flag on
  the order; pre-sign validation rejects any
  limit order that would cross the book.
- **v0.50c**: fill analytics — slippage,
  time-to-fill, partial-fill rate. New IPC
  `list_fills_analytics` + Dashboard card.

The real CLOB feed (replacing v0.47a placeholders)
is now out-of-scope for v0.50 — it requires
actual Polymarket API integration. v0.51+.

### v0.49 hygiene: what we left on the table

- **Per-session log file rotation** — currently
  one file per process; for long-running users
  that could be megabytes. v0.50+ could rotate
  hourly.
- **`purge_telemetry_logs` IPC only fires on
  startup + manual button click.** A scheduled
  sweep inside the scheduler would be cleaner.
- **`weights` field on ActiveModel** — currently
  always null. v0.50+ will populate it once the
  sidecar writes weights to active.json.
- **Self-test auto-poll cadence** — currently
  30s. Fine for now; tunable per-deck.
- **`scheduler_status` IPC is unused** — L1
  should surface the next-brief timestamp
  somewhere. v0.50+ candidate.

## Notes for v0.50

The current `place_signed_order` lives in
`commands::bet.rs` and is currently a thin
wrapper around the CLOB sign step. v0.50a will:
1. Extend `PlaceSignedOrderArgs` with `order_type`
   (`Market` | `Limit` | `StopLoss`) and
   `limit_price` / `stop_price` optional fields.
2. Add `validate_order_args()` pure helper that
   rejects invalid combinations (e.g. Limit
   without limit_price, StopLoss below market).
3. Update `paper_fills` to record `order_type`.
4. Update `bets` table with `order_type` column
   (idempotent ALTER TABLE migration).
5. The CLOB submission step (v0.50+ real CLOB
   integration) will honor `order_type`.

The post-only enforcement (v0.50b) requires a
real order book (not the v0.47a 0.5 placeholder),
so it's deferred until v0.51+. For now, v0.50b
just adds the flag and validates that it's only
combined with Limit orders.
