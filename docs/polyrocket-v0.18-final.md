# polyrocket v0.18 — final

**Branch**: main (local-only, not pushed)
**Commits**: `5d1a07f` v0.18a → `6e99c6c` v0.18c
**Released**: 2026-06-17 (local, awaiting user push)

## What changed since v0.17

| sub-version | hash       | one-liner                                          | tests at landing |
|-------------|------------|----------------------------------------------------|------------------|
| v0.18a      | `5d1a07f`  | Rust `promote_model` IPC + `PromoteResult` DTO     | 512              |
| v0.18b      | `18c7670`  | L1 `promoteModel` wrapper + types + 4 tests         | 516              |
| v0.18c      | `6e99c6c`  | ModelLab — Promote button + race-condition guard   | 516              |
| v0.18d      | this file  | 7 reducer tests + ship log + tally                 | 523              |

**Test totals at v0.18 final**: cargo 231/231, vitest 248/248, python 44/44. **Total 523/523.**

## Highlights

### 1. Promote to active button — closed loop

v0.17 added the Train button (which writes a candidate.json).
v0.18 closes the loop: the user can now promote that
candidate to the active slot with a single click.

**Before v0.18**: the user trains a model, sees the
candidate.json path in the TrainProgress panel, and...
nothing. The candidate sits there. The Python sidecar
had a `promote_model` method but no L2 IPC command
wrapped it. The user would have to SSH into the box
and `mv candidate.json active.json` manually.

**After v0.18**: the same TrainProgress panel now shows
a "last-candidate hint" with the job_id + Brier, and
a new "Promote to active" button in the card's action
slot. One click promotes; ~10ms later the toast says
"Model promoted / Now active: logistic-train-441c352b"
and the `ModelVersionPill` at the top of the page
updates to the new version.

### 2. Race-condition protection via job_id

The promote call passes the candidate's `job_id`:

```ts
promoteModel(lastCandidate ? { job_id: lastCandidate.jobId } : {})
```

The Python sidecar's `run_promote_model` accepts this
optional `job_id` and refuses to promote a candidate
from a different job:

```python
if job_id is not None and candidate.get("job_id") != job_id:
    return {"promoted": False, ...}
```

This protects against a subtle race: the user clicks
Promote, but in the meantime a second train finishes
and overwrites the candidate. Without the `job_id`
check, the promote would promote the new candidate
(not the one the user intended). With it, the
Python sidecar refuses and returns a clear error.

The `lastCandidate` state in v0.18c captures the
job_id at train time and passes it to promote.
This is the kind of small-but-important correctness
fix that's easy to miss in a casual review.

### 3. Why no progress events?

Unlike `train_job` (4 sequential trials, 2-30s), the
`promote_model` operation is a single atomic file
move (~10ms). There's nothing to "watch" — the IPC
returns the result in <100ms. v0.18a doesn't emit
`promote_model:started` / `promote_model:finished`
events; the L1 just shows a brief loading state on
the button, then displays the result.

If promote ever becomes slow (e.g. cross-device copy
or remote storage), the v0.17a-d event pattern can
be retrofitted. For now: simple, no events.

### 4. Reducer tests for the lastCandidate state

v0.18c's `lastCandidate` state has 4 transitions
(set, overwrite, clear-on-promote, keep-on-failure).
v0.18d extracts the transition logic into a pure
`lastCandidateReducer` function and tests it in
isolation (7 tests).

The state transitions are:

| action              | before           | after            |
|---------------------|------------------|------------------|
| `train_completed`   | `null`           | `{ jobId, ... }` |
| `train_completed`   | `{ jobId: A }`   | `{ jobId: B }`   |  ← overwrite
| `train_failed`      | `null` or `{ ... }` | unchanged   |  ← keep
| `promote_succeeded` | `{ jobId }`      | `null`           |
| `promote_failed`    | `{ jobId }`      | `{ jobId }`      |  ← keep

The reducer is pure (input state + action → new state).
The 7 tests cover all transitions + the null
invariants. Future changes to the reducer (e.g. adding
"keep on cancel") can be tested in isolation without
mounting React.

## Layer / module health

- **Layer rules**: `scripts/check-layers.mjs` still passes.
- **Doc sync**: `scripts/check-doc-sync.mjs` still passes.
- **CI guards**: `cargo test` + `pnpm test` + `pnpm typecheck` all pass.
- **End-to-end smoke**: `dev_smoke` builds and runs.
- **No native code changes**, no rebuild needed.

