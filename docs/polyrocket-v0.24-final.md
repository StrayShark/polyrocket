# polyrocket v0.24 — final

**Branch**: main (local-only, not pushed)
**Commits**: `78febf0` v0.24a → `<this commit>` v0.24b
**Released**: 2026-06-17 (local, awaiting user push)

## What changed since v0.23

| sub-version | hash       | one-liner                                            | tests at landing |
|-------------|------------|------------------------------------------------------|------------------|
| v0.24a      | `78febf0`  | Per-trial badges in PromoteHistory rows             | 590              |
| v0.24b      | this file  | Ship log + tally                                     | 590              |

**Test totals at v0.24 final**: cargo 249/249, vitest 279/279, python 62/62. **Total 590/590.**

## Highlights

### 1. Make the audit trail easier to read

v0.24 is a focused 2-commit version that surfaces
data we already had. v0.21a wrote `trial_index`
to the promotion history (Python side) and
v0.21a parsed it (Rust DTO). v0.24a just makes
the L1 actually show it.

The user can now see at a glance whether a
history entry was:
- The auto-picked best trial (badge: "best trial")
- A bulk-promoted specific trial (badge: "trial #N")

Combined with the existing model_version suffix
(`-t2`, `-t3`), the trial source is now
immediately visible.

### 2. Why both badge and suffix?

The model_version suffix (`-t2`) is hard to
spot in a long list — the user has to look
carefully at the text. The badge is visually
distinct and shows the trial number prominently.

