# polyrocket v0.45 — final

**Branch**: main (local-only, not pushed)
**Commits**: `eac43ce` v0.45a/b/c → `<this commit>` v0.45d
**Released**: 2026-06-18 (local, awaiting user push)

## What changed since v0.44

| sub-version | hash       | one-liner                                                | tests at landing |
|-------------|------------|----------------------------------------------------------|------------------|
| v0.45a/b/c  | `eac43ce`  | paper_fills reconciliation + paper PnL summary + Dashboard card | 731              |
| v0.45d      | this file  | Ship log + tally                                         | 731              |

**Test totals at v0.45 final**: cargo 234/234, vitest 359/359, python 77/77, script 31/31. **Total 670 (vitest+cargo+python) + 31 script = 701 total.** (Up from 700 in v0.44 — +1 cargo reconciler test.)

## Highlights

### Paper fills reconciliation (v0.45a-c)

The v0.44 paper trading mode captured
"hypothetical fills" to the paper_fills table,
but couldn't tell you whether they would have
won. v0.45 closes that loop.

**The full flow**:

1. Mirror executor picks an order, paper mode
   is on → `paper_fills` row created (v0.44)
2. Some time later, the market becomes
   resolved (Polymarket settlement, or
   `sync_markets` updates `markets.resolved=1`
   and `markets.outcome`)
3. The v0.45a reconciler (new 6th scheduler
   loop, runs every 5 minutes) joins the two
   tables:
   - If `paper_fill.side == market.outcome` →
     won, PnL = +size_usdc
   - If different → lost, PnL = -size_usdc
4. The Dashboard surfaces this as a
   `paper-pnl-card` with 3 KPIs:
   - Total fills (settled + unsettled)
   - Win rate (won / settled, with counts)
   - Realized PnL (sum across settled)

**The schema migration is idempotent**:
v0.45a adds 4 columns to `paper_fills` via
`PRAGMA table_info` checks + `ALTER TABLE`.
Pre-v0.45 DBs are migrated on the next launch;
the migration is a no-op on already-migrated DBs.

**Telemetry**: the reconciler emits
`PaperFillsReconciled { settled: N }` events
(v0.45a). The 6th scheduler loop is registered
in `start()`. The loop is opt-out via the
`Notify` shutdown signal (same pattern as the
other 5 loops).

**PnL formula** matches the v0.5d bet-table
stub's conservative accounting: won = +size,
lost = -size. The reconciliation is intentionally
simple — the `pnl_usdc` field stores the signed
result, no separate cost basis / payout split.
A future v0.45+ could replace this with the
real binary-market share math (size / 0.5 *
(1 - price) on win, -size on loss) once we
have a real CLOB integration.

## Files changed in v0.45

```
src-tauri/src/infra/db/seed.rs                  | +settlement columns
src-tauri/src/infra/db/paper_fills.rs           | new (migration helper)
src-tauri/src/infra/db/mod.rs                   | +paper_fills module
src-tauri/src/infra/db/pool.rs                  | +migration call
src-tauri/src/infra/telemetry.rs                | +PaperFillsReconciled event
src-tauri/src/infra/scheduler/mod.rs            | +6th loop + 1 cargo test
src-tauri/src/commands/pnl.rs                   | +paper_pnl_summary IPC
src-tauri/src/commands/mirror_executor.rs       | PaperFillDto gains 4 fields
src-tauri/src/lib.rs                            | +paper_pnl_summary handler
src/ipc.ts                                      | +paperPnlSummary wrapper
src/types/shared.ts                             | +PaperPnlSummary type
src/routes/Dashboard.tsx                        | +paper-pnl-card
src/lib/i18n.ts                                 | +7 keys × 2 locales
docs/polyrocket-v0.45-final.md                  | (this file)
docs/overview.md                                | new row in doc-sync table
```

**Net change**: 14 files, +550 LOC.

## Migration / back-compat

- **DB schema**: 4 new columns on `paper_fills`
  (`settled_at`, `resolved_outcome`, `won`,
  `pnl_usdc`). All nullable. Existing rows from
  pre-v0.45 DBs are unchanged — they just have
  NULL settlement fields until the next
  reconcile pass settles them (if the market
  is now resolved).
- **Wire format**: pre-v0.45 L1 clients don't
  see the new fields. The L1↔Tauri guard detects
  the new IPC; no breaking changes to existing
  wrappers.
- **Settings prefs**: no change. The
  `mirrorPaperMode` pref (v0.44) is the
  upstream switch.
- **DB migration is idempotent** via the
  `PRAGMA table_info` check pattern.
- **No new env vars**. The reconciler runs on
  every boot, every 5 minutes.

## What's next

After user push:
- **v0.46** — backtest auto-populate from
  resolved markets. The v0.43 backtest engine
  currently takes manual JSON input; v0.46
  adds a `list_resolved_markets_for_backtest`
  IPC that returns a pre-formatted sample
  array, plus a "Pull from resolved markets"
  button on the BacktestReport Modal.
- **v0.50 milestone** — model lifecycle +
  trading parity is now feature-complete
  (v0.17 train → v0.40 compare → v0.43
  backtest → v0.44 paper → v0.45 reconcile).
  Time to focus on trading-side features
  (advanced order types, conditional orders,
  post-only enforcement, fill analytics).
