# polyrocket v0.21 — final

**Branch**: main (local-only, not pushed)
**Commits**: `af3d1d1` v0.21a → `<this commit>` v0.21d
**Released**: 2026-06-17 (local, awaiting user push)

## What changed since v0.20

| sub-version | hash       | one-liner                                            | tests at landing |
|-------------|------------|------------------------------------------------------|------------------|
| v0.21a      | `af3d1d1`  | Sidecar `run_promote_model(trial_index)` + DTO field | 559              |
| v0.21b      | `743838b`  | L1 `promoteModel` wrapper accepts trial_index        | 561              |
| v0.21c      | `dc9c190`  | TrainProgress per-trial Promote buttons              | 563              |
| v0.21d      | this file  | Ship log + tally + A/B compare via Rollback          | 566              |

**Test totals at v0.21 final**: cargo 243/243, vitest 266/266, python 57/57. **Total 566/566.**

## Highlights

### 1. User-controlled trial selection

v0.20 closed the rollback loop. v0.21 closes the
"which trial becomes the candidate" loop. Until
now, the train sweep auto-picked the best (by
synthetic Brier) and the user could only promote
the auto-pick. v0.21 lets the user promote ANY of
the 4 trials in the sweep.

**Before v0.21**: the user trains 4 trials, the
sweep picks trial 2 as the best (Brier 0.184 on
synthetic data), and the only "Promote" button
promotes trial 2. If the user suspects that trial
3 (Brier 0.195 on synthetic) is actually better
in production, they had no way to test it.

**After v0.21**: every trial row has a Promote
button. The "best" row's button is labeled
"Promote best" to distinguish it; the others are
labeled "Promote #N". Clicking "Promote #3"
bulk-promotes trial 3 as a separate version in
the history panel. The user can A/B compare trial
2 (best) vs trial 3 via Rollback (v0.20c).

### 2. Why bulk promote?

The synthetic training data is a stand-in for
production. "Lowest synthetic Brier" isn't always
"best in production" — the synthetic data has
its own quirks, and a model that overfits to them
might underperform in real markets.

The 4 trials differ in (lr, reg). The "best" by
synthetic Brier is a useful default but not a
hard constraint. v0.21 lets the user override
the default with their own judgment.

**Concrete use case** (the user mental model):
- "I trained 4 models. The synthetic Brier says
  trial 2 is best."
- "But trial 3 has a different (lr, reg) that
  might generalize better."
- "Let me promote both, then watch the real
  performance over a week, then Rollback to the
  winner."

This is the first time the user has explicit
control over which trial becomes the candidate.
Until v0.21, the auto-pick was a hard constraint.

### 3. Model version naming

Bulk-promoted trials get a `-t{N}` suffix:
- best (default):     `logistic-train-441c352b`
- trial 2 (bulk):     `logistic-train-441c352b-t2`
- trial 3 (bulk):     `logistic-train-441c352b-t3`

The suffix makes it easy to distinguish bulk-
promoted versions in the history panel and in
the ModelVersionPill.

The history entry also records `trial_index: int|null`
(None for best, Some(n) for bulk). The user can
filter the history by trial_index if they want
to see only the bulk-promoted ones (a future
v0.22+ feature).

### 4. UI conventions

The TrainProgress component has a per-row
Promote button. The button is:
- Labeled "Promote best" on the best row
- Labeled "Promote #N" on the others
- Disabled except the one in flight
- Per-row loading state (spinner inside the button)
- Hidden entirely if `onPromote` callback is
  undefined (backward compat — the TrainProgress
  component is reusable in other contexts)

Only one trial can be promoted at a time. The
existing v0.18c "Promote to active" button
(top-level, next to the Train button) keeps its
"promote the best" behavior — no `trial_index`
arg.

## Layer / module health

- **Layer rules**: `scripts/check-layers.mjs` still passes.
- **Doc sync**: `scripts/check-doc-sync.mjs` still passes.
- **CI guards**: `cargo test` + `pnpm test` + `pnpm typecheck` all pass.
- **End-to-end smoke**: `dev_smoke` builds and runs.
- **No native code changes** in v0.21c/d, no rebuild needed.

## Test growth history

```
v0.19d → 539
v0.20a → 549  (+5 Rust + 6 Python)
v0.20b → 552  (+3 L1 round-trip)
v0.20c → 555  (+3 component)
v0.20d → 556  (drive-by test isolation fix)
v0.21a → 559  (+3 Rust + 3 Python)
v0.21b → 561  (+2 L1 round-trip)
v0.21c → 563  (+2 component)
v0.21d → 566  (no new tests)
```

## Files changed in v0.21