The two reinforce each other:
- The suffix is the canonical identifier
  (used everywhere: history panel, ModelVersionPill,
  predict's model_version field). Machine-readable.
- The badge is a quick visual cue ("this was a
  manual override, not the auto-pick"). Human-readable.

If we ever change the suffix format (e.g. drop
it, change to `:2` or `_t2`), the badge stays
the same. The badge is presentation; the suffix
is data.

### 3. Pure L1 feature (no Rust, no Python)

The data was already in `promote_history.entries[].trial_index`
since v0.21a. v0.24a is purely the L1 consumption:
- TS interface gets a new field
- UI component renders a badge
- i18n adds labels
- Tests cover the rendering

This is the second pure-L1 version in a row
(v0.22 was the first; v0.23 had Rust+Python
changes for auto-promote). The cadence has
shifted toward "consume data we already have"
rather than "build new IPCs".

### 4. Backward compat with v0.18 entries

Old history entries from before v0.21a don't
have the `trial_index` field at all. The L1
type marks it as optional + nullable:

```typescript
trial_index?: number | null;
```

Three cases handled:
- `null` (Python returned null for the auto-pick)
- `Some(n)` (Python returned the trial index)
- `undefined` (field is missing entirely — v0.18 back-compat)

All three are tested explicitly in
`PromoteHistory.test.tsx`.

## Layer / module health

- **Layer rules**: `scripts/check-layers.mjs` still passes.
- **Doc sync**: `scripts/check-doc-sync.mjs` still passes.
- **CI guards**: `cargo test` + `pnpm test` + `pnpm typecheck` all pass.
- **End-to-end smoke**: `dev_smoke` builds and runs.
- **No native code changes** in v0.24, no rebuild needed.

## Test growth history

```
v0.22c → 573
v0.23a → 584  (+6 Rust + 5 Python)
v0.23b → 587  (+3 L1 round-trip)
v0.23c → 587  (no new tests; pure UI)
v0.23d → 587  (no new tests)
v0.24a → 590  (+3 component)
v0.24b → 590  (no new tests)
```

## Files changed in v0.24

```
src/ipc.ts                                 (v0.24a — PromoteHistoryEntry.trial_index)
src/components/feedback/PromoteHistory.tsx (v0.24a — per-trial badge in row)
src/components/feedback/PromoteHistory.test.tsx (v0.24a — 3 new tests)
src/ipc.events.test.ts                     (v0.24a — added trial_index to test interface)
src/lib/i18n.ts                            (v0.24a — 2 new keys, en + zh)
src/lib/i18n.test.ts                       (v0.24a — assert new keys)
docs/overview.md                           (doc-sync table, 2 new rows)
docs/polyrocket-v0.24-final.md             (this file) [new]
```

## Release binary

Not re-built for v0.24 — no native code changes.
The v0.13 final binary remains the current shipped
build.

```
cd src-tauri && cargo build --release
```

Cold build ~1m12s.

## New convention (per 2026-06-17 user)

All v0.24 commits are local-only. The user pushes
manually.

## Next steps (deferred to v0.25+)

The v0.23 final doc listed 3 candidates. v0.24
closed the per-trial badges one. Remaining:
1. **Promote batch** — "promote all 4 trials" with
   one click, instead of 4 separate button clicks
2. **Background scheduler auto-promote** — have
   the scheduler call `auto_promote_if_better`
   after each train completes
3. **Hover tooltips on chart dots** — hovering a
   dot shows "Brier 0.184, logistic-train-XYZ,
   promoted 3h ago"

**Most natural v0.25 candidate: Promote batch**.

The user can train 4 trials per click (v0.17).
With bulk promote (v0.21c), they can promote
each one with a separate click. Promote batch
would promote all 4 with one click — useful for
A/B comparison.

The data foundation is in place:
- `candidate.json` has `all_trials[]` (4 trials)
- `run_promote_model` accepts `trial_index` (v0.21a)
- `promotion_history` records each promote (v0.19a)
- The history panel already shows the trial badges (v0.24a)

The implementation:
- v0.25a: Python `run_promote_all_trials()` — runs
  `run_promote_model` for each of the 4 trials
  and returns a list of results
- v0.25b: Rust IPC `promote_all_trials` + L1 wrapper
- v0.25c: TrainProgress "Promote all 4" button
- v0.25d: Tests + final docs

The user mental model: "I want to see how all
4 trials perform on real markets. Promote all
4, then watch the Brier chart for a week, then
rollback to the winner." This is the natural
next step in the model lifecycle.

Other v0.25+ candidates:
- **Background scheduler auto-promote**: have
  the scheduler call `auto_promote_if_better`
  after each train completes. Removes the need
  for the user to click "Promote if better".
  Higher risk (silent model swap), but useful
  for power users.
- **Hover tooltips on chart dots**: hovering a
  dot shows "Brier 0.184, logistic-train-XYZ,
  promoted 3h ago". Simple but nice-to-have.
  Adds accessibility for users who can't easily
  resolve small visual differences.
- **Filter the history panel by trial type**:
  add a small toggle "Show only bulk-promoted" /
  "Show only best-trial" so the user can focus
  on one or the other.

## Tally

```
2 commits, 7 files changed (across v0.24a-b)
+ 1 new L1 field (PromoteHistoryEntry.trial_index)
+ 1 new UI element (per-trial badge)
+ 1 new test interface field (test-side PromoteHistoryEntry)
+ 2 new i18n keys × 2 locales
+ 3 new component tests
+ 0 native code changes (no Rust/Python)
+ 0 new IPCs (pure L1 feature)
+ 590 total tests, 100% pass
+ 54 PNG snapshots regenerated, no MD5 drift
```

## Acknowledgments

v0.24 was a focused 2-commit version. The
pattern was: pure-L1 feature with no Rust/Python
changes. The data was already in the wire
format (since v0.21a); v0.24a just surfaced it
in the UI.

The "consume existing data" pattern is the
right approach for "make the audit trail
easier to read" features. No new IPC, no new
DTO, no new tests on the wire format. Just
"we have this data; let's show it better".

The trial-badge design choice (suffix AND
badge) is worth noting. The suffix is the
canonical identifier (used in code); the
badge is the visual cue (for humans). They
serve different audiences.

The v0.18 back-compat for `trial_index` is
also worth noting. The L1 type uses
`trial_index?: number | null;` which covers
all three cases (null, Some, undefined). The
test explicitly covers all three. The Python
side writes null for the best; v0.18 entries
don't have the field at all. Both are handled
gracefully.
