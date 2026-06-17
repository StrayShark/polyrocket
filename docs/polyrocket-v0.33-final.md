# polyrocket v0.33 — final

**Branch**: main (local-only, not pushed)
**Commits**: `2811950` v0.33a-c → `<this commit>` v0.33d
**Released**: 2026-06-17 (local, awaiting user push)

## What changed since v0.32

| sub-version | hash       | one-liner                                            | tests at landing |
|-------------|------------|------------------------------------------------------|------------------|
| v0.33a      | `2811950`  | Python: archive dropped entries (4 tests)           | 635              |
| v0.33b      | (same)     | Rust: list_promote_history_archive IPC (4 tests)     | 639              |
| v0.33c      | (same)     | L1: IPC wrapper (6 wire-format tests)                | 645              |
| v0.33d      | this file  | Ship log + tally                                     | 645              |

**Test totals at v0.33 final**: cargo 257/257, vitest 313/313, python 69/69, script 6/6. **Total 639 (vitest+cargo+python) + 6 script = 645 total.**

## Highlights

### The 20-entry cap was silently dropping data

v0.19a added `promotion_history[]` inside `active.json`,
capped at 20 entries. Every time the 21st promotion
happened, the oldest entry was silently dropped —
no archive, no warning, just gone. For a user
who promotes once a week, that's 5 months of audit
data lost.

v0.33 fixes this by writing dropped entries to
a separate JSONL file **before** the cap takes
effect. The user can now audit the full history,
not just the most recent 20.

### The architecture

```
              +-------------------+
promote ----> |  active.json      |  (last 20, in-memory)
              |   promotion_history| 
              +-------------------+
                     |
                     |  overflow? (>= 20 entries)
                     v
              +-------------------+
              |  archive.jsonl    |  (append-only, all)
              |  1 JSON / line    |
              +-------------------+
                     ^
                     |  list_promote_history_archive
                     |  IPC (v0.33b)
                     |
              Rust L2 + L1
```

The Python sidecar writes to both. The Rust
side reads both. The L1 can show either:
- "Promotion history" panel (last 20, in-memory)
- "View archive" panel (full history, archived)

### Why JSONL, not SQLite

The archive is JSONL for 3 reasons:

1. **Append-only is the natural fit**: JSONL is
   line-delimited, so append is O(1) per entry.
   No transaction overhead. The Python sidecar
   already uses single-line JSON for `active.json`,
   so the pattern is consistent.

2. **No schema migration burden**: a SQLite table
   would need a migration on schema change. JSONL
   just parses whatever fields are present —
   missing fields default to `null` on the TS
   side.

3. **The expected size is small**: a few hundred
   entries per year at most. The O(N) read cost
   is fine for that scale. If the archive ever
   grows to millions of entries, we'd migrate
   to SQLite with an index on `promoted_at_ms`.

### Failure modes handled

- **Archive file doesn't exist yet**: the Rust
  IPC returns `ok: true` with `entries: []`
  and a message "no archive yet; archive is
  created on first overflow". The L1 shows
  "no archive" instead of an error.
- **Archive write fails** (e.g. disk full): the
  Python sidecar logs a warning and continues
  the promote. The primary audit trail (the
  20-entry in-memory history) is unaffected.
  The user gets a slightly degraded experience
  (no archive for the dropped entry) but the
  promote still succeeds.
- **Archive read fails** (e.g. corrupted line):
  the Rust IPC skips malformed lines silently
  and continues. The user sees the valid lines
  with no error toast.
- **Archive file is huge** (millions of entries):
  the Rust IPC reads the whole file. For the
  expected scale (hundreds) this is fast. For
  the unexpected scale (millions), the user
  can clean it up manually with a shell command.

## Layer / module health

- **Layer rules**: `scripts/check-layers.mjs` still passes.
- **Doc sync**: `scripts/check-doc-sync.mjs` still passes.
- **L1↔Tauri guard v2 (v0.32a)**: dual-direction; passes.
- **CI guards**: `cargo test` + `pnpm test` +
  `pnpm typecheck` all pass.
- **End-to-end smoke**: `dev_smoke` builds and runs.

## Test growth history

```
v0.30  → 629  (+5 history filter tests)
v0.31  → 629  (no new tests)
v0.32a → 631  (+2 guard v2 tests)
v0.32b → 631  (no new tests)
v0.33a → 635  (+4 python archive tests)
v0.33b → 639  (+4 rust archive tests)
v0.33c → 645  (+6 L1 wire-format archive tests)
v0.33d → 645  (no new tests)
```

## Files changed in v0.33

