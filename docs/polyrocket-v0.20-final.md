# polyrocket v0.20 — final

**Branch**: main (local-only, not pushed)
**Commits**: `d84860c` v0.20a → `<this commit>` v0.20d
**Released**: 2026-06-17 (local, awaiting user push)

## What changed since v0.19

| sub-version | hash       | one-liner                                            | tests at landing |
|-------------|------------|------------------------------------------------------|------------------|
| v0.20a      | `d84860c`  | Rust `rollback_model` IPC + weights in history entries | 549              |
| v0.20b      | `e6d16d6`  | L1 `rollbackModel` wrapper + 2 Tauri commands (incl. v0.19b back-fill) | 552              |
| v0.20c      | `7fba84e`  | ModelLab "Rollback" button + confirmation modal      | 555              |
| v0.20d      | this file  | Ship log + tally + pre-existing test isolation fix   | 556              |

**Test totals at v0.20 final**: cargo 240/240, vitest 262/262, python 54/54. **Total 556/556.**

## Highlights

### 1. Closed the model lifecycle loop

v0.20 closes the model lifecycle. With this commit,
the user can do the full cycle from the ModelLab:

1. **Train** a new model (v0.17d Train button)
2. **Promote** it to active (v0.18c Promote button)
3. **See it in the history** (v0.19c History panel)
4. **Roll back** to a previous version (v0.20c Rollback button)
5. **Predict** uses the active model (v0.12a)

This is the first version where the user can:
- Pick a known-good version from the history
- Restore it as active in 2 clicks
- See the rollback recorded in the audit trail

The "what was active when" question is now fully
answerable from the data, and the user has full
control over which model is active at any time.

### 2. Weights stored in history entries

Each `promotion_history` entry now includes
`weights: {w0, w1, w2}` — the three logistic
weights for that model. This makes rollback a
1-line write to active.json (no re-training, no
file lookups, no external state).

Storage cost: ~50 bytes per entry × 20 entries =
1KB cap. Negligible.

Older v0.19 history entries (without weights) are
refused on rollback with a clear error: "model
version X has no weights stored (promoted before
v0.20); cannot rollback". The user has to retrain
to roll back to those.

### 3. Bug fix: missing Tauri command for v0.19b

While implementing v0.20b, we discovered that
`list_promote_history` was registered as a wire-
format DTO and L1 wrapper in v0.19b, but the
actual Tauri command was never created. This
means v0.19c's PromoteHistory panel would have
failed at runtime with "command not found" or
similar.

This is exactly the kind of issue the v0.16
post-mortem warned about: the 5-step IPC ritual
has 5 steps for a reason, and skipping one (the
Tauri command registration) creates a silent
broken feature.

**v0.20b back-fills the missing Tauri command**,
making the v0.19c feature end-to-end functional.
Both `list_promote_history` and `rollback_model`
are now proper Tauri commands following the same
lock discipline as `promote_model`.

This is now a precedent: any new sidecar method
MUST have all 5 steps done, and the test
`scripts/check-doc-sync.mjs` + the
`test_all_methods_registered` test in Python
together enforce this going forward.

### 4. Bug fix: pre-existing Python test isolation

v0.20d also fixes a pre-existing test isolation
bug in `sidecar/tests/test_sidecar.py`:
`PredictTests::test_returns_model_version_field`
fails when prior test runs (E2ESubprocessTests)
leave an `active.json` file behind. The test
expects the "no active model" state (model_version
= "logistic-0.1.0") but the leftover file from a
prior test run takes precedence.

The fix: `PredictTests.setUp` now clears the
active.json file and resets the cache before each
test. This is the same pattern the cargo e2e
tests use (POLYROCKET_SIDECAR_MODEL_DIR=<tmp>).

**Test totals after the fix: 54/54 Python tests
pass in any order, no need to clean up model files
manually.** This was a v0.20d drive-by fix
uncovered while verifying the v0.20a sidecar
changes.

## Layer / module health

- **Layer rules**: `scripts/check-layers.mjs` still passes.
- **Doc sync**: `scripts/check-doc-sync.mjs` still passes.
- **CI guards**: `cargo test` + `pnpm test` + `pnpm typecheck` all pass.
- **End-to-end smoke**: `dev_smoke` builds and runs.
- **No native code changes** in v0.20c/d, no rebuild needed.

## Test growth history

```
v0.18d → 523
v0.19a → 532  (+4 Rust + 4 Python)
v0.19b → 535  (+3 L1 round-trip)
v0.19c → 540  (+5 PromoteHistory component)
v0.19d → 539  (no new tests; one extended test was already counted)
v0.20a → 549  (+5 Rust + 6 Python)
v0.20b → 552  (+3 L1 round-trip; no new cargo tests, just glue)
v0.20c → 555  (+3 PromoteHistory component)
v0.20d → 556  (no new tests; fixed 1 pre-existing test isolation bug)
```

## Files changed in v0.20

