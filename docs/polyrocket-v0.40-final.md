# polyrocket v0.40 — final

**Branch**: main (local-only, not pushed)
**Commits**: `ede0e2d` v0.40a-b → `<this commit>` v0.40c
**Released**: 2026-06-18 (local, awaiting user push)

## What changed since v0.39

| sub-version | hash       | one-liner                                            | tests at landing |
|-------------|------------|------------------------------------------------------|------------------|
| v0.40a      | `ede0e2d`  | ModelComparison component (modal + "lowest Brier")  | 699              |
| v0.40b      | (same)     | PromoteHistory checkboxes + ModelLab button + query  | 699              |
| v0.40c      | this file  | Ship log + tally                                     | 699              |

**Test totals at v0.40 final**: cargo 257/257, vitest 342/342, python 69/69, script 31/31. **Total 668 (vitest+cargo+python) + 31 script = 699 total.**

## Highlights

### Multi-model comparison view

v0.40 closes the "multi-model comparison
view" deferred item from the v0.36 final
doc. The user can now:

1. Select 2-3 entries in the PromoteHistory
   panel (via checkboxes in each row)
2. Click "Compare (N)" button that appears
   when N >= 2
3. A Modal opens showing the entries in a
   side-by-side grid:
   - trial badge (best / trial N)
   - model_version
   - promoted_at (relative)
   - Brier (colored by value)
   - best_params (lr, reg)
4. The "lowest Brier wins" column is
   highlighted (green border + "★ best")
5. A summary at the bottom shows the
   winning model_version and Brier

### Why this matters

The user has been making rollback
decisions based on "eyeballing Brier
values in the history panel". A real
side-by-side view makes the decision
easier and more accurate. The user can
see:
- Which model has the best Brier
- What hyperparameters were used
- When each was promoted (newer vs older)
- Whether the new model is significantly
  better than the old one

The Rollback flow now has a clearer
"compare before you rollback" step.

### The "lowest Brier wins" convention

The convention is: lower Brier = better
model. The column with the smallest
`best_brier` is highlighted. If two
entries have the same Brier, the older
one wins (Set preserves insertion order,
and the in-memory history is oldest-first).

For null Brier (pre-v0.18 entries), the
entry is excluded from the "best"
calculation (we don't want a null Brier
to win over a 0.2 Brier).

### Files changed in v0.40

```
src/components/feedback/ModelComparison.tsx       (v0.40a — new component, ~180 lines)
src/components/feedback/ModelComparison.test.tsx  (v0.40a — 5 component tests)
src/components/feedback/PromoteHistory.tsx       (v0.40b — checkbox + selection props)
src/routes/ModelLab.tsx                          (v0.40b — selection state + button + query)
src/lib/i18n.ts                                  (v0.40b — 4 new keys × 2 locales)
docs/overview.md                                 (doc-sync table, 2 new rows)
docs/polyrocket-v0.40-final.md                   (this file) [new]
```

## Test growth history

```
v0.39c → 694
v0.40a → 699  (+5 ModelComparison tests)
v0.40b → 699  (no new tests; integration
                only, covered by manual)
v0.40c → 699  (no new tests)
```

## Release binary

Not re-built for v0.40 — pure L1 change.

```
cd src-tauri && cargo build --release
```

Cold build ~1m12s.

## New convention (per 2026-06-17 user)

All v0.40 commits are local-only. The
user pushes manually.

## What v0.40 completes

v0.40 closes the "multi-model comparison
view" deferred item. The model lifecycle
is now:

1. **Train** (v0.17a)
2. **Auto-promote** (v0.28, with OS
   notification v0.39)
3. **History** (v0.19c, with archive
   v0.33-0.34)
4. **Filter** (v0.30)
5. **Compare** (v0.40 — NEW)
6. **Rollback** (v0.20c)

The "compare before rollback" flow is
now first-class. The user can:
- Train a model
- See it auto-promote (with OS notification)
- Open the ModelLab page
- See the new entry at the top of history
- Compare it with 2-3 prior entries
- Decide whether to rollback (if the new
  model is actually worse than the old one)
  or keep it

