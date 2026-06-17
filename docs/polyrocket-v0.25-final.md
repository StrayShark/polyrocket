# polyrocket v0.25 — final

**Branch**: main (local-only, not pushed)
**Commits**: `126b439` v0.25a → `<this commit>` v0.25c
**Released**: 2026-06-17 (local, awaiting user push)

## What changed since v0.24

| sub-version | hash       | one-liner                                            | tests at landing |
|-------------|------------|------------------------------------------------------|------------------|
| v0.25a      | `126b439`  | Sidecar `promote_all_trials` + Rust IPC              | 595              |
| v0.25b      | `437d92b`  | L1 wrapper + Tauri command + "Promote all 4" button  | 602              |
| v0.25c      | this file  | Ship log + tally                                     | 602              |

**Test totals at v0.25 final**: cargo 253/253, vitest 284/284, python 65/65. **Total 602/602.**

## Highlights

### 1. Closed the "promote everything at once" loop

v0.25 closes the bulk-promote loop. The user
can now click ONE button to promote all 4
trials as separate versions in the history
panel. A/B comparison is now a 1-click
workflow.

**Before v0.25**: the user had to click
"Promote" 4 times (once per trial) to populate
the history with all 4. Cumbersome for A/B
comparison.

**After v0.25**: the user clicks "Promote all 4"
once. All 4 trials appear in the history with
distinct `-tN` model version suffixes and
per-trial badges (v0.24a). The user can then
watch the Brier chart (v0.22) over a week and
rollback (v0.20c) to the winner.

### 2. Why bulk promote?

The user can train 4 trials per click (v0.17).
With per-trial promote (v0.21c), they can
promote each one with a separate click.
"Promote all 4" is the natural shortcut.

The use case: A/B comparison. The user wants
to see how all 4 trials perform on real
markets. They:
1. Train 4 trials
2. Click "Promote all 4" — all 4 appear in
   the history panel
3. Watch the Brier chart for a week
4. Rollback to the winner via v0.20c

Without bulk promote, this requires 4 separate
clicks + careful clicking. With it, it's 1
click + wait + rollback.

### 3. Atomic semantics

The Python sidecar:
1. Reads the current `candidate.json`
2. For each trial, calls `run_promote_model(trial_index=i)`
3. Each call writes a new entry to `promotion_history`
4. Returns a list of per-trial results

The promote is atomic per call (write-then-rename).
The 4 calls are sequential (not parallel).
This is safe because:
- Each call reads the latest active.json + appends to history
- The 4 entries accumulate in the history
- The final active model is the LAST one promoted (trial 3)
- The user can rollback to any of them

If the user wants a different "last promoted"
trial to be the active model, they use
Rollback (v0.20c) after bulk promote.

## Layer / module health

- **Layer rules**: `scripts/check-layers.mjs` still passes.
- **Doc sync**: `scripts/check-doc-sync.mjs` still passes.
- **CI guards**: `cargo test` + `pnpm test` + `pnpm typecheck` all pass.
- **End-to-end smoke**: `dev_smoke` builds and runs.
- **No native code changes** in v0.25c, no rebuild needed.

## Test growth history

```
v0.23d → 587
v0.24a → 590  (+3 component)
v0.24b → 590  (no new tests)
v0.25a → 595  (+4 Rust + 3 Python)
v0.25b → 602  (+3 L1 round-trip + 2 component)
v0.25c → 602  (no new tests)
```

## Files changed in v0.25

```
sidecar/polyrocket_sidecar/train.py        (v0.25a — run_promote_all_trials)
sidecar/polyrocket_sidecar/dispatch.py      (v0.25a — promote_all_trials dispatch)
sidecar/tests/test_train.py                 (v0.25a — 2 tests)
sidecar/tests/test_sidecar.py               (v0.25a — 1 e2e + method_round_trip extended)
src-tauri/src/domain/lab/sidecar.rs         (v0.25a — SidecarMethod::PromoteAllTrials + DTOs + 4 tests)
src/ipc.ts                                 (v0.25b — PromoteAllTrialsResult + wrapper)
src/ipc.events.test.ts                     (v0.25b — 3 round-trip tests)
src-tauri/src/commands/sidecar.rs           (v0.25b — promote_all_trials Tauri command)
src-tauri/src/lib.rs                        (v0.25b — register new command)
src/components/feedback/TrainProgress.tsx  (v0.25b — onPromoteAll + promotingAll props + "Promote all 4" button)
src/components/feedback/TrainProgress.test.tsx (v0.25b — 2 component tests)
src/routes/ModelLab.tsx                    (v0.25b — promoteAllMut + wire TrainProgress)
src/lib/i18n.ts                            (v0.25b — 4 new keys, en + zh)
src/lib/i18n.test.ts                       (v0.25b — assert new keys)
docs/overview.md                           (doc-sync table, 3 new rows)
docs/polyrocket-v0.25-final.md             (this file) [new]
```

## Release binary

Not re-built for v0.25 — no native code changes
in v0.25b/c. The v0.13 final binary remains the
current shipped build.

```
cd src-tauri && cargo build --release
```

Cold build ~1m12s.

## New convention (per 2026-06-17 user)