```
sidecar/polyrocket_sidecar/train.py                  (v0.33a — _archive_dropped_entries + ARCHIVE_FILE)
sidecar/tests/test_train.py                          (v0.33a — TestPromoteHistoryArchive with 4 tests)
src-tauri/src/commands/sidecar.rs                    (v0.33b — list_promote_history_archive + 4 tests)
src-tauri/src/lib.rs                                 (v0.33b — register new IPC)
src/ipc.ts                                           (v0.33c — listPromoteHistoryArchive wrapper + 3 interfaces)
src/ipc.events.test.ts                               (v0.33c — 6 wire-format tests)
docs/overview.md                                     (doc-sync table, 4 new rows)
docs/polyrocket-v0.33-final.md                       (this file) [new]
```

## Release binary

Not re-built for v0.33 — pure audit trail change.
The Python sidecar behavior change is internal
(the user-visible interface is the same; the
old `list_promote_history` IPC still returns the
last 20 entries).

```
cd src-tauri && cargo build --release
```

Cold build ~1m12s.

## New convention (per 2026-06-17 user)

All v0.33 commits are local-only. The user pushes
manually.

## What v0.33 completes

v0.33 closes the "promote history auto-cleanup"
deferred item from v0.31 final doc. The audit
trail is now durable beyond the 20-entry in-memory
cap. The user can:

1. Promote 100 models
2. The first 80 (the ones that fell off the
   in-memory cap) are safely in the archive
3. View the archive via a future L1 button
   (UI not in v0.33, but the IPC + types are
   ready)
4. Roll back to ANY of the 100 models (the
   archive has the weights, just like the
   in-memory history)

The audit trail is now:
- **Primary**: `active.json::promotion_history[]`
  (last 20, in-memory, fast)
- **Secondary**: `archive.jsonl` (full history,
  append-only, durable)

## What v0.34+ holds (deferred)

The v0.31 final doc listed 3 candidates. v0.33
closed the "promote history auto-cleanup" one.
Remaining:

1. **Snapshot diffing** — capture the 54 PNG
   snapshots BEFORE a change, capture them
   AFTER, diff the two sets, alert on unintended
   visual changes. The current `snapshot_pages.py`
   just regenerates.
2. **Export/import of the auto-promote config**
   — share preferred margin with other users.
   Currently only one user per install.
3. **L1 "View archive" button** — wire the
   v0.33c IPC into a real button on the ModelLab
   page so the user can actually see the archive
   (the IPC + types are ready; just need a
   `PromoteHistoryArchive` component + a
   `View archive` button in ModelLab).

**Most natural v0.34 candidate: L1 "View
archive" button.** The v0.33c IPC and types
are ready; the missing piece is the UI. It's
a small L1-only version (1-2 commits).

Other v0.34+ candidates:
- **Snapshot diffing**: the snapshots are PNG
  bytes; a diff tool would compare them and
  alert on changes. The diff itself is
  straightforward (just compare bytes or
  pixel-diff with a tolerance), but the
  "before" capture workflow is non-trivial
  (need to know what "before" means in a
  pre-commit hook).
- **Export/import of prefs**: the prefs store
  is zustand+localStorage; exporting it
  requires a JSON shape, importing requires
  validation. A real user-shareable format
  would also need versioning.

## Tally

```
4 commits (v0.33a-c bundled into 1 + v0.33d docs), 7 files changed
+ 1 JSONL archive file format (Python sidecar writes)
+ 1 new Rust Tauri command (list_promote_history_archive)
+ 1 new L1 IPC wrapper + 3 interfaces
+ 14 new tests (4 python + 4 rust + 6 L1 wire)
+ 1 durable audit trail (was 20-entry in-memory only)
+ 645 total tests (was 631 in v0.32)
+ 0 release binary changes
+ 0 native code changes
+ 0 L1 components added (UI for archive deferred to v0.34)
+ 54 PNG snapshots unchanged
```

## Acknowledgments

v0.33 is a focused 4-commit version (3 code
commits bundled + 1 docs commit) that closes
a real audit-trail data-loss bug. The bug was
silent: when the 21st promote happened, the
oldest entry was dropped with no warning, no
archive, no nothing. A user promoting once a
week would lose 5 months of audit data in a
year.

The fix is straightforward: write dropped
entries to a JSONL file BEFORE the cap takes
effect. The Python sidecar change is ~30 lines
(`_archive_dropped_entries` helper + the
`if len(history) >= 20` check). The Rust
IPC is ~80 lines (read + filter + sort +
paginate). The L1 wrapper is ~30 lines
(interfaces + the invoke call).

The cost is 14 new tests. The benefit is a
durable audit trail beyond the 20-entry in-
memory cap.

The L1 "View archive" UI is deferred to v0.34
to keep v0.33 focused. The IPC + types are
ready, so v0.34 is a pure-L1 commit.
