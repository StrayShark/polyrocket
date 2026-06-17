# polyrocket v0.17 — final

**Branch**: main (local-only, not pushed)
**Commits**: `42610c1` v0.17a → `8ecd5df` v0.17d
**Released**: 2026-06-17 (local, awaiting user push)

## What changed since v0.16

| sub-version | hash       | one-liner                                          | tests at landing |
|-------------|------------|----------------------------------------------------|------------------|
| v0.17a      | `42610c1`  | Rust `train_job` IPC + 2 progress events           | 494              |
| v0.17b      | `5316643`  | L1 train types + listen wrappers                   | 498              |
| v0.17c      | `75ebcfc`  | `TrainProgress` component (per-trial table)        | 498              |
| v0.17d      | `8ecd5df`  | ModelLab — Train button + live progress panel      | 498              |
| v0.17e      | this file  | 8 component tests + ship log                        | 506              |

**Test totals at v0.17 final**: cargo 225/225, vitest 237/237, python 44/44. **Total 506/506.**

## Highlights

### 1. The 13-version-old "Trigger a training job from Settings → Model Lab in v0.4.1" placeholder is gone

The ModelLab page has had a placeholder empty state since v0.4
(13 sub-versions ago):

> "No training runs yet / Trigger a training job from Settings
> → Model Lab in v0.4.1."

This commit replaces it with a real **Train** button + a
live progress panel that shows the 4-trial sweep as it runs.

The user can now:
- Click "Train" → 4-trial hyperparameter sweep runs (2-30s)
- Watch each trial's Brier score populate in a live table
- See the best trial highlighted in green
- Read the best Brier + the winning weights (w0, w1, w2)
- See the absolute path of the candidate.json the sidecar wrote
- Get a toast on completion: "Training completed — Best Brier: 0.184"

The new "Promote to active" button is a natural follow-up
but not part of v0.17 (it would need its own progress UI too).

### 2. The same v0.15a-d pattern, but for `train_job`

v0.17 follows the v0.15 (LLM analyze progress) pattern:

| v0.15 (LLM analyze)                  | v0.17 (train_job)             |
|--------------------------------------|-------------------------------|
| 4 events: started / provider_done /  | 2 events: started / finished  |
|   consensus_done / finished          |                               |
| Per-provider status grid             | Per-trial table               |
| `AnalyzeProgress` component          | `TrainProgress` component     |
| L1 listen wrappers + typed payloads   | L1 listen wrappers + typed    |
| Component tests with mocked events    | Component tests with mocked    |

The key difference: `train_job` runs 4 trials SEQUENTIALLY
inside a single Python call. The stdio protocol (one
request, one response) doesn't allow per-trial events. So
v0.17 has just `started` and `finished` — no per-trial events.
The Python sidecar returns the full trial array in the final
response, which the L1 renders in a static table.

This means the L1 sees a single "Training…" pill during the
run, then the per-trial table when finished. Same UX as v0.15
minus the per-provider live updates.

### 3. The chicken-and-egg strikes again

Same as v0.15c (Analysis page) and v0.15d, the
`useRef<boolean>` flag pattern solves the "events fire before
IPC returns" problem:

```ts
// ModelLab page
expectedTrainRef.current = true;  // user clicks Train
trainMut.mutate();
// ...later, the next `train_job:started` event fires
// and the useEffect captures the job_id:
onTrainStarted((e) => {
  if (expectedTrainRef.current) {
    setActiveTrainJobId(e.job_id);
    expectedTrainRef.current = false;
  }
});
```

The Rust `train_job` command (v0.17a) generates the UUID
internally and emits `train_job:started` BEFORE returning.
By the time `trainMut.onSuccess` fires, the train is already
done. The flag pattern lets the page stay in sync without a
Rust-side refactor.

### 4. Pattern reuse: 3 events, 3 components

We now have a well-established pattern for "long-running IPC
with progress events":

| event       | fired when              | when events fire vs IPC return |
|-------------|-------------------------|-------------------------------|
| `started`   | Rust command starts    | BEFORE IPC returns            |
| `finished`  | Rust command completes | BEFORE IPC returns            |
| optional per-N events | Each sub-task | mixed timing               |

The L1 side always:
1. Subscribes via a global listener (set up on mount).
2. Uses a `useRef<boolean>` flag to capture the in-flight
   id from the next `started` event.
3. Renders a `*Progress` component (AnalyzeProgress,
   TrainProgress, etc.) that subscribes to the same events
   and updates the UI in real time.
4. On IPC return (in onSuccess), just toasts the result +
   invalidates dependent queries.

The v0.17c `TrainProgress` component reuses the
v0.15c `AnalyzeProgress` patterns (subscribedRef, cancelled
flag, data-testid attributes, per-event state updates).

## Layer / module health