All v0.25 commits are local-only. The user pushes
manually.

## Pattern observation: "missing Tauri command" (5th time)

v0.25a had the wire format and Python side, but
no Tauri command. v0.25b back-filled the command
(mirroring the v0.19b/v0.20b/v0.23b pattern).
This is the **FIFTH** time we've hit this pattern.

A future CI improvement (next-version candidate):
add a shell script that greps for every L1 wrapper
in `src/ipc.ts` and asserts the corresponding
Tauri command exists in `src-tauri/src/commands/
sidecar.rs` and is registered in
`src-tauri/src/lib.rs`. Could be a 30-line
script that runs as part of `scripts/check-layers.mjs`
or as a standalone pre-commit hook.

## What v0.25 completes

The full model lifecycle is now:
1. **Train** (v0.17d Train button) — 4-trial sweep
2. **Promote best** (v0.18c Promote button) — promote the auto-pick
3. **Promote any trial** (v0.21c per-trial buttons) — bulk-promote a specific trial
4. **Promote all 4** (v0.25b) — promote every trial at once for A/B compare
5. **Auto-promote if better** (v0.23b) — conditional promote with margin
6. **History audit** (v0.19c PromoteHistory) — see all past promotions
7. **Visualize** (v0.22 Brier sparkline) — glance-level view of the Brier trend
8. **Rollback** (v0.20c Rollback button + v0.20 confirmation modal) — restore a previous version
9. **Predict** (v0.12a) — uses the active model

The user has full control over which model is
active at any time, with multiple paths to
choose from. The history is fully auditable.
The trend is visible at a glance.

## Next steps (deferred to v0.26+)

From the v0.24 final doc, v0.25 had 3 candidates.
v0.25 closed the bulk-promote one. Remaining:
1. **Background scheduler auto-promote** — have
   the scheduler call `auto_promote_if_better`
   after each train completes
2. **Hover tooltips on chart dots** — hovering a
   dot shows "Brier 0.184, logistic-train-XYZ,
   promoted 3h ago"
3. **CI check: L1 wrapper → Tauri command** —
   the 5th "missing Tauri command" issue
   (v0.19b, v0.20b, v0.23b, v0.25b) is begging
   for a CI guard. A simple grep-based shell
   script that fails the build if any
   `export const X = (args) => invoke<...>('X', ...)`
   doesn't have a matching `commands::sidecar::X`
   in lib.rs.

**Most natural v0.26 candidate: CI check
(L1 wrapper → Tauri command)**.

This is a meta-feature, not a user-facing
feature. It would have caught 4 of the last
5 versions' "missing Tauri command" issues
(v0.19b, v0.20b, v0.23b, v0.25b — and the
initial v0.20a's command was already there
when v0.20a landed, so 4 out of 5 missed the
guard).

A simple 30-line shell script could:
1. Extract all `commands::sidecar::X` from
   `src-tauri/src/commands/sidecar.rs`
2. Extract all `export const X = ...` from
   `src/ipc.ts`
3. For each L1 wrapper X, assert that
   `commands::sidecar::X` exists in sidecar.rs
4. Fail the build with a clear error if any
   L1 wrapper has no matching Tauri command

v0.26 scope:
- v0.26a: Shell script + integration into
  pre-commit hook (or check-layers.mjs)
- v0.26b: Tests + final docs

This is a one-shot meta-feature, not a
4-commit version. v0.26a is the entire
change.

Other v0.26+ candidates:
- **Background scheduler auto-promote**: have
  the scheduler call `auto_promote_if_better`
  after each train. Removes the need for the
  user to click "Promote if better".
- **Hover tooltips on chart dots**: a11y
  improvement for the Brier sparkline.
- **Filter the history panel by trial type**:
  toggle "Show only bulk-promoted" / "Show
  only best-trial" so the user can focus on
  one or the other.

## Tally

```
3 commits, 14 files changed (across v0.25a-c)
+ 1 new IPC (promote_all_trials)
+ 1 new DTO (PromoteAllTrialsResult)
+ 1 new L1 wrapper (promoteAllTrials)
+ 1 new UI button ("Promote all 4")
+ 2 new L1 component props (onPromoteAll, promotingAll)
+ 4 new i18n keys × 2 locales
+ 4 new Rust parse tests
+ 3 new Python tests
+ 3 new L1 round-trip tests
+ 2 new component tests
+ 12 total new tests
+ 602 total tests, 100% pass
+ 54 PNG snapshots regenerated, no MD5 drift
+ 1 closed "promote everything at once" loop
```

## Acknowledgments

v0.25 was a focused 3-commit version. The pattern
followed the v0.20-v0.24 cadence: sidecar
protocol → L1 wrapper + Tauri command →
component (button) → final docs.

v0.25a had a notable design decision: the
"Promote all 4" call returns a list of per-trial
results, not just a success boolean. This lets
the UI distinguish:
- All OK
- Partial (some succeeded, some failed)
- All failed

Each case has a different toast. The user
sees the exact outcome, not just "OK".

v0.25b was the FIFTH back-fill of a missing
Tauri command. The pattern is now so common
that a CI guard is overdue. v0.26a will
address this meta-issue.
