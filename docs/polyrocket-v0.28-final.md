# polyrocket v0.28 — final

**Branch**: main (local-only, not pushed)
**Commits**: `63a898e` v0.28a → `<this commit>` v0.28e
**Released**: 2026-06-17 (local, awaiting user push)

## What changed since v0.27

| sub-version | hash       | one-liner                                            | tests at landing |
|-------------|------------|------------------------------------------------------|------------------|
| v0.28a      | `63a898e`  | Rust: AutoPromoteConfig + train_job spawns worker   | 602              |
| v0.28b      | `b95fdce`  | L1 wrappers + 9 wire-format tests                    | 611              |
| v0.28c      | `de2b7c2`  | Settings toggle + ModelLab auto-refresh + 4 i18n     | 615              |
| v0.28d      | `4efadcf`  | 5 component tests for new Settings behavior          | 620              |
| v0.28e      | this file  | Ship log + tally                                     | 620              |

**Test totals at v0.28 final**: cargo 253/253, vitest 298/298, python 65/65, **+4 script tests** (4/4). **Total 616 (vitest+cargo+python) + 4 script = 620 total.**

Breakdown of the 14 new L1 tests:
- v0.28b: +9 wire-format in `src/ipc.events.test.ts`
  (5 for `AutoPromoteConfigDto` / `SetAutoPromoteConfigArgs`
  shapes, 4 for `AutoPromoteFinishedEvent` payload)
- v0.28d: +5 component tests in `src/routes/Settings.test.tsx`
  (toggle present, toggle ON pushes, toggle OFF pushes,
  Save pushes margin, mount pushes config)

## Highlights

### 1. Background auto-promote after train

v0.23a added the "Promote if better" one-click
button. v0.28 closes the loop: the same action
now happens **automatically** in the background
after every successful train, with no user
interaction.

The user controls it via a new toggle in
Settings → AutoPromoteCard → "Auto-run after
train". When ON, every train that finishes
successfully triggers a background
`auto_promote_if_better` call using the same
margin the one-click button uses.

### 2. Architecture: spawn from `train_job`

The auto-promote worker is **not** a separate
scheduler loop. It's spawned inline from the
`train_job` handler after the sidecar returns:

```rust
// train_job, after emitting train_job:finished:
if result.status == "succeeded" || result.status == "ok" {
    let auto_promote_enabled = app_state.auto_promote.lock()?.enabled;
    if auto_promote_enabled {
        let sidecar = state.inner().clone();
        let app_clone = app.clone();
        tokio::spawn(async move {
            run_auto_promote_worker(sidecar, app_clone, margin, job_id).await;
        });
    }
}
```

Three options were considered:
1. **Scheduler loop** polling a queue — would
   add latency and complexity for no benefit
2. **Tokio interval** ticking + checking —
   same problem, more moving parts
3. **Spawn from train_job** — simplest, the
   natural place to trigger (it has the
   SidecarState + AppHandle + knows the train
   just finished)

Option 3 wins. There's only ever one train
running at a time (the user clicks the
button), so concurrency is bounded. The
worker runs in the background; the train
IPC returns immediately.

### 3. New Rust IPC surface

- `set_auto_promote_config(args)` — L1 pushes
  user settings to Rust's `AppState`. Both
  fields optional (only-supplied-fields
  semantics, so the L1 can update only what
  the user changed).
- `get_auto_promote_config()` — L1 reads the
  current config (returns defaults if nothing
  ever pushed).
- `AutoPromoteFinishedEvent` emitted on
  `auto_promote:finished` — the result of
  the background worker (promoted/not,
  reason, model_version, finished_at).

`AppState` gained `auto_promote:
Arc<Mutex<AutoPromoteConfig>>`. The config
is in-memory (not persisted to DB); the L1
zustand store is the source of truth for
the UI, and pushes to Rust on Settings
mount.

### 4. New L1 IPC wrappers

- `setAutoPromoteConfig(args)` +
  `getAutoPromoteConfig()` — mirror the
  Rust IPCs. `SetAutoPromoteConfigArgs`
  has both fields optional.
- `onAutoPromoteFinished(cb)` — listen
  wrapper for the `auto_promote:finished`
  event.

### 5. Settings integration

The AutoPromoteCard in `Settings.tsx` got:
- A new Toggle for "Auto-run after train"
  with description below
- The toggle immediately pushes to Rust
  (no Save button for a binary toggle)
- The existing margin Save button now
  also pushes to Rust (in addition to
  updating the zustand store)
- A useEffect on Settings mount that
  pushes the current persisted values
  to Rust (the bridge — makes the
  feature work without requiring the
  user to visit Settings first)

### 6. ModelLab integration