- **Layer rules**: `scripts/check-layers.mjs` still passes.
- **Doc sync**: `scripts/check-doc-sync.mjs` still passes.
- **CI guards**: `cargo test` + `pnpm test` + `pnpm typecheck` all pass.
- **End-to-end smoke**: `dev_smoke` builds and runs.
- **No native code changes**, no rebuild needed.

## Test growth history

```
v0.16c → 491
v0.17a → 494  (+3 train_progress serde tests)
v0.17b → 498  (+4 train event round-trip tests)
v0.17c → 498  (no new tests)
v0.17d → 498  (no new tests)
v0.17e → 506  (+8 TrainProgress component tests)
```

## Files changed in v0.17

```
src-tauri/src/domain/lab/sidecar.rs          (v0.17a — build_train_request + TrainResult + parse_train_response)
src-tauri/src/domain/lab/train_progress.rs  (v0.17a — 2 event payloads + 3 tests) [new]
src-tauri/src/domain/lab/mod.rs               (v0.17a — register train_progress submodule)
src-tauri/src/commands/sidecar.rs            (v0.17a — train_job IPC command + 2 emit calls)
src-tauri/src/lib.rs                          (v0.17a — register train_job IPC)
src/ipc.ts                                   (v0.17b — 3 event types + trainJob wrapper + 2 listen wrappers)
src/ipc.events.test.ts                       (v0.17b — 4 round-trip tests)
src/components/feedback/TrainProgress.tsx    (v0.17c — new component)
src/components/feedback/TrainProgress.test.tsx (v0.17e — 8 component tests) [new]
src/routes/ModelLab.tsx                      (v0.17d — Train button + trainMut + listener)
src/lib/i18n.ts                              (v0.17c — 10 train.progress.* keys; v0.17d — 6 modellab.runs.train + train.toast.*)
src/lib/i18n.test.ts                         (v0.17c/d — assert new keys)
docs/overview.md                             (doc-sync table, 4 new rows)
docs/polyrocket-v0.17-final.md               (this file) [new]
```

## Release binary

Not re-built for v0.17 — no native code changes. The
v0.13 final binary remains the current shipped build.

```
cd src-tauri && cargo build --release
```

Cold build ~1m12s.

## New convention (per 2026-06-17 user)

All v0.17 commits are local-only. The user pushes
manually. Saved to `~/.mavis/memory/user.md`.

## Next steps (deferred to v0.18+)

From the v0.13 final doc's deferred list:

1. **Promote to active button** — natural follow-up to
   v0.17. After the user sees a successful train, a
   "Promote" button next to the candidate path that
   calls `promote_model` and refreshes the active model
   pill. Same event pattern.
2. **Parallel predict A/B test** — needs a rethink because
   the I/O bottleneck (single sidecar process) prevents
   true parallelism. v0.13d's `predict_async` + v0.15
   event pattern + v0.16 DTO discipline + v0.17 train
   pattern are the building blocks.
3. **Real `rs-clob-client` integration** — wire the real
   Polymarket CLOB client (currently orders are signed-
   order stubs).
4. **On-chain mirror execution** — currently the mirror
   executor records decisions but doesn't actually send
   transactions.
5. **Code-sign + DMG** — notarized `.dmg` for distribution.
6. **Auto-update feed** — Tauri updater pointed at GitHub
   Releases.
7. **Visual regression** — Playwright with the 54 PNG
   snapshots as baselines.
8. **numpy vectorize predict** — replace the hot-path
   Python loop with a numpy vectorized implementation.

The v0.17d `Train button` + `TrainProgress` + `trainMut`
pattern is a template for v0.18 #1 (`Promote to active`).
The v0.17c `TrainProgress` component is a template for
future progress UIs (auto-update progress, code-sign
progress, etc.).

## Open question

The v0.16c DTO tests added coverage for `LlmAnalysis`,
`LlmRecommendation`, `LlmCallLog`,
`RecordLlmDecisionArgs`. The v0.17b round-trip tests
add coverage for `TrainStartedEvent`, `TrainTrialDto`,
`TrainFinishedEvent`, `TrainResult`.

There's still no dedicated test file for the
`TrainResult` IPC return value (only event-payload
round-trips). v0.18 should add a test for the
`parse_train_response` Rust function — same way v0.16c
documented the DTO shape.

## Tally

```
5 commits, 12 files changed (across v0.17a-d)
+ 1 new doc
+ 1 new module (domain::lab::train_progress)
+ 1 new component (TrainProgress)
+ 1 new IPC (train_job)
+ 2 new Tauri events
+ 16 new i18n keys × 2 locales
+ 15 new tests (3 serde + 4 round-trip + 8 component)
+ 506 total tests, 100% pass
+ 54 PNG snapshots regenerated, no MD5 drift
+ 1 13-version-old placeholder removed
```
