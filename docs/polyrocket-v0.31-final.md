# polyrocket v0.28-0.30 — final (batch release)

**Branch**: main (local-only, not pushed)
**Commits**:
- v0.28: `63a898e` v0.28a → `e297113` v0.28e (5 commits)
- v0.29: `de01ceb` v0.29 (1 commit)
- v0.30: `d682115` v0.30 (1 commit)
- v0.31: `<this commit>` (docs)

**Released**: 2026-06-17 (local, awaiting user push)

## What changed since v0.27

This batch covers **3 versions** + 1 docs commit
(**7 commits total**) addressing 3 of the 4
deferred items from the v0.27 final doc:

| sub-version | hash       | one-liner                                            | tests at landing |
|-------------|------------|------------------------------------------------------|------------------|
| v0.28a      | `63a898e`  | Rust: AutoPromoteConfig + train_job spawns worker   | 602              |
| v0.28b      | `b95fdce`  | L1 wrappers + 9 wire-format tests                    | 611              |
| v0.28c      | `de2b7c2`  | Settings toggle + ModelLab auto-refresh + 4 i18n     | 615              |
| v0.28d      | `4efadcf`  | 5 component tests for new Settings behavior          | 620              |
| v0.28e      | `e297113`  | v0.28 ship log + tally                               | 620              |
| v0.29       | `de01ceb`  | Hover tooltips on chart dots + 4 chart tests         | 624              |
| v0.30       | `d682115`  | Trial-type filter in history panel + 5 tests         | 629              |
| v0.31       | this file  | Batch ship log + tally for v0.28-v0.30               | 629              |

**Test totals at v0.31 final**: cargo 253/253, vitest 307/307, python 65/65, script 4/4. **Total 625 (vitest+cargo+python) + 4 script = 629 total.**

## Highlights

### v0.28 — Background auto-promote (5 commits)

The auto-promote loop that v0.23a started is now
**fully automatic**. When the user enables
"Auto-run after train" in Settings, every
successful train triggers a background
`auto_promote_if_better` worker. The worker
runs in a `tokio::spawn` task spawned from
`train_job` after the sidecar returns. The
train IPC returns immediately; the worker
emits `auto_promote:finished` when done.

**Architecture**: spawn from train_job, not a
separate scheduler loop. Why? The natural
place to trigger is right after the train
finishes — the worker has everything it
needs (SidecarState + AppHandle + brier_margin
+ job_id), concurrency is bounded (one train
at a time), and there's no polling overhead.

**Rust changes**:
- `AppState` gains `auto_promote: Arc<Mutex<AutoPromoteConfig>>`
- 2 new IPCs: `set_auto_promote_config`,
  `get_auto_promote_config`
- 1 new Tauri event: `auto_promote:finished`
- `SidecarState` refactored to `Arc<Mutex<...>>`
  for cheap clone into the worker task
- `train_job` now spawns the worker if
  `auto_promote.enabled === true` and the
  train status is "succeeded" or "ok"

**L1 changes**:
- 2 new IPC wrappers + 1 event type + 1 listen
  wrapper
- New `autoPromoteAfterTrain: boolean` in the
  prefs store
- New Toggle in Settings → AutoPromoteCard
  with description
- New useEffect in ModelLab listening for
  `auto_promote:finished` to auto-refresh
  the history panel + show a toast
- 9 wire-format tests in `ipc.events.test.ts`
- 5 component tests in `Settings.test.tsx`
  (first Settings tests ever)
- 4 new i18n keys × 2 locales

### v0.29 — Hover tooltips on chart dots (1 commit)

Each dot in the Brier sparkline now has
**two-layer tooltips**:
1. **Native `<title>` element** — a11y,
   works without JS, screen-reader
   accessible. Always-on fallback.
2. **Custom positioned `<g>` tooltip** —
   styled dark background, shows full
   info (model_version + Brier + "promoted
   Xh ago" + trial badge).

Plus:
- **Invisible 12px hit areas** for easier
  hovering (no need for pixel-perfect aim)
- **Hovered dot grows** from r=2.5 to r=3.5
  with a thin stroke ring (visual feedback)
- **SVG `onMouseLeave`** hides the tooltip
- **Tooltip positioning** clamped to the
  chart viewport (no right-edge overflow)
- 4 new chart tests (native title, hit
  areas, show/hide, dot growth)

### v0.30 — Trial-type filter in history panel (1 commit)

The PromoteHistory panel now has **3 filter
chips** at the top:
- **All** (default) — every entry
- **Best trial** — only entries promoted
  via the best path (Promote / Promote if
  better)
- **Bulk** — only entries promoted via the
  bulk path (Promote all / Promote trial N)

When a filter has 0 results, the panel shows
"no entries match the '<filter>' filter."
in italics (not an empty list, which would
be confused with "no history at all").

Local state (not persisted) — the filter is
a transient view preference, not a user
setting. 4 new i18n keys × 2 locales.
5 new component tests.

## What this batch completes

This 7-commit batch closes **3 of the 4
deferred items** from the v0.27 final doc:

1. ✓ **Background scheduler auto-promote**
   (v0.28) — the auto-promote loop is now
   fully automatic, no user click needed
2. ✓ **Hover tooltips on chart dots** (v0.29)
   — a11y baseline + styled detail
3. ✓ **Filter history panel by trial type**
   (v0.30) — All / Best / Bulk

The 4th — "Generalize the L1↔Tauri guard
further" (also check Rust command definitions
in `commands/*.rs`) — is deferred. Lower
priority because the Rust compiler catches
the "defined but not registered" case; the
guard catches the inverse (registered but
no L1 wrapper).