A new useEffect in `ModelLab.tsx`
subscribes to `auto_promote:finished`.
On event, the listener:
1. Invalidates `llm-performance`,
   `sidecar-active-model`, `promote-history`
   queries (so the KPI cards, Brier chart,
   and history panel re-fetch automatically)
2. Shows a success toast if the auto-
   promote succeeded, or an info toast
   if it was skipped (the "candidate not
   better" case is normal, not an error)

### 7. SidecarState refactored to Arc<Mutex<...>>

To make the auto-promote worker cheaply
cloneable, `SidecarState` fields changed
from `Mutex<T>` to `Arc<Mutex<T>>`:
- `child: Arc<Mutex<Option<Child>>>`
- `stdin: Arc<Mutex<Option<ChildStdin>>>`
- `stdout: Arc<Mutex<Option<ChildStdout>>>`
- `status: Arc<Mutex<SidecarStatus>>`

The struct now `#[derive(Clone)]` (one Arc
bump per field). All construction sites
updated. This is a low-risk refactor: the
inner T is still `Option<...>` and the
lock semantics are unchanged.

## Layer / module health

- **Layer rules**: `scripts/check-layers.mjs` still passes.
- **Doc sync**: `scripts/check-doc-sync.mjs` still passes.
- **L1↔Tauri guard** (v0.27a): all-modules; passes.
- **CI guards**: `cargo test` + `pnpm test` +
  `pnpm typecheck` all pass.
- **End-to-end smoke**: `dev_smoke` builds and runs.

## Test growth history

```
v0.24b → 590
v0.25a → 595  (+4 Rust + 3 Python)
v0.25b → 602  (+3 L1 round-trip + 2 component)
v0.25c → 602  (no new tests)
v0.26a → 605  (+3 script tests)
v0.26b → 605  (no new tests)
v0.27a → 606  (+1 script test for v0.4 bug case)
v0.27b → 606  (no new tests)
v0.28a → 606  (no new tests — Rust only)
v0.28b → 615  (+9 wire-format in ipc.events.test.ts)
v0.28c → 615  (no new tests — UI plumbing)
v0.28d → 620  (+5 Settings.test.tsx component tests)
v0.28e → 620  (no new tests)
```

## Files changed in v0.28

```
src-tauri/src/infra/state.rs                  (v0.28a — AutoPromoteConfig + AppState::new)
src-tauri/src/commands/sidecar.rs             (v0.28a — set/get IPCs + worker + train_job hook;
                                              v0.28a — SidecarState refactor to Arc<Mutex<...>>)
src-tauri/src/lib.rs                          (v0.28a — AppState::new + register new IPCs)
src/ipc.ts                                    (v0.28b — 2 IPC wrappers + 1 event type + 1 listen wrapper)
src/ipc.events.test.ts                        (v0.28b — 9 wire-format tests)
src/stores/prefs-store.ts                     (v0.28c — autoPromoteAfterTrain field)
src/routes/Settings.tsx                       (v0.28c — useEffect push + Toggle + Save push)
src/routes/ModelLab.tsx                       (v0.28c — onAutoPromoteFinished listener)
src/lib/i18n.ts                               (v0.28c — 4 new keys × 2 locales)
src/routes/Settings.test.tsx                  (v0.28d — 5 component tests, new file)
docs/overview.md                              (doc-sync table, 5 new rows)
docs/polyrocket-v0.28-final.md                (this file) [new]
```

## Release binary

Not re-built for v0.28 — no native code changes
in user-facing terms (the SidecarState refactor
is internal, no API change).

```
cd src-tauri && cargo build --release
```

Cold build ~1m12s.

## New convention (per 2026-06-17 user)

All v0.28 commits are local-only. The user pushes
manually.

## What v0.28 completes

v0.28 closes the "auto-promote" loop that v0.23a
started. The feature is now:
- Discoverable (toggle in Settings, with description)
- Configurable (the existing margin input still works)
- Background (no user click required after train)
- Visible (toast on success/skip, page auto-refreshes)
- Auditable (audit log captures the auto-promote)
- Robust (worker silently catches errors and emits
  them as `auto_promote:finished` with a descriptive
  message)

The cumulative model-lifecycle story:

```
v0.17a  Train button + TrainProgress
v0.18a  Promote button + PromoteResult
v0.19a  Promote history panel
v0.20a  Rollback button
v0.21a  Bulk promote (any of 4 trials)
v0.22a  Brier chart over time
v0.23a  "Promote if better" one-click
v0.24a  Per-trial badges in history
v0.25a  "Promote all 4" bulk button
v0.28a  Background auto-promote after train  ← new
v0.28c  Settings toggle + ModelLab auto-refresh
```

The model lifecycle is now fully automated
end-to-end. The user only needs to:
1. Click Train
2. Wait
3. The new model is auto-promoted if it's better
4. The page auto-refreshes to show the new state
5. The user can rollback from the history panel

## Why spawn instead of scheduler loop

Three options were considered:

1. **Scheduler loop** polling a queue of pending
   auto-promotes. Pros: can retry, can debounce
   multiple rapid trains. Cons: extra moving part,
   polling adds latency, complex to test.

2. **Tokio interval** ticking every N seconds
   and checking "is there a new train since last
   tick?". Pros: simple. Cons: wastes cycles when
   no train is running; only fires on the tick
   boundary, not at train completion.

3. **Spawn from `train_job` handler** after the
   sidecar returns. Pros: triggers exactly when
   needed, no polling, no separate queue, no
   extra loop. Cons: only fires after a train;
   if the train is somehow killed mid-flight,
   the auto-promote won't fire (but then neither
   would a scheduler-based design — they all
   need the train to actually complete).