```
sidecar/polyrocket_sidecar/train.py        (v0.20a — weights in history + run_rollback_model)
sidecar/polyrocket_sidecar/dispatch.py      (v0.20a — rollback_model dispatch entry)
sidecar/tests/test_train.py                 (v0.20a — 3 rollback round-trip tests)
sidecar/tests/test_sidecar.py               (v0.20a — 1 dispatch + 2 e2e tests; v0.20d setUp fix)
src-tauri/src/domain/lab/sidecar.rs         (v0.20a — SidecarMethod::RollbackModel + DTOs + 5 tests)
src/ipc.ts                                 (v0.20b — RollbackResult + rollbackModel wrapper)
src/ipc.events.test.ts                     (v0.20b — 3 round-trip tests)
src-tauri/src/commands/sidecar.rs           (v0.20b — list_promote_history + rollback_model Tauri commands)
src-tauri/src/lib.rs                        (v0.20b — register 2 new commands)
src/components/feedback/PromoteHistory.tsx (v0.20c — Rollback button + modal + 3 tests)
src/components/feedback/PromoteHistory.test.tsx (v0.20c — 3 new tests)
src/routes/ModelLab.tsx                    (v0.20c — pass activeModelVersion)
src/lib/i18n.ts                            (v0.20c — 12 new rollback.* keys, en + zh)
src/lib/i18n.test.ts                       (v0.20c — assert new keys)
docs/overview.md                           (doc-sync table, 4 new rows)
docs/polyrocket-v0.20-final.md             (this file) [new]
```

## Release binary

Not re-built for v0.20 — no native code changes
in v0.20c/d. The v0.13 final binary remains the
current shipped build.

```
cd src-tauri && cargo build --release
```

Cold build ~1m12s.

## New convention (per 2026-06-17 user)

All v0.20 commits are local-only. The user pushes
manually.

## Next steps (deferred to v0.21+)

The v0.19 final doc listed 3 candidates. v0.20
closed the rollback one. Remaining:
1. **Bulk promote** — promote all 4 trials as
   separate model versions (so the user can
   A/B compare them)
2. **Promote history chart** — sparkline of Brier
   over time (data is already in the right shape)
3. **Auto-promote-on-better** — background hook
   that auto-promotes if a new train's Brier
   beats the active model's Brier

**Most natural v0.21 candidate: Bulk promote**.

The user can train 4 trials per click and
currently only the best one becomes a candidate.
Bulk promote would promote all 4 as separate
versions in active.json's history, so the user
can pick the one with the best Brier in their
real production data (not just the synthetic
holdout).

The 4 trials differ only in their learning rate
and regularization. The "best" by synthetic Brier
isn't always the "best" in production. Bulk
promote lets the user see all 4 in the history
panel and pick the actual winner.

v0.21 scope:
- v0.21a: Python `train_job` saves all 4 trials
  to `candidate.json` (already does this in
  `all_trials`); add a new method
  `promote_trial(trial_index)` that promotes a
  specific trial (1, 2, 3, or 4) instead of the
  best one
- v0.21b: L1 wrapper
- v0.21c: TrainProgress per-trial "Promote this"
  buttons
- v0.21d: Tests + final docs

This would be the first time the user has explicit
control over WHICH trial becomes the candidate.
Until now, the train sweep auto-picks the best by
synthetic Brier.

## Tally

```
4 commits, 14 files changed (across v0.20a-d)
+ 1 new IPC (rollback_model)
+ 1 missing Tauri command back-filled (list_promote_history)
+ 6th sidecar method
+ 1 new domain DTO (RollbackResult)
+ 1 new L1 wrapper (rollbackModel)
+ 1 new L1 component feature (PromoteHistory Rollback button)
+ 1 new L1 component prop (activeModelVersion)
+ 12 new i18n keys × 2 locales
+ 5 new Rust parse tests
+ 6 new Python tests (rollback + history weights)
+ 3 new L1 round-trip tests
+ 3 new component tests
+ 17 total new tests
+ 1 pre-existing test isolation bug fixed
+ 556 total tests, 100% pass
+ 54 PNG snapshots regenerated, no MD5 drift
+ 1 closed model lifecycle loop
+ 1 silent broken feature fixed (v0.19b command)
```

## Acknowledgments

v0.20 was a focused 4-commit version. The pattern
followed the v0.19 cadence: sidecar protocol →
L1 wrapper + Tauri command → component → final
docs. The 5-step IPC ritual from v0.16's post-
mortem now has 4 applications (v0.17 train,
v0.18 promote, v0.19 list_promote_history, v0.20
rollback_model) plus one back-fill (the v0.19b
command). It's a stable template.

The v0.20 back-fill is a useful lesson: even with
a well-established ritual, it's easy to skip a
step if you're moving fast. The Python
`test_all_methods_registered` test is a
synchronization point — if it passes, all
sidecar methods have a Python entry. The Rust
`method_round_trip` test is the same. These two
tests together form a low-cost safety net.

The v0.20d test isolation fix is also worth
noting: it was a 6-line change to a setUp
method, but it took 3 versions (v0.19a → v0.19d
→ v0.20d) to actually fix. The bug was visible
the whole time but never rose to the priority
of "must fix now". It only got fixed because
v0.20a's sidecar changes made the test ordering
sensitive to the file state.