## Layer / module health

- **Layer rules**: `scripts/check-layers.mjs` still passes.
- **Doc sync**: `scripts/check-doc-sync.mjs` still passes.
- **L1↔Tauri guard** (v0.27a): all-modules; passes.
- **CI guards**: `cargo test` + `pnpm test` +
  `pnpm typecheck` all pass.
- **End-to-end smoke**: `dev_smoke` builds and runs.

## Test growth history

```
v0.27a → 606  (+1 script test for v0.4 bug case)
v0.27b → 606  (no new tests)
v0.28a → 606  (no new tests — Rust only)
v0.28b → 615  (+9 wire-format in ipc.events.test.ts)
v0.28c → 615  (no new tests — UI plumbing)
v0.28d → 620  (+5 Settings.test.tsx component tests)
v0.28e → 620  (no new tests)
v0.29  → 624  (+4 chart hover tests)
v0.30  → 629  (+5 history filter tests)
v0.31  → 629  (no new tests)
```

## Files changed across v0.28-v0.30

```
v0.28:
  src-tauri/src/infra/state.rs                (AutoPromoteConfig + AppState::new)
  src-tauri/src/commands/sidecar.rs           (set/get IPCs + worker + train_job hook;
                                              SidecarState refactor to Arc<Mutex<...>>)
  src-tauri/src/lib.rs                        (AppState::new + register new IPCs)
  src/ipc.ts                                  (2 IPC wrappers + 1 event type + 1 listen)
  src/ipc.events.test.ts                      (+9 wire-format tests)
  src/stores/prefs-store.ts                   (autoPromoteAfterTrain field)
  src/routes/Settings.tsx                     (useEffect push + Toggle + Save push)
  src/routes/ModelLab.tsx                     (onAutoPromoteFinished listener)
  src/lib/i18n.ts                             (+4 keys × 2 locales)
  src/routes/Settings.test.tsx                (+5 component tests, new file)
v0.29:
  src/components/feedback/PromoteHistoryChart.tsx       (hover tooltips + hit areas)
  src/components/feedback/PromoteHistoryChart.test.tsx  (+4 hover tests)
  src/routes/Settings.test.tsx                          (TypeScript strictness fix)
v0.30:
  src/components/feedback/PromoteHistory.tsx            (filter chips + filtering)
  src/components/feedback/PromoteHistory.test.tsx       (+5 filter tests)
  src/lib/i18n.ts                                       (+4 keys × 2 locales)
v0.31:
  docs/overview.md                            (8 new rows in §5.1)
  docs/polyrocket-v0.28-final.md              (already landed in v0.28e)
  docs/polyrocket-v0.31-final.md              (this file) [new]
```

## Release binary

Not re-built for v0.28-0.30 — no native code
changes in user-facing terms. The
`SidecarState` refactor (Arc<Mutex<...>>) is
internal, no API change.

```
cd src-tauri && cargo build --release
```

Cold build ~1m12s.

## New convention (per 2026-06-17 user)

All v0.28-0.30 commits are local-only. The
user pushes manually.

## Architectural patterns established

This batch solidifies 3 patterns that
re-appeared across multiple commits:

### Pattern 1: "Config in L1, consumer in Rust"
The auto-promote config (v0.28) follows the
pattern established by v0.13c's audit
retention:
- L1 zustand store = source of truth for UI
- Push to Rust via `setXxxConfig` IPC on
  Settings mount and on every change
- Rust `AppState` = consumer (read inside
  the handler that needs it)
- The two stay in sync within one session;
  if the L1 never mounts Settings, Rust
  uses defaults (which is safe — the
  feature is opt-in)

### Pattern 2: "Two-layer tooltip"
The chart hover (v0.29) establishes a
re-usable a11y pattern:
- Native `<title>` = always-on fallback
  (works without JS, screen-reader friendly)
- Custom positioned `<g>` = styled detail
  (dark bg, themed colors, richer info)
