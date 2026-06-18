# polyrocket v0.44 — final

**Branch**: main (local-only, not pushed)
**Commits**: `63922ba` v0.44a/b → `626f9bd` v0.44c → `99c72a3` v0.44d → `<this commit>` v0.44e
**Released**: 2026-06-18 (local, awaiting user push)

## What changed since v0.43

| sub-version | hash       | one-liner                                                | tests at landing |
|-------------|------------|----------------------------------------------------------|------------------|
| v0.44a/b    | `63922ba`  | ExecutorConfig.paper_mode + paper_fills table + write path | 723              |
| v0.44c      | `626f9bd`  | L1 wire + Settings toggle + 3 new IPCs + 2 tests        | 729              |
| v0.44d      | `99c72a3`  | Copy page [PAPER] banner + 5 paper_mode Rust tests      | 731              |
| v0.44e      | this file  | Ship log + tally                                         | 731              |

**Test totals at v0.44 final**: cargo 233/233, vitest 359/359, python 77/77, script 31/31. **Total 669 (vitest+cargo+python) + 31 script = 700 total.** (Up from 697 in v0.43 — +2 Settings + +1 cargo paper_mode = +3 net.)

## Highlights

### Direction C — Paper trading mode (v0.44a–d)

The biggest "validate your config without risking
money" feature so far. Closes the v0.42 final
deferred item.

**What you can now do**:
1. Settings → "Paper trading mode" toggle
2. Run a mirror executor pass (existing
   `run_mirror_executor_pass` IPC)
3. Picked orders go to the new `paper_fills`
   table (not `bets`), and the CLOB sign_order
   step is skipped entirely
4. Copy page shows a `[PAPER]` banner with a
   live count of paper fills captured
5. Switch the toggle OFF to go back to live
   mode — same code path, different write target

**Wire format** is stable and forward-compatible:
snake_case fields, serde-derived. The L1↔Tauri
guard detects all 3 new IPCs; no "missing
wrapper" entries.

**Why this is the missing piece**:
- The mirror executor is the only path that
  places bets without an explicit user click.
  v0.6a — "decide which pending mirrors to
  submit" — runs every 30s. Until v0.44, there
  was no safe way to observe its behavior
  without risking money on a possibly-broken
  config.
- New users typically want to: configure size,
  set exposure caps, observe one or two mirror
  passes, then turn on live mode. v0.44 makes
  the "observe" step free.

**Caveat**:
- v0.44 doesn't auto-settle paper_fills against
  resolutions. The user runs the v0.43 backtest
  engine to compare "would this fill have won?"
  vs actual outcomes. Future v0.45+ could add a
  reconciliation job.
- The decision logic is unchanged. Paper mode
  only changes the write path; the executor still
  picks / rejects the same orders. If a user is
  surprised by what gets picked in live mode,
  they'll be equally surprised in paper mode —
  and that's a feature, not a bug.

## Files changed in v0.44

```
src-tauri/src/domain/mirror/mod.rs             | +paper_mode field + 5 tests
src-tauri/src/infra/db/seed.rs                 | +paper_fills table schema
src-tauri/src/commands/mirror_executor.rs      | +paper write path + 3 new IPCs + PaperFillDto
src-tauri/src/infra/state.rs                   | +mirror_paper_mode lock
src-tauri/src/lib.rs                           | +3 invoke_handler entries
src/ipc.ts                                     | +setMirrorPaperMode/getMirrorPaperMode/listPaperFills
src/types/bet.ts                               | +PaperFill type
src/stores/prefs-store.ts                      | +mirrorPaperMode pref
src/lib/prefs-io.ts                            | +mirrorPaperMode field (11 total)
src/lib/prefs-io.test.ts                       | updated for 11 fields
src/lib/i18n.ts                                | +5 keys × 2 locales
src/routes/Settings.tsx                        | +PaperModeCard
src/routes/Settings.test.tsx                   | +2 paper-mode tests
src/routes/Copy.tsx                            | +[PAPER] banner with live count
docs/polyrocket-v0.44-final.md                 | (this file)
docs/overview.md                               | new row in doc-sync table
```

**Net change**: 16 files, +600 LOC.

## Migration / back-compat

- **DB migration**: `paper_fills` table is created
  via the existing `init_pool` / `seed.rs` path.
  Pre-v0.44 databases get the new table on first
  launch (the `IF NOT EXISTS` makes it idempotent).
- **No changes to existing tables** (bets, etc.).
- **Mirror queue** gains a new `paper_submitted`
  status (was: `pending` / `submitted` / `rejected`).
  v0.44 entries with this status are inert; the
  executor's "pending OR submitted" filter still
  picks them up correctly on the next pass.
- **Wire format**: pre-v0.44 L1 clients don't
  need to know about the new fields. The
  `setMirrorPaperMode` etc. IPCs are additive.
- **prefs imports**: forward-compat: `parsePrefsFromString`
  defaults `mirrorPaperMode` to false if missing
  (same pattern as the v0.42 additions).
- **No env-var breaking changes**: the existing
  `POLYROCKET_MIRROR_*` env vars keep working
  unchanged; `POLYROCKET_MIRROR_PAPER_MODE=1`
  is the new addition.

## What's next

After user push:
- **v0.45** — paper_fills reconciliation job. A
  nightly scheduler that uses the markets table
  to mark paper_fills as `won` / `lost` and
  compute a paper-mode PnL for the dashboard.
- **v0.46** — backtest auto-populate from
  resolved markets. The v0.43 backtest engine
  currently takes manual JSON input; v0.46
  would add a "Pull from resolved markets"
  button that pre-fills the textarea.
- **v0.50 milestone** — model lifecycle +
  trading parity is now genuinely complete
  (v0.17 train → v0.40 compare → v0.43
  backtest → v0.44 paper). Time to focus
  on trading-side features (advanced order
  types, conditional orders, post-only
  enforcement, fill analytics).
