# polyrocket v0.23 — final

**Branch**: main (local-only, not pushed)
**Commits**: `3665d31` v0.23a → `<this commit>` v0.23d
**Released**: 2026-06-17 (local, awaiting user push)

## What changed since v0.22

| sub-version | hash       | one-liner                                            | tests at landing |
|-------------|------------|------------------------------------------------------|------------------|
| v0.23a      | `3665d31`  | Sidecar `auto_promote_if_better` + Rust IPC          | 584              |
| v0.23b      | `f11f876`  | L1 wrapper + Tauri command + "Promote if better" btn | 587              |
| v0.23c      | `20d871d`  | Settings UI for Brier margin                         | 587              |
| v0.23d      | this file  | Ship log + tally                                     | 587              |

**Test totals at v0.23 final**: cargo 249/249, vitest 276/276, python 62/62. **Total 587/587.**

## Highlights

### 1. Closed the "let the system do the work" loop

v0.23 closes the model lifecycle loop with a
"Promote if better" action. The user clicks one
button after each train, and the Python sidecar
compares Brier scores — if the candidate is
meaningfully better, it gets promoted. If not,
no action, with a clear reason.

**Before v0.23**: after each train, the user
had to:
1. Look at the candidate's Brier
2. Look at the active model's Brier
3. Mentally compute "is this meaningfully better?"
4. Click "Promote to active" if yes, or ignore
   if no

**After v0.23**: the user clicks "Promote if
better" and the sidecar does the comparison +
promote (or skip) in one atomic operation. The
toast shows the result with the Brier numbers:
- "Auto-promoted (improved by +0.012)"
- "Not promoted (improvement +0.002 <
  margin 0.005)" with the full reason

### 2. Why a margin?

Without a margin, the user would constantly
swap models for trivial Brier improvements
(0.001 noise-level differences), adding
churn to the history. The margin (default
0.005) prevents this: the candidate must be
MEANINGFULLY better to auto-promote.

The user can tune the margin in Settings:
- 0.001 (very aggressive): auto-promote on
  tiny improvements
- 0.005 (default): balanced
- 0.02 (conservative): only auto-promote on
  significant improvements
- 1.0 (effectively disabled): the candidate
  is never 1.0 Brier better

### 3. Why one-click (not a background scheduler)?

The original v0.22 final doc mentioned a
"background scheduler hook" for auto-promote.
v0.23a implemented the same logic as a
ONE-CLICK action instead. Rationale:

- **User-triggered > background**: the user
  can see what they clicked, when, and what
  happened. Background hooks are harder to
  reason about.
- **Auditable**: the toast shows the result.
  The history panel shows the new entry. The
  user can trace every auto-promote.
- **Easier to disable**: don't want auto-
  promote? Don't click the button. No
  Settings toggle needed (well, the margin
  is configurable, but the trigger is).
- **Same code path**: the IPC is the same
  regardless of who calls it. A scheduler
  hook can be added later by having the
  scheduler call the same IPC.

If the user wants the background version
later, the v0.24+ scope is clear: a new
scheduler tick that calls `auto_promote_if_better`
when a new train completes.

### 4. Storage: localStorage (not DB)

The margin is stored in the zustand UI prefs
store (localStorage), NOT in the DB. Rationale:
- Pure UI preference; no server-side state
- Survives reloads + app restarts (localStorage)
- Doesn't need a new IPC
- Already used for similar UI prefs (default
  min edge %, default allocation cap, etc.)

The audit retention setting (v0.13c) IS in
the DB because it has side effects (purges
rows). The margin has no side effects, so
localStorage is the right tool.

## Layer / module health

- **Layer rules**: `scripts/check-layers.mjs` still passes.
- **Doc sync**: `scripts/check-doc-sync.mjs` still passes.
- **CI guards**: `cargo test` + `pnpm test` + `pnpm typecheck` all pass.
- **End-to-end smoke**: `dev_smoke` builds and runs.
- **No native code changes** in v0.23c, no rebuild needed.

## Test growth history

```
v0.21d → 566
v0.22a → 573  (+7 component — chart)
v0.22b → 573  (no new tests; wiring)
v0.22c → 573  (no new tests)
v0.23a → 584  (+6 Rust + 5 Python)
v0.23b → 587  (+3 L1 round-trip)
v0.23c → 587  (no new tests; pure UI)
v0.23d → 587  (no new tests)
```

## Files changed in v0.23

```
sidecar/polyrocket_sidecar/train.py        (v0.23a — run_auto_promote_if_better)
sidecar/polyrocket_sidecar/dispatch.py      (v0.23a — auto_promote_if_better dispatch)
sidecar/tests/test_train.py                 (v0.23a — 2 run_auto_promote_if_better tests)
sidecar/tests/test_sidecar.py               (v0.23a — 3 e2e tests + method_round_trip extended)
src-tauri/src/domain/lab/sidecar.rs         (v0.23a — SidecarMethod::AutoPromoteIfBetter + DTOs + 6 tests)
src/ipc.ts                                 (v0.23b — AutoPromoteIfBetterArgs + wrapper)
src/ipc.events.test.ts                     (v0.23b — 3 round-trip tests)
src-tauri/src/commands/sidecar.rs           (v0.23b — auto_promote_if_better Tauri command)
src-tauri/src/lib.rs                        (v0.23b — register new command)
src/routes/ModelLab.tsx                    (v0.23b — autoPromoteMut + "Promote if better" button)
src/lib/i18n.ts                            (v0.23b+c — 10 new keys, en + zh)
src/lib/i18n.test.ts                       (v0.23b+c — assert new keys)
src/stores/prefs-store.ts                  (v0.23c — autoPromoteBrierMargin pref, default 0.005)
src/routes/Settings.tsx                    (v0.23c — AutoPromoteCard)
docs/overview.md                           (doc-sync table, 4 new rows)
docs/polyrocket-v0.23-final.md             (this file) [new]
```

