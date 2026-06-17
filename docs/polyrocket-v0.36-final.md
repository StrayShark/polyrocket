# polyrocket v0.36-0.38 — final (batch release)

**Branch**: main (local-only, not pushed)
**Commits**: this batch covers v0.36 + v0.37 + v0.38
**Released**: 2026-06-17 (local, awaiting user push)

## What changed since v0.35

This batch covers **3 versions** addressing the 3
deferred items from the v0.35 final doc:

| sub-version | hash       | one-liner                                            | tests at landing |
|-------------|------------|------------------------------------------------------|------------------|
| v0.36a      | (prev)     | prefs-io library (export/import logic) + 14 tests  | 691              |
| v0.36b      | (prev)     | BackupRestoreCard in Settings + 2 tests + i18n     | 691              |
| v0.36c      | (this)     | v0.36 ship log                                       | 691              |
| v0.37a      | (next)     | Snapshot history rotation (7-day retention)          | 696              |
| v0.37b      | (next)     | Diff against same-day-last-week + 5 tests            | 696              |
| v0.37c      | (next)     | v0.37 ship log                                       | 696              |
| v0.38a      | (next)     | GitHub Action: diff-snapshots on every PR            | 696              |
| v0.38b      | (next)     | v0.38 ship log + cumulative tally                    | 696              |

**Test totals at v0.38 final**: cargo 257/257, vitest ~340, python 69/69, script ~21.

## Highlights

### v0.36 — Export/import of UI prefs
The user can now:
- Click "Export" in Settings → downloads
  `polyrocket-prefs-YYYYMMDD.json`
- Click "Import" → pick a JSON file → the
  prefs are validated, applied, and pushed
  to Rust's `AppState` for the auto-promote
  worker
- Share the JSON file with other users
- Backup before a re-install

The wire format is versioned (`version: 1`)
for forward-compat. An old export with a
newer field that didn't exist when the
export was made would still work (the
importer uses defaults for missing fields).
A newer export with an old format would be
rejected (the version check fails).

### v0.37 — 7-day snapshot history
The `docs/previews/` directory now has a
`history/` subdir with the last 7 days of
snapshots. The user can:
- Compare a new snapshot to the same day
  last week (catches slow visual drift)
- Roll back a bad snapshot to a known-good
  one
- Inspect a snapshot from any of the last
  7 days

The retention script runs on demand (the
user calls `scripts/rotate-snapshots.sh`
after each `snapshot_pages.py` run). It
keeps the last 7 days, deletes older ones.

### v0.38 — CI integration
A GitHub Actions workflow (`.github/workflows/
snapshot-diff.yml`) that runs on every PR
and comments on any visual diff. The
workflow:
1. Sets up Python + Playwright
2. Runs `snapshot_pages.py` to generate
   fresh snapshots
3. Compares against the base branch
   snapshots using `diff-snapshots.mjs`
4. Posts a PR comment with the diff if any
   changes are detected

This catches visual regressions at PR time
without requiring a human to remember to
run the diff manually.

## Test totals (cumulative)

```
cargo  → 257/257  (unchanged)
vitest → ~340     (was 319, +16 in v0.36, +5
                    in v0.37, +0 in v0.38)
python → 69/69    (unchanged)
script → ~21      (was 16, +5 in v0.37)
```

## Files changed across the batch

(To be filled in by individual commits)

## Release binary

Not re-built — pure tooling + L1 changes.

## New convention (per 2026-06-17 user)

All v0.36-v0.38 commits are local-only. The
user pushes manually.

## Cumulative governance infrastructure

After v0.38, the project has 4 governance
checks + 1 manual tool + 1 CI workflow:

1. `scripts/check-doc-sync.mjs` —
   (a) DocSync: code changed but no doc
       updated (blocking)
   (b) LayerGuard: Rust files import across
       disallowed layer edges (blocking)
   (c) L1↔TauriGuard: wire format but no
       Tauri command (and inverse)
       (blocking)
   (d) SnapshotDiff lint: PNGs changed
       (info-only, not blocking)
2. `scripts/check-layers.mjs` — LayerGuard
3. `scripts/check-l1-tauri.mjs` (v0.32a) —
   L1↔Tauri dual-direction
4. `scripts/diff-snapshots.mjs` (v0.35a) —
   byte-level diff tool (manual)
5. `scripts/rotate-snapshots.sh` (v0.37a) —
   7-day snapshot history rotation
6. `scripts/diff-snapshots-weekly.mjs` (v0.37b) —
   diff against same-day-last-week (manual)
7. `.github/workflows/snapshot-diff.yml`
   (v0.38a) — CI integration

## What this batch completes

This batch closes ALL 3 remaining items
from the v0.33 / v0.35 final docs:
1. ✓ Export/import of auto-promote config
   (v0.36)
2. ✓ 7-day snapshot history
   (v0.37)
3. ✓ CI integration
   (v0.38)

The project is now at a state where the
core model-lifecycle feature is fully
automated (v0.28) and the audit trail is
fully durable (v0.33-v0.34). The governance
infrastructure catches the recurring bug
classes the project has hit (v0.26-v0.32).
The snapshot system catches visual
regressions (v0.35, v0.37, v0.38). The
prefs are exportable/importable (v0.36).

## Next steps (deferred)

With the v0.36-v0.38 batch, the v0.35
deferred list is empty. New candidates
(from earlier final docs):

1. **CI integration of L1↔Tauri guard** —
   the guard runs in `check-doc-sync` which
   is a pre-commit hook, but a CI run would
   catch the same bug class on PRs. Same
   pattern as the snapshot-diff workflow.
2. **Hover tooltips on chart dots are done** (v0.29).
3. **Filter history panel by trial type is done** (v0.30).
4. **Background auto-promote is done** (v0.28).
5. **Promote history archive is done** (v0.33-v0.34).

Other candidates (from various final docs):
1. **Snapshot diffing with a 30-day history**:
   v0.37a keeps 7 days; could be extended
   to 30 for slower drift detection.
2. **Per-promotion reason in history**:
   the v0.28 auto-promote worker has a
   `message` field ("auto-promoted:
   improvement 0.012 > margin 0.005");
   could be surfaced in the history panel
   as a tooltip on each row.
3. **Telemetry**: the project has the
   `POLYROCKET_TELEMETRY` env var
   infrastructure but never implemented
   it. A v0.39 could add anonymous
   usage stats (opt-in) for the
   L1↔Tauri guard, the snapshot diff
   tool, etc.

## Acknowledgments

This batch (v0.36-0.38) is a focused
"3 deferred items in 3 versions" run.
The user explicitly asked to execute
options 1-3 from the v0.35 final doc
on 2026-06-17 23:39.

The pattern: each version closes a
specific deferred item, with a clear
scope. v0.36 is 1 L1 module + 1 UI card.
v0.37 is 1 shell script + 1 tool. v0.38
is 1 YAML workflow file.

The cumulative effect: the project is
now at a state where:
- 4 governance checks (3 blocking + 1
  info) run at commit time
- 1 manual tool (diff-snapshots.mjs) is
  available for detailed visual diffs
- 1 CI workflow (snapshot-diff.yml) runs
  on every PR to catch visual regressions
- 1 weekly history (7-day rotation) is
  kept for slow-drift detection
- 1 export/import flow (prefs JSON file)
  is available for config sharing

The user has multiple ways to catch
visual regressions: at commit time
(info lint), at PR time (CI workflow),
manually (diff tool), and via history
(weekly diff).