The full comparison view is in v0.40.
The "rollback" flow is in v0.20c. The
two work together: compare → rollback
decision is now a real workflow.

## What v0.41+ holds (deferred)

The v0.36 final doc listed 3 candidates,
all closed (export/import, snapshot
history, CI). v0.39 closed "system
notifications". v0.40 closed "multi-model
comparison". New candidates:

1. **Per-promotion reason tooltips** (v0.41):
   the v0.28 worker's `message` field
   ("auto-promoted: improvement 0.012 >
   margin 0.005") is currently only in the
   toast. Surface it in the history panel
   as a hover tooltip on each row.
2. **List virtualization**: promote
   history now has 100+ entries
   (especially with archive). A virtual
   list would handle 1000+ without
   re-rendering everything.
3. **Telemetry** (POLYROCKET_TELEMETRY=1):
   the env var infrastructure exists but
   the implementation doesn't.
4. **CI integration of L1↔Tauri guard**:
   currently only runs as pre-commit hook;
   a CI run would catch the same bug class
   on PRs.
5. **Notification for skipped events**:
   v0.39 only fires OS notifications for
   `promoted: true`. A toggle for "notify
   on skip too" would let the user opt in.
6. **Compare in archive modal**: v0.40
   only compares the in-memory history
   (last 20). A "Compare from archive"
   option would let the user compare
   any 2-3 entries from the full history.

**Most natural v0.41 candidate: Per-
promotion reason tooltips.** The data
already exists (v0.28 worker's message
field). It's a small L1-only change.
Wires v0.28's metadata into v0.19c's
panel. About 1-2 commits.

Other v0.41+ candidates:
- **List virtualization**: bigger refactor
  (~3-4 commits for the virtualization
  infrastructure plus tests)
- **CI integration of guards**: small
  (~1-2 commits for a workflow file)
- **Telemetry**: large (~5-6 commits for
  the data model, IPC, UI)

## Tally

```
3 commits (v0.40a-b + v0.40c), 6 files changed
+ 1 new component (ModelComparison, ~180 lines)
+ 1 multi-select pattern in PromoteHistory
+ 1 "Compare (N)" button in ModelLab
+ 4 new i18n keys × 2 locales
+ 5 new component tests
+ 1 closed meta-loop (compare before
  rollback is now a real workflow)
+ 699 total tests (was 694 in v0.39)
+ 0 new IPCs
+ 0 release binary changes
+ 54 PNG snapshots unchanged
```

## Acknowledgments

v0.40 is a focused 3-commit version: 1 new
component + 1 integration + docs. The
work is the v0.36 final doc's "multi-model
comparison view" deferred item.

The pattern: the comparison view is a
pure L1 component that reads the existing
in-memory history. No new IPCs, no new
Rust code, no new schema. The new logic
is:
1. A `Set<string>` for selection state at
   the page level
2. A checkbox in each row (controlled by
   `onSelectionChange` prop)
3. A "Compare (N)" button that shows up
   when N >= 2
4. A Modal that filters the entries by
   the selected job_ids and renders
   side-by-side

The cumulative v0.39 + v0.40 + earlier
work on the model lifecycle:
- v0.17a — Train button + progress events
- v0.18a — Promote button
- v0.19a — History panel
- v0.20a — Rollback
- v0.21a — Bulk promote
- v0.22a — Brier chart
- v0.23a — "Promote if better" one-click
- v0.24a — Per-trial badges
- v0.25a — "Promote all 4" bulk button
- v0.28a — Background auto-promote worker
- v0.33a — Promote history archive
- v0.34a — "View archive" UI
- v0.39 — OS notification on completion
- v0.40 — Multi-model comparison view

The model lifecycle is now feature
complete for the core "view and decide"
loops. The user can train, see results
(via history, archive, chart, comparison),
and rollback with full context.