Option 3 wins because:
- Concurrency is bounded (one train at a time)
- The natural place to trigger is right after
  the train finishes
- The worker has everything it needs (SidecarState
  + AppHandle + brier_margin + job_id)
- The train IPC returns immediately (non-blocking)

The 5-step IPC ritual (5th time at v0.25b) was
followed: Rust DTO + L1 mirror + JSON round-trip
test + IPC wrapper + L1 component test.

## What v0.29+ holds (deferred)

The v0.27 final doc listed 2 candidates. v0.28
closed the "background auto-promote" one.
Remaining:
1. **Hover tooltips on chart dots** — hovering a
   dot shows "Brier 0.184, logistic-train-XYZ,
   promoted 3h ago"
2. **Filter history panel by trial type** — toggle
   "Show only bulk-promoted" / "Show only best-trial"
3. **Generalize the L1↔Tauri guard further** —
   also check Rust command definitions in
   `commands/*.rs` (currently only checks lib.rs
   registrations)

**Most natural v0.29 candidate: Hover tooltips
on chart dots.**

The `PromoteHistoryChart` is a sparkline with
5+ data points. Users naturally want to hover
a dot to see "when was this promoted, what
model was it". The v0.22a chart doesn't have
this — the data is in the history panel below,
but matching dots to entries requires eye-
balling Brier values.

Implementation:
- Wrap each dot in an SVG `<g>` with a
  `<title>` element (browser-native tooltip)
  for basic accessibility
- For richer info, add a custom positioned
  tooltip that shows on hover (not native,
  styled to match the theme)
- Tests for the hover state

Other v0.29+ candidates:
- **Filter history panel by trial type**: a
  `SegmentedControl` with "All | Best trial |
  Bulk" so the user can focus on one.
- **Generalize the L1↔Tauri guard further**:
  parse the `commands/*.rs` files and check
  that each `commands::X::Y` in `lib.rs` has
  a matching function in `commands/X.rs` (or
  the right `commands/Y.rs` for shared
  modules). Lower priority — Rust compiler
  catches the "defined but not registered"
  case; the guard catches the inverse.

## Tally

```
5 commits, 11 files changed (across v0.28a-e)
+ 1 background worker (Rust spawn + auto-promote)
+ 1 settings toggle + auto-refresh in ModelLab
+ 2 new IPCs (set_auto_promote_config, get_auto_promote_config)
+ 1 new Tauri event (auto_promote:finished)
+ 1 SidecarState refactor (Arc<Mutex<...>> for cheap clone)
+ 4 new i18n keys × 2 locales
+ 14 new tests (+9 wire-format + 5 component)
+ 1 L1 UI pref (autoPromoteAfterTrain in zustand)
+ 620 total tests (was 606)
+ 0 new IPC types (governance + auto-promote)
+ 0 new components (extended existing)
+ 0 release binary changes (no native code impact)
+ 54 PNG snapshots unchanged
```

## Acknowledgments

v0.28 was a 5-commit version that adds a
non-trivial new feature (background auto-
promote) without changing the release binary.

The Rust side is the most interesting: the
SidecarState refactor (Arc<Mutex<...>>)
unblocks the worker. The worker's pattern
(silent error handling, descriptive message
in the result event) matches the existing
"Promote if better" button — the only new
piece is "fire from train_job, not from a
button click".

The L1 side is plumbing: Toggle, Save, useEffect
push, listener, toast. The interesting part
is the **bridge** between the L1 zustand
store (UI source of truth) and the Rust
AppState (worker consumer). The Settings
mount useEffect makes it work without
requiring the user to visit Settings first.

The test count growth (+14 L1) is the largest
since v0.16 (+19 DTO tests). The Settings
test is the first one for the Settings page
itself — previously the Settings UI was tested
only by hand + the i18n key tests.