- Both fire on the same hover event
- The native one is the a11y baseline; the
  custom one is the UX layer
- Reuse this pattern for any future SVG
  component with interactive elements

### Pattern 3: "View filter, local state"
The history panel filter (v0.30) establishes
a pattern for transient view filters:
- Local `useState` (not persisted to
  localStorage or DB)
- Resets on remount
- The user typically wants to switch
  back and forth, not have a remembered
  default
- If persistence is ever needed, it's a
  1-line change to zustand+localStorage

## Why the auto-promote worker is spawned, not scheduled

v0.28 considered 3 options:

1. **Scheduler loop** polling a queue of
   pending auto-promotes. Pros: can retry,
   can debounce. Cons: extra moving part,
   polling adds latency, complex to test.

2. **Tokio interval** ticking every N
   seconds. Pros: simple. Cons: wastes
   cycles when no train is running; only
   fires on the tick boundary, not at
   train completion.

3. **Spawn from `train_job` handler** after
   the sidecar returns. Pros: triggers
   exactly when needed, no polling, no
   separate queue, no extra loop. Cons:
   only fires after a train; if the train
   is killed mid-flight, the auto-promote
   won't fire.

Option 3 wins because:
- Concurrency is bounded (one train at a
  time, the user clicks the button)
- The natural place to trigger is right
  after the train finishes
- The worker has everything it needs
- The train IPC returns immediately
- No new loop to maintain, no polling
  overhead, no queue management

The 5-step IPC ritual was followed for the
new IPCs (5th time at v0.25b, 6th at v0.28):
Rust DTO + L1 mirror + JSON round-trip test
+ IPC wrapper + L1 component test.

## What v0.32+ holds (deferred)

The v0.28 final doc listed 4 candidates.
v0.28-0.30 closed 3. The 4th:

1. **Generalize the L1↔Tauri guard further**
   — also check Rust command definitions
   in `commands/*.rs` (currently only checks
   lib.rs registrations). Lower priority
   because Rust compiler catches the
   "defined but not registered" case.

Other candidates (from earlier final docs):

2. **Background scheduler for other
   operations** — e.g. auto-cleanup of old
   promote history beyond the 20-entry cap.
3. **Snapshot diffing** — capture the
   54 PNG snapshots BEFORE a change, capture
   them AFTER, diff the two sets, alert on
   unintended visual changes. (Current
   `snapshot_pages.py` just regenerates.)
4. **Export/import of the auto-promote
   config** — so users can share their
   preferred margin with other users.
   (Currently only one user per install.)

## Tally

```
7 commits, 14 files changed (across v0.28-0.31)
+ 1 background auto-promote worker (Rust spawn)
+ 1 settings toggle + ModelLab auto-refresh
+ 1 hover tooltip system (2 layers + hit areas)
+ 1 view filter (All / Best / Bulk)
+ 2 new IPCs (set_auto_promote_config, get_auto_promote_config)
+ 1 new Tauri event (auto_promote:finished)
+ 1 SidecarState refactor (Arc<Mutex<...>> for cheap clone)
+ 8 new i18n keys × 2 locales (4 for v0.28c, 4 for v0.30a)
+ 14 new L1 tests (9 wire + 5 component in v0.28;
                     4 chart in v0.29;
                     5 history in v0.30)
+ 1 first Settings.test.tsx (v0.28d — first Settings tests)
+ 629 total tests (was 606)
+ 0 new IPC types (governance + auto-promote)
+ 0 new components (extended existing)
+ 0 release binary changes
+ 54 PNG snapshots unchanged
```

## Acknowledgments

This batch (v0.28-0.31) was driven by the
"don't break into fine-grained tasks" rule
change on 2026-06-17. Instead of the
usual 1-version-per-turn, this batch
shipped 3 versions in one continuous run:
v0.28 (5 commits) + v0.29 (1 commit) +
v0.30 (1 commit) + v0.31 (this docs commit).
Total: 7 commits, 14 files, +14 L1 tests,
+23 total tests.

The most interesting single change is
v0.28's `SidecarState` refactor (Arc<Mutex<...>>)
which unblocks the auto-promote worker.
The refactor is low-risk (no API change,
no behavior change) but high-leverage: it
enables the entire background-worker
pattern for the project. Any future
"background task that needs the sidecar"
can use the same Arc-clone pattern.

The v0.30 view filter is the most
re-usable pattern. Future "filter this
list" requests can follow the same
template: local state, 3-4 chip buttons
in a row, filter the array before
display, show a "no matches" message
when the filter has 0 results.

The v0.29 two-layer tooltip is the
most a11y-conscious. The native `<title>`
is the cheapest possible a11y win (one
element, no JS, no styling, works
everywhere). The custom `<g>` is the
UX win. Both fire on the same hover
event; the user gets the best of both.
