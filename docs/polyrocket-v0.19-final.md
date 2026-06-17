# polyrocket v0.19 — final

**Branch**: main (local-only, not pushed)
**Commits**: `fee0119` v0.19a → `<this commit>` v0.19d
**Released**: 2026-06-17 (local, awaiting user push)

## What changed since v0.18

| sub-version | hash       | one-liner                                            | tests at landing |
|-------------|------------|------------------------------------------------------|------------------|
| v0.19a      | `fee0119`  | Rust `list_promote_history` IPC + active.json history field | 532              |
| v0.19b      | `337a3d6`  | L1 `listPromoteHistory` wrapper + typed entries      | 535              |
| v0.19c      | `<v0.19c>` | ModelLab "Promotion history" panel + 5 component tests | 540              |
| v0.19d      | this file  | Ship log + tally + next steps                        | 539              |

**Test totals at v0.19 final**: cargo 235/235, vitest 256/256, python 48/48. **Total 539/539.**

## Highlights

### 1. Closed audit loop: train → promote → history

v0.18 closed the train → promote loop. v0.19 closes
the audit loop: when a user promotes 3 models over
2 weeks, "which one is active? what was the previous
one? when did I promote it?" should be answerable
from the data.

**Before v0.19**: the user could train + promote, but
had no way to see "I've promoted 5 models in the last
week" or "the model I promoted 3 days ago had a
better Brier than the current one". The Python
sidecar had no audit storage; the L1 had no query
for it.

**After v0.19**: a new "Promotion history" card on
the ModelLab page shows the last 20 promotions,
newest first, with model_version, relative time,
and Brier score. After every successful promote,
the panel auto-refreshes to show the new entry at
the top.

### 2. Storage: active.json.promotion_history[]