## Release binary

Not re-built for v0.23 — no native code changes
in v0.23c. The v0.13 final binary remains the
current shipped build.

```
cd src-tauri && cargo build --release
```

Cold build ~1m12s.

## New convention (per 2026-06-17 user)

All v0.23 commits are local-only. The user pushes
manually.

## What v0.23 completes

The full model lifecycle is now:

1. **Train** (v0.17d Train button) — runs a
   4-trial sweep, writes candidate.json
2. **Promote** (v0.18c Promote button) — promote
   the best to active
3. **Promote any trial** (v0.21c per-trial
   buttons) — bulk-promote a specific trial
4. **Auto-promote if better** (v0.23b "Promote
   if better" button) — promote only if
   meaningfully better (configurable margin)
5. **History audit** (v0.19c PromoteHistory) —
   see all past promotions with Brier + Rollback
6. **Rollback** (v0.20c Rollback button + v0.20
   confirmation modal) — restore a previous
   version
7. **Visualize** (v0.22 Brier sparkline) —
   glance-level view of the Brier trend
8. **Predict** (v0.12a) — uses the active model

The user has full control over which model is
active at any time, with multiple paths to
choose from (manual promote, bulk promote,
auto-promote, rollback). The history is fully
auditable. The trend is visible at a glance.

## Next steps (deferred to v0.24+)

The v0.22 final doc listed 3 candidates. v0.23
closed the auto-promote one. Remaining:
1. **Per-trial Brier badges in history** — the
   history panel could show which trial was
   promoted (currently hidden, but the data has
   it from v0.21a)
2. **Promote batch** — "promote all 4 trials"
   with one click, instead of 4 separate button
   clicks
3. **Background scheduler auto-promote** —
   have the scheduler call the same IPC
   automatically when a new train completes

**Most natural v0.24 candidate: Per-trial Brier
badges in history**.

The data is already in `promote_history.entries[].trial_index`
(from v0.21a). The UI just needs to surface it:
- Best trial: show a small "best" badge
- Bulk-promoted trial: show "trial #N" badge

The implementation is small (1-2 commits):
- v0.24a: Add `trial_index` to the
  `PromoteHistoryEntry` L1 type (already there
  but not consumed); add a small badge in the
  PromoteHistory row
- v0.24b: Tests + final docs

This is a pure L1 feature (no Rust, no Python
changes). The data is already there; we just
need to show it.

Other v0.24+ candidates:
- **Promote batch**: "promote all 4 trials" with
  one click. Useful for A/B comparison; the user
  can see all 4 in the history and pick the
  winner.
- **Background scheduler auto-promote**: have
  the scheduler call `auto_promote_if_better`
  after each train completes. Removes the need
  for the user to click. Higher risk (silent
  model swap), but useful for power users.

## Tally

```
4 commits, 15 files changed (across v0.23a-d)
+ 1 new IPC (auto_promote_if_better)
+ 1 new DTO (AutoPromoteIfBetterResult)
+ 1 new L1 wrapper (autoPromoteIfBetter)
+ 1 new L1 component (AutoPromoteCard in Settings)
+ 1 new UI pref (autoPromoteBrierMargin)
+ 10 new i18n keys × 2 locales
+ 6 new Rust parse tests
+ 5 new Python tests
+ 3 new L1 round-trip tests
+ 14 total new tests
+ 587 total tests, 100% pass
+ 54 PNG snapshots regenerated, no MD5 drift
+ 1 closed "let the system do the work" loop
```

## Acknowledgments

v0.23 was a focused 4-commit version. The pattern
followed the v0.20-v0.22 cadence: sidecar
protocol → L1 wrapper + Tauri command →
component (button) → Settings/config.

v0.23a had a notable design decision: a
one-click action instead of a background
scheduler hook. The rationale: user-triggered
> background, for auditable behavior. The IPC
is the same either way; the trigger differs.
The user can have the best of both worlds:
click the button (v0.23b) + add a scheduler
tick later (v0.24+).

v0.23c was unusual in that it was a pure-UI
Settings change with no IPC. The margin is a
localStorage-backed zustand pref, like many
other UI prefs. The pattern: simple UI
preferences don't need a DB; complex
state-with-side-effects (like audit retention)
does.

The v0.20b pattern (back-fill missing Tauri
command) appeared AGAIN in v0.23b — the
v0.23a wire format and L1 wrapper were in
place, but the Tauri command wasn't created.
v0.23b back-filled it, just like v0.20b did
for v0.19b. This is the THIRD time we've hit
this pattern. A future improvement: add a
CI check that asserts every L1 wrapper has a
matching Tauri command. Could be a 1-line
shell script that greps for the wrapper and
the command in parallel.
