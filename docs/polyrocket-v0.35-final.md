# polyrocket v0.35 — final

**Branch**: main (local-only, not pushed)
**Commits**: `c9c511c` v0.34b → `<this commit>` v0.35c
**Released**: 2026-06-17 (local, awaiting user push)

## What changed since v0.34

| sub-version | hash       | one-liner                                            | tests at landing |
|-------------|------------|------------------------------------------------------|------------------|
| v0.35a      | (prev)     | diff-snapshots.mjs (byte-level) + 10 tests          | 661              |
| v0.35b      | (prev)     | check-doc-sync lint (info-only)                     | 661              |
| v0.35c      | this file  | Ship log + tally                                     | 661              |

**Test totals at v0.35 final**: cargo 257/257, vitest 319/319, python 69/69, script 16/16. **Total 645 (vitest+cargo+python) + 16 script = 661 total.**

## Highlights

### Catching visual regressions at commit time

The project's `snapshot_pages.py` script
generates 54 PNGs (3 themes × 18 pages) into
`docs/previews/`. Until v0.35, there was no
way to detect which PNGs changed after a code
change. A visual regression that broke a
single page would slip through the doc-sync
check (the docs themselves were unchanged).

v0.35a adds a byte-level diff tool
(`scripts/diff-snapshots.mjs`) that compares
two directories of PNGs and reports any
differences. The user can:

1. Run `snapshot_pages.py` to regenerate
2. Run `diff-snapshots.mjs docs/previews/
   /tmp/snapshots-baseline` to see what
   changed
3. Review the diff output
4. Either commit the visual changes (if
   intentional) or fix the regression (if
   not)

### Why byte comparison, not pixel diff

- **Fast**: O(N) in total file size, no PNG
  decoding
- **Deterministic**: no tolerance to tune
- **Catches ALL visual changes**: any pixel
  change → bytes differ
- **The PNG encoder is itself deterministic**
  given the same source, so a byte diff ==
  a visual diff

A pixel-diff approach would have:
- Required a PNG decoder (extra dep)
- Required a tolerance (subjective, hard
  to tune)
- Been slower (decode → compare pixels →
  encode diff)
- Missed some changes (e.g. font rendering
  might be deterministic at the byte level
  but not at the pixel level)

### The snapshot diff lint (v0.35b)

The `check-doc-sync.mjs` script now has a
4th check (informational only) that runs
when any `docs/previews/*.png` is staged.
It uses `git diff --cached --name-only
--diff-filter=AM` to find the list of
changed PNGs and prints them in the
commit output. This makes the change
visible to the reviewer without blocking
the commit.

The lint is NOT a blocker. Visual changes
may be intentional (e.g. a new feature
that affects the UI). The user can:

- Review the printed list and confirm
  the changes are intentional
- Run `diff-snapshots.mjs <before> <after>`
  manually for a more detailed diff
- Run `git checkout -- docs/previews/` to
  revert unintended changes

## Layer / module health

- **Layer rules**: `scripts/check-layers.mjs` still passes.
- **Doc sync**: `scripts/check-doc-sync.mjs` still passes.
- **L1↔Tauri guard v2 (v0.32a)**: dual-direction; passes.
- **Snapshot diff lint (v0.35b)**: info-only; reports changes.
- **CI guards**: `cargo test` + `pnpm test` +
  `pnpm typecheck` all pass.
- **End-to-end smoke**: `dev_smoke` builds and runs.

## Test growth history

```
v0.33d → 645
v0.34a → 651  (+6 L1 component tests for archive modal)
v0.34b → 651  (no new tests)
v0.35a → 661  (+10 diff-snapshots tests; 1 TS strictness fix)
v0.35b → 661  (no new tests — info-only lint)
v0.35c → 661  (no new tests)
```

## Files changed in v0.35

```
scripts/diff-snapshots.mjs              (v0.35a — byte-level diff tool, ~150 lines)
scripts/diff-snapshots.test.mjs         (v0.35a — 10 standalone tests)
scripts/check-doc-sync.mjs              (v0.35b — informational lint for changed PNGs)
src/components/feedback/PromoteHistoryArchive.test.tsx  (TS strictness fix from v0.34a)
docs/overview.md                        (doc-sync table, 1 new row)
docs/polyrocket-v0.35-final.md          (this file) [new]
```

## Release binary

Not re-built for v0.35 — pure tool/docs change.

```
cd src-tauri && cargo build --release
```

Cold build ~1m12s.

## New convention (per 2026-06-17 user)