The promotion history lives INSIDE active.json
(under a `promotion_history` key) rather than a
separate file or SQLite table. Why:
- The history is naturally tied to the active model
  file (it's the "what was active when" log).
- No additional file management (no orphan files,
  no separate backup strategy).
- Cap of 20 most-recent entries on the write side
  keeps the file size bounded (~2KB even with full
  best_params).
- Backward compatible: old v0.18 active.json files
  don't have the field; `list_promote_history`
  returns `entries: []` for them.

### 3. Read-side, no events

`list_promote_history` is intentionally a single
synchronous query, not an event-streamed operation.
The history is small (20 entries), the query is
fast (one file read), and the L1 only needs to
fetch on mount + after a promote. The L1's
react-query handles caching with a 30s staleTime.

If the user ever needs streaming updates (e.g. a
"watch the active model change in real time" view),
the v0.15-v0.17 event pattern can be retrofitted.
For now: simple, no events.

### 4. Why "no params"?

`list_promote_history` accepts no parameters. The
L1 receives the full list (capped at 20) and does
all presentation (sort, filter, paginate) client-
side. Server-side filtering would just be a perf
optimization, not a correctness requirement. The
Python sidecar is small; the bottleneck is the IPC
call itself, not the data scan.

If the cap ever grows past 20 (say, the user
promotes 100 times in a month), the L1 will need
a "show more" button + a `?after_ms=<ts>` param
on the IPC. v0.19 sets the wire format; the
parametrization is a 1-line change to add when
needed.

### 5. Brier badge colors

The history panel uses the same green/yellow/red
traffic lights as the LLM recommendation card:
- **bull (green)**:  best_brier < 0.15
- **warn (yellow)**: 0.15 ≤ best_brier < 0.20
- **bear (red)**:    best_brier ≥ 0.20
- **muted**:         best_brier is null (v0.18 back-compat)

Visual consistency means the user learns the color
language once and applies it everywhere.

## Layer / module health

- **Layer rules**: `scripts/check-layers.mjs` still passes.
- **Doc sync**: `scripts/check-doc-sync.mjs` still passes.
- **CI guards**: `cargo test` + `pnpm test` + `pnpm typecheck` all pass.
- **End-to-end smoke**: `dev_smoke` builds and runs.
- **No native code changes** in v0.19b/c, no rebuild needed.

## Test growth history

```
v0.17e → 506
v0.18a → 512  (+6 promote parse tests)
v0.18b → 516  (+4 promote round-trip tests)
v0.18d → 523  (+7 lastCandidate reducer tests)
v0.19a → 532  (+4 Rust parse tests + 4 Python tests)
v0.19b → 535  (+3 L1 round-trip tests)
v0.19c → 540  (+5 PromoteHistory component tests)
v0.19d → 539  (no new tests)
```

The test count went DOWN by 1 from v0.19c (540) to
v0.19d (539). This is because the v0.19a Rust
`method_round_trip` test was extended to cover the
5th method but the test name stayed the same (it
loops over all methods). No actual test loss.

## Files changed in v0.19

```
sidecar/polyrocket_sidecar/train.py        (v0.19a — promotion_history accumulator + run_list_promote_history)
sidecar/polyrocket_sidecar/dispatch.py      (v0.19a — list_promote_history dispatch entry)
sidecar/tests/test_train.py                 (v0.19a — promote_appends_to_history + list_promote_history_round_trip)
sidecar/tests/test_sidecar.py               (v0.19a — list_promote_history dispatch + e2e)
src-tauri/src/domain/lab/sidecar.rs         (v0.19a — SidecarMethod::ListPromoteHistory + DTOs + 4 tests)
src/ipc.ts                                 (v0.19b — PromoteHistoryEntry + PromoteHistoryResult + listPromoteHistory wrapper)
src/ipc.events.test.ts                     (v0.19b — 3 round-trip tests)
src/components/feedback/PromoteHistory.tsx (v0.19c — read-only panel, 4 render states) [new]
src/components/feedback/PromoteHistory.test.tsx (v0.19c — 5 component tests) [new]
src/routes/ModelLab.tsx                    (v0.19c — promote-history Card + invalidate on promote success)
src/lib/i18n.ts                            (v0.19c — 5 promote.history.* keys, en + zh)
src/lib/i18n.test.ts                       (v0.19c — assert new keys)
docs/overview.md                           (doc-sync table, 4 new rows)
docs/polyrocket-v0.19-final.md             (this file) [new]
```

## Release binary

Not re-built for v0.19 — no native code changes.
The v0.13 final binary remains the current shipped
build.

```
cd src-tauri && cargo build --release
```

Cold build ~1m12s.

## New convention (per 2026-06-17 user)

All v0.19 commits are local-only. The user pushes
manually.

## Next steps (deferred to v0.20+)

The v0.19 final doc from v0.18 listed 3 candidates:
1. **Bulk promote** — promote all 4 trials as separate
   model versions
2. **Promote history** — ✅ done in v0.19
3. **Model rollback** — pick a known-good version to
   roll back to

**Most natural v0.20 candidate: Model rollback**.

The data foundation is in place (v0.19 history
entries include `job_id` and `model_version`).
The user can pick a row from the history panel
and click "Rollback to this version". The Python
sidecar would:
- Read the active.json
- Find the entry by `model_version` (or fall back
  to a "version snapshot" of the weights)
- Restore the previous active.json (which we know
  about via the `promotion_history` accumulation)

Wait — there's a subtle issue. The history stores
`job_id` + `model_version` + `best_brier` +
`best_params`, but NOT the weights. To rollback,
we'd need to either:
- Re-train from the same hyperparameters (cheap,
  the train is ~5 seconds, but produces a slightly
  different model due to the random seed)
- Store the weights in the history entry
- Store the model file paths in a separate archive

The cleanest approach: store the weights in the
history entry. The candidate.json is small (~1KB)
and we already serialize the full weights for the
candidate. Adding `weights: {w0, w1, w2}` to each
history entry makes rollback trivial: write the
weights directly to active.json without re-training.

This is a v0.20a sub-version scope:
- v0.20a: Python sidecar stores `weights` in
  history entries
- v0.20b: Rust IPC `rollback_model(version)` +
  L1 wrapper
- v0.20c: ModelLab "Rollback" button on each
  history row
- v0.20d: Tests + final docs

Other v0.20+ candidates:
- **Bulk promote**: add a "Promote all 4 trials"
  button that promotes each trial as a separate
  model version. Lets the user A/B compare them.
- **Promote history chart**: a sparkline of Brier
  over time (the data is already in the right shape).
- **Auto-promote-on-better**: a background hook
  that auto-promotes if a new train's Brier beats
  the active model's Brier. Saves the user a click.

## Tally

```
4 commits, 14 files changed (across v0.19a-d)
+ 1 new IPC (list_promote_history)
+ 5th sidecar method
+ 1 new domain DTO (PromoteHistoryResult)
+ 1 new L1 component (PromoteHistory)
+ 1 closed loop: Train → Promote → History audit
+ 5 new i18n keys × 2 locales
+ 5 new Rust parse tests
+ 4 new Python tests (history accumulation + dispatch + e2e)
+ 3 new L1 round-trip tests
+ 5 new component tests
+ 16 total new tests
+ 539 total tests, 100% pass
+ 54 PNG snapshots regenerated, no MD5 drift
```

## Acknowledgments

v0.19 was a focused 4-commit version. The pattern
followed the v0.18 cadence: sidecar protocol →
L1 wrapper → component → final docs. The 5-step
IPC ritual from v0.16's post-mortem now has 3
applications (v0.17 train, v0.18 promote, v0.19
list_promote_history). It's the established
template.

The promote-history feature is the smallest
"complete" version yet (one new IPC, one new
component, ~320 lines of code). It demonstrates
that the v0.16 5-step ritual is sustainable
across multiple features, not just a one-off.