## Test growth history

```
v0.17e → 506
v0.18a → 512  (+6 promote parse tests)
v0.18b → 516  (+4 promote round-trip tests)
v0.18c → 516  (no new tests)
v0.18d → 523  (+7 lastCandidate reducer tests)
```

The v0.18a Rust parse tests are the first set of
wire-format tests for the lab/sidecar DTOs. They
cover both the success and failure paths of the
Python sidecar's response, including the "no
candidate" case (user clicked Promote before
Train) and the "job_id mismatch" case. The v0.17
final doc flagged this as an open question; v0.18a
sets the precedent for testing the wire-format DTOs
at the Rust level (round-trip + edge cases), not
just at the L1 round-trip level.

## Files changed in v0.18

```
src-tauri/src/domain/lab/sidecar.rs         (v0.18a — build_promote_request + PromoteResult + parse_promote_response + 6 tests)
src-tauri/src/commands/sidecar.rs           (v0.18a — promote_model IPC + PromoteModelArgs)
src-tauri/src/lib.rs                         (v0.18a — register promote_model IPC)
src/ipc.ts                                  (v0.18b — PromoteResult + PromoteModelArgs + promoteModel wrapper)
src/ipc.events.test.ts                      (v0.18b — 4 promote round-trip tests)
src/routes/ModelLab.tsx                     (v0.18c — lastCandidate + promoteMut + Promote button + last-candidate hint)
src/routes/ModelLab.lastCandidate.test.ts   (v0.18d — 7 reducer tests) [new]
src/lib/i18n.ts                             (v0.18c — 6 promote.* keys)
src/lib/i18n.test.ts                        (v0.18c — assert new keys)
docs/overview.md                            (doc-sync table, 3 new rows)
docs/polyrocket-v0.18-final.md              (this file) [new]
```

## Release binary

Not re-built for v0.18 — no native code changes. The
v0.13 final binary remains the current shipped build.

```
cd src-tauri && cargo build --release
```

Cold build ~1m12s.

## New convention (per 2026-06-17 user)

All v0.18 commits are local-only. The user pushes
manually. Saved to `~/.mavis/memory/user.md`.

## Next steps (deferred to v0.19+)

From the v0.13 final doc's deferred list:

1. **Parallel predict A/B test** — needs a rethink
   because the I/O bottleneck (single sidecar
   process) prevents true parallelism. v0.13d's
   `predict_async` + v0.15 event pattern + v0.16
   DTO discipline + v0.17 train pattern are the
   building blocks. v0.18a-b's Rust parse tests
   set a precedent for testing new IPCs.
2. **Real `rs-clob-client` integration** — wire the
   real Polymarket CLOB client.
3. **On-chain mirror execution** — currently the
   mirror executor records decisions but doesn't
   actually send transactions.
4. **Code-sign + DMG** — notarized `.dmg` for
   distribution.
5. **Auto-update feed** — Tauri updater pointed at
   GitHub Releases.
6. **Visual regression** — Playwright with the 54
   PNG snapshots as baselines.
7. **numpy vectorize predict** — replace the
   hot-path Python loop with a numpy vectorized
   implementation.

The v0.18 pattern (Train button → progress panel →
Promote button → result) is a complete model
lifecycle. v0.19 candidates:
- **Bulk promote**: add a "Promote all" button that
  promotes all the candidates in `n_trials` (e.g.
  the 4 trials from the last train) as separate
  model versions, so the user can A/B compare them.
- **Promote history**: show a list of past promotes
  in the ModelLab (the sidecar already records them
  in active.json's `promoted_at_ms`).
- **Model rollback**: add a "Rollback to previous
  active" button (the `previous_path` field from
  PromoteResult already records it).

The most natural follow-up: **Promote history** —
let the user see "you've promoted 5 models in the
last week, here's the timeline" so they can pick a
known-good version to roll back to if the latest
model is worse.

## Tally

```
4 commits, 9 files changed (across v0.18a-c)
+ 1 new doc
+ 1 new IPC (promote_model)
+ 6 Rust parse tests (the v0.17 "open question")
+ 4 L1 round-trip tests
+ 7 reducer tests (lastCandidate state machine)
+ 17 total new tests
+ 523 total tests, 100% pass
+ 54 PNG snapshots regenerated, no MD5 drift
+ 1 closed loop: Train → Promote → active model
+ 1 race-condition guard (job_id check)
```