All v0.35 commits are local-only. The user pushes
manually.

## What v0.35 completes

v0.35 closes the "snapshot diffing" deferred
item from the v0.31 final doc. The CI guard
infrastructure is now:

1. `scripts/check-doc-sync.mjs` —
   (a) DocSync: code changed but no doc
       updated
   (b) LayerGuard: Rust files import across
       disallowed layer edges
   (c) L1↔TauriGuard: wire format but no
       Tauri command (and inverse)
   (d) **SnapshotDiff lint (v0.35b)**: PNGs
       changed (info-only, not blocking)
2. `scripts/check-layers.mjs` — refuses
   commits where Rust files import across
   disallowed layer edges
3. `scripts/check-l1-tauri.mjs` (v0.32a) —
   refuses commits where an L1 wrapper has no
   matching Tauri command OR a Tauri command
   is defined but not registered
4. `scripts/diff-snapshots.mjs` (v0.35a) —
   manual byte-level diff tool for two
   directories of PNGs (not auto-run, the
   user invokes it explicitly)

The cumulative effect: governance is
self-enforcing for the recurring bug classes
the project has hit. The new tool is the
first one the user runs MANUALLY rather
than via a pre-commit hook — the rationale
is that visual changes are usually
intentional, so we don't want to block on
them. The lint in check-doc-sync provides
the visibility without the block.

## What v0.36+ holds (deferred)

The v0.33 final doc listed 3 candidates.
v0.34 + v0.35 closed 2. Remaining:

1. **Export/import of the auto-promote config**
   — share preferred margin with other users.
   Currently only one user per install.
2. **Snapshot diffing with a 7-day history**:
   keep the last 7 days of snapshots, compare
   each new snapshot to the same day last week.
   Useful for catching slow visual regressions.
3. **CI integration of `diff-snapshots.mjs`**:
   add a GitHub Action that runs
   `diff-snapshots.mjs` on every PR and
   comments on any visual diff. Currently
   it's a local tool; the user has to run
   it manually.

**Most natural v0.36 candidate: Export/import
of the auto-promote config.** The user
already has `autoPromoteBrierMargin` in
zustand+localStorage (v0.23c) and
`autoPromoteAfterTrain` (v0.28c). An
export/import flow would let them:
- Share their preferred config with other
  users (export to a JSON file, import from
  a JSON file)
- Backup their config before a re-install

This is a small L1 + IPC version (1-2
commits). The wire format is just the
prefs-store shape, the IPC is trivial
(read the file, parse, validate, write
to the prefs store).

Other v0.36+ candidates:
- **7-day snapshot history**: keep
  `docs/previews/history/{date}/` and
  compare each new snapshot to the same
  day last week. Catches slow visual
  drift (e.g. a font upgrade that subtly
  changes the layout).
- **CI integration**: a GitHub Action
  that runs `diff-snapshots.mjs` on every
  PR. Currently the project has no
  `.github/workflows/*.yml` for this
  (the existing `ci.yml` only runs
  cargo + pnpm test).

## Tally

```
3 commits (v0.35a-b + v0.35c), 4 files changed
+ 1 byte-level diff tool (150 lines, 10 tests)
+ 1 informational lint in check-doc-sync
+ 1 closed meta-loop (visual regressions now
  detectable at commit time, even if not
  blocking)
+ 661 total tests (was 651 in v0.34)
+ 0 native code changes
+ 0 new IPCs
+ 0 release binary changes
+ 54 PNG snapshots unchanged
```

## Acknowledgments

v0.35 is a focused 3-commit version: 1 tool +
1 lint + docs. The tool is small but useful
for a project that has 54 PNG snapshots and
no way to detect visual regressions until
now.

The pattern: build a small CLI tool, give
it standalone tests, integrate it as an
informational lint in the existing
governance entry point. The user can run
the tool manually for a detailed diff
(`diff-snapshots.mjs <before> <after>`)
or rely on the lint to alert them when
visual changes are staged.

The byte-level approach was chosen over
pixel-diff because:
- It's deterministic (no tolerance to tune)
- It's fast (no PNG decoding)
- The PNG encoder is itself deterministic,
  so a byte diff == a visual diff
- It catches all changes (any pixel change
  → bytes differ)

The cumulative governance infrastructure
is now:
- 3 blocking checks (DocSync, LayerGuard,
  L1↔TauriGuard)
- 1 informational check (SnapshotDiff lint)
- 1 manual tool (diff-snapshots.mjs)

The user has 4 ways to catch a regression
at commit time, plus 1 way to diff
snapshots manually.
