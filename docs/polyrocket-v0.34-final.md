# polyrocket v0.34 — final

**Branch**: main (local-only, not pushed)
**Commits**: `2811950` v0.34a → `<this commit>` v0.34b
**Released**: 2026-06-17 (local, awaiting user push)

## What changed since v0.33

| sub-version | hash       | one-liner                                            | tests at landing |
|-------------|------------|------------------------------------------------------|------------------|
| v0.34a      | (prev)     | PromoteHistoryArchive component + ModelLab button   | 651              |
| v0.34b      | this file  | Ship log + tally                                     | 651              |

**Test totals at v0.34 final**: cargo 257/257, vitest 319/319, python 69/69, script 6/6. **Total 645 (vitest+cargo+python) + 6 script = 651 total.**

## Highlights

### The audit trail is now user-visible

v0.33a-c made the audit trail durable by
archiving dropped entries to `archive.jsonl`.
v0.33b added the Rust IPC to read the archive.
v0.34a (this version) is the L1 UI for actually
seeing the archive.

The user can now:
1. Click "View archive" below the existing
   Promotion history panel on the ModelLab page
2. A Modal opens with the full audit trail
   (paginated, 25 entries per page)
3. Each row shows the model_version, Brier
   (colored by value), and trial badge
   (best trial vs trial N)
4. Prev/Next buttons navigate the pages
5. The Modal can be closed with Esc or the
   X button

### The Modal pattern

The PromoteHistoryArchive uses the project's
existing Modal component (v0.10c) which has
the full a11y treatment:
- Focus trap (Tab/Shift+Tab cycle inside)
- Esc closes
- Focus is restored to the trigger button
  on close
- `role="dialog"` + `aria-modal="true"`
- The Modal is mounted at the page level (not
  inside the Card) to avoid z-index issues

### Why the L1 came last

The pattern: build the durable storage (v0.33a),
then the IPC (v0.33b), then the L1 wrappers
(v0.33c), then the L1 UI (v0.34a). Each layer
unlocks the next.

This pattern is **bottom-up**: the lowest layer
that doesn't depend on anything else is built
first. The Python archive is the lowest layer
(no L1, no Rust). The Rust IPC is the next
layer (depends on the Python archive). The L1
wrapper is the next layer (depends on the Rust
IPC). The L1 UI is the topmost layer (depends
on the L1 wrapper).

The benefit: each commit is independently
testable. v0.33a's Python test could verify
the archive file format without touching the
Rust or L1. v0.33b's Rust test could verify
the IPC contract without touching the L1.
v0.34a's L1 test could verify the UI without
touching the Rust or Python.

## Layer / module health

- **Layer rules**: `scripts/check-layers.mjs` still passes.
- **Doc sync**: `scripts/check-doc-sync.mjs` still passes.
- **L1↔Tauri guard v2 (v0.32a)**: dual-direction; passes.
- **CI guards**: `cargo test` + `pnpm test` +
  `pnpm typecheck` all pass.
- **End-to-end smoke**: `dev_smoke` builds and runs.

## Test growth history

```
v0.32b → 631
v0.33a → 635  (+4 python archive tests)
v0.33b → 639  (+4 rust archive tests)
v0.33c → 645  (+6 L1 wire-format archive tests)
v0.34a → 651  (+6 L1 component tests for archive modal)
v0.34b → 651  (no new tests)
```

## Files changed in v0.34

```
src/components/feedback/PromoteHistoryArchive.tsx       (v0.34a — new component, ~180 lines)
src/components/feedback/PromoteHistoryArchive.test.tsx  (v0.34a — 6 component tests)
src/routes/ModelLab.tsx                                 (v0.34a — "View archive" button + state)
src/lib/i18n.ts                                         (v0.34a — 7 new keys × 2 locales)
docs/overview.md                                        (doc-sync table, 1 new row)
docs/polyrocket-v0.34-final.md                          (this file) [new]
```

## Release binary

Not re-built for v0.34 — pure L1 change.

```
cd src-tauri && cargo build --release
```

Cold build ~1m12s.

## New convention (per 2026-06-17 user)

All v0.34 commits are local-only. The user pushes
manually.

## What v0.34 completes

v0.34 completes the audit-trail work that v0.33
started. The full chain is now:

```
promote ─→ active.json (last 20, in-memory)
       │
       └─→ archive.jsonl (full history, append-only)
              │
              └─→ Rust IPC (v0.33b)
                     │
                     └─→ L1 wrapper (v0.33c)
                            │
                            └─→ L1 UI (v0.34a)
```

The user can:
- See the last 20 promotions in the
  Promotion history panel (existing, v0.19c)
- See the full history in the Archive modal
  (new, v0.34a)
- Roll back to ANY of the 100+ promotions
  (the archive has the weights, just like
  the in-memory history)

## What v0.35+ holds (deferred)

The v0.33 final doc listed 3 candidates.
v0.34 closed the "L1 View archive button"
one. Remaining:

1. **Snapshot diffing** — capture the 54 PNG
   snapshots BEFORE a change, capture them
   AFTER, diff the two sets, alert on unintended
   visual changes. The current `snapshot_pages.py`
   just regenerates.
2. **Export/import of the auto-promote config**
   — share preferred margin with other users.
   Currently only one user per install.
3. **Snapshot diffing** with a 7-day history:
   keep the last 7 days of snapshots, compare
   each new snapshot to the same day last week.
   Useful for catching slow visual regressions.

**Most natural v0.35 candidate: Snapshot diffing.**

The `snapshot_pages.py` script is the project's
"visual test suite". It generates 54 PNG files
(3 themes × 18 pages). Today, it just regenerates
them — there's no "before" vs "after" comparison.
A diff tool would:
1. Copy the current 54 PNGs to a `before/` dir
2. Run the snapshot script (regenerates)
3. Diff each new PNG vs the corresponding
   before/ PNG
4. Report visual regressions (pixel diff
   with a tolerance)

This is a **meta-feature** like the L1↔Tauri
guard — no user-visible behavior, just catches
a bug class (visual regressions).

## Tally

```
2 commits (v0.34a + v0.34b), 5 files changed
+ 1 L1 component (PromoteHistoryArchive, ~180 lines)
+ 1 "View archive" button in ModelLab
+ 7 new i18n keys × 2 locales
+ 6 new L1 component tests
+ 1 closed meta-loop (audit trail now user-visible)
+ 651 total tests (was 645 in v0.33)
+ 0 native code changes
+ 0 new IPCs
+ 0 release binary changes
+ 54 PNG snapshots unchanged
```

## Acknowledgments

v0.34 is a focused 2-commit version: 1 L1
component + final docs. The work is a small
but user-visible addition: the audit trail
that v0.33 made durable is now readable
through the UI.

The v0.34a commit is the largest single L1
change in several versions. The component
has 4 render states (loading/error/empty/
populated), pagination state, a11y via the
existing Modal component, and a 7-key i18n
treatment in two locales. The 6 component
tests cover the main paths: empty, populated,
trial badge, pagination, disabled state,
and the `enabled: open` query flag.

The pattern: each layer is bottom-up. v0.33a
(write the archive) → v0.33b (read it via IPC)
→ v0.33c (L1 wrapper) → v0.34a (L1 UI). Each
commit is independently testable.

The next meta-feature candidate is snapshot
diffing (v0.35). The pattern is the same as
the L1↔Tauri guard: a small script that catches
a recurring bug class (visual regressions) at
commit time.