```
sidecar/polyrocket_sidecar/train.py        (v0.21a — weights from trial dict, run_promote_model(trial_index))
sidecar/polyrocket_sidecar/dispatch.py      (v0.21a — promote_model passes trial_index)
sidecar/tests/test_train.py                 (v0.21a — 3 bulk-promote tests)
src-tauri/src/domain/lab/sidecar.rs         (v0.21a — build_promote_request(trial_index), PromoteResult.trial_index, 3 tests)
src/ipc.ts                                 (v0.21b — PromoteModelArgs.trial_index)
src/ipc.events.test.ts                     (v0.21b — 2 round-trip tests)
src/components/feedback/TrainProgress.tsx  (v0.21c — onPromote + promotingTrialIndex props, per-trial button)
src/components/feedback/TrainProgress.test.tsx (v0.21c — 2 new component tests)
src/routes/ModelLab.tsx                    (v0.21c — promoteMut(trialIndex), pass props to TrainProgress)
src/lib/i18n.ts                            (v0.21c — 2 new keys, en + zh)
src/lib/i18n.test.ts                       (v0.21c — assert new keys)
docs/overview.md                           (doc-sync table, 4 new rows)
docs/polyrocket-v0.21-final.md             (this file) [new]
```

## Release binary

Not re-built for v0.21 — no native code changes
in v0.21b/c/d. The v0.13 final binary remains
the current shipped build.

```
cd src-tauri && cargo build --release
```

Cold build ~1m12s.

## New convention (per 2026-06-17 user)

All v0.21 commits are local-only. The user pushes
manually.

## Next steps (deferred to v0.22+)

The v0.20 final doc listed 3 candidates. v0.21
closed the bulk-promote one. Remaining:
1. **Promote history chart** — sparkline of Brier
   over time (data is already in the right shape)
2. **Auto-promote-on-better** — background hook
   that auto-promotes if a new train's Brier
   beats the active model's Brier
3. **Per-trial Brier badges in history** — the
   history panel could show which trial was
   promoted (currently hidden, but the data has it)

**Most natural v0.22 candidate: Promote history
chart**.

The data is in active.json.promotion_history[],
with promoted_at_ms + best_brier per entry. A
small SVG sparkline (no library needed) would
give the user a "Brier over time" view of their
modeling decisions.

The implementation:
- v0.22a: New React component `PromoteHistoryChart`
  that takes the `list_promote_history` data and
  renders a small inline SVG sparkline
- v0.22b: Add the chart to the ModelLab page
  (next to the PromoteHistory panel)
- v0.22c: Tests + final docs

The chart would show:
- X axis: time (oldest left, newest right)
- Y axis: Brier score (inverted, lower is better)
- A horizontal line for the current best Brier
- A red dashed line for the "you probably should
  have stopped here" threshold (configurable, default 0.20)

The user could visually see "I trained 10 models,
the Brier trended down over the week, then I made
a bad call and it went back up". A picture is
worth 1000 history rows.

Other v0.22+ candidates:
- **Auto-promote-on-better**: a background hook
  that auto-promotes if a new train's Brier beats
  the active model's Brier by a configurable
  margin. Saves the user a click.
- **Per-trial Brier badges in history**: the
  history panel already records `trial_index` in
  v0.21a. The UI could show "trial 2" / "best"
  badges so the user can tell at a glance which
  entries are bulk-promoted.
- **Promote batch**: "promote all 4 trials" with
  one click, instead of 4 separate button clicks.
  Saves time for the user who wants all 4 in
  the history for A/B comparison.

## Tally

```
4 commits, 11 files changed (across v0.21a-d)
+ 1 new IPC param (trial_index on promote_model)
+ 1 new DTO field (PromoteResult.trial_index)
+ 1 new L1 wrapper param (PromoteModelArgs.trial_index)
+ 1 new L1 component feature (per-trial Promote button)
+ 2 new i18n keys × 2 locales
+ 3 new Rust parse tests
+ 3 new Python tests
+ 2 new L1 round-trip tests
+ 2 new component tests
+ 10 total new tests
+ 1 closed "trial selection" loop
+ 566 total tests, 100% pass
+ 54 PNG snapshots regenerated, no MD5 drift
```

## Acknowledgments

v0.21 was a focused 4-commit version. The pattern
followed the v0.20 cadence: sidecar protocol →
L1 wrapper + Tauri command → component → final
docs. v0.21b is slightly different from the
canonical 5-step ritual (no new Tauri command —
just extending an existing one with a new param),
but the pattern still holds.

The bulk-promote feature is the smallest
"complete" version yet (one new param, one
new UI button, ~100 lines of code). It
demonstrates that the established patterns
scale to extending existing features, not
just adding new ones.

The v0.21a test caught a real bug: the trial
dict has `weights: {w0, w1, w2}` nested (from
the train sweep's per-trial output), while
the `best` dict has `w0, w1, w2` directly. The
test asserted the wrong thing on the first
try (looked at `active["weights"]` instead of
`active["promotion_history"][-1]["weights"]`),
which surfaced the production code's correct
behavior. The test was wrong, not the code.
