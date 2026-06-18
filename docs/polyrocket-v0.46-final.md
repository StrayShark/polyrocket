# polyrocket v0.46 — final

**Branch**: main (local-only, not pushed)
**Commits**: `b5361b4` v0.46a/b → `<this commit>` v0.46c
**Released**: 2026-06-18 (local, awaiting user push)

## What changed since v0.45

| sub-version | hash       | one-liner                                                | tests at landing |
|-------------|------------|----------------------------------------------------------|------------------|
| v0.46a/b    | `b5361b4`  | Backtest auto-populate from resolved markets + L1 button | 736              |
| v0.46c      | this file  | Ship log + tally                                         | 736              |

**Test totals at v0.46 final**: cargo 236/236, vitest 360/360, python 77/77, script 31/31. **Total 673 (vitest+cargo+python) + 31 script = 704 total.** (Up from 701 in v0.45 — +1 cargo + +1 vitest + 1 carryover from v0.44d = +2 net.)

## Highlights

### Backtest auto-populate from resolved markets (v0.46a–b)

The v0.43 backtest engine shipped with a JSON
textarea for samples. v0.46 closes the "how do
I get data into this?" loop with a one-click
pre-fill from the resolved markets table.

**The flow**:
1. Open BacktestReport modal (v0.43d)
2. Click "Pull from resolved markets" (new in v0.46b)
3. The L1 calls `list_resolved_markets_for_backtest`
4. The Rust IPC queries `markets` where
   `resolved=1 AND outcome IS NOT NULL`
5. Each market becomes a sample:
   - `price` = 0.5 (degenerate, see below)
   - `market_age_hours` = 24 (the "predict 1 day
     before close" convention)
   - `outcome` = YES → 1.0, NO → 0.0
   - `label` = the market question
6. The textarea is replaced with the JSON array
7. Click "Run" to get the Brier mean

**Known limitation (documented)**:
v0.46 has no historical price snapshots, so
`price=0.5` is a degenerate default. The model
should at least beat 0.5 (random guessing) on
settled markets as a sanity check. v0.46+ could
add a price-snapshot table to enable real
backtests.

This is documented in three places:
- The IPC doc comment (Rust side)
- The Pull button hint text (i18n)
- The L1 wire comment

The user can still edit the textarea before
clicking Run if they have real prices.

## Files changed in v0.46

```
src-tauri/src/commands/market.rs              | +ResolvedMarketSample + IPC + 1 test
src-tauri/src/lib.rs                          | +IPC registration
src/ipc.ts                                    | +ResolvedMarketSample + wrapper
src/components/feedback/BacktestReport.tsx    | +Pull button + limit input + handler
src/components/feedback/BacktestReport.test.tsx | +1 test
src/lib/i18n.ts                               | +5 keys × 2 locales
docs/polyrocket-v0.46-final.md                | (this file)
docs/overview.md                              | new row in doc-sync table
```

**Net change**: 8 files, +350 LOC.

## Migration / back-compat

- **No DB schema changes.** The IPC reads from
  the existing `markets` table.
- **No new env vars.**
- **No changes to existing IPCs.**
- **Pre-v0.46 L1 clients** still work — the
  Pull button is opt-in; the JSON textarea is
  still the primary input.
- **The IPC is opt-in on the L1**: the modal
  doesn't fetch resolved markets on mount.
  The user clicks the button when they want
  to pre-fill.

## What's next

After user push:
- **v0.50 milestone** — model lifecycle +
  trading parity is now feature-complete:
  - v0.17 train
  - v0.18 promote
  - v0.19 history
  - v0.20 rollback
  - v0.21 bulk promote
  - v0.22 Brier chart
  - v0.23 promote-if-better
  - v0.24 per-trial badges
  - v0.25 promote-all-4
  - v0.28 background auto-promote
  - v0.33-0.34 archive + archive modal
  - v0.39 OS notifications
  - v0.40 multi-model comparison
  - v0.41 per-promotion reason
  - v0.42 telemetry + governance
  - v0.43 backtest engine
  - v0.44 paper trading
  - v0.45 paper reconciliation
  - v0.46 backtest auto-populate

  All gaps from the v0.40 deferred list are
  now closed. The next milestone is trading-
  side:
  - Advanced order types (limit, stop-loss,
    post-only enforcement)
  - Conditional orders (price/Brier triggers)
  - Fill analytics (slippage, time-to-fill,
    partial-fill rate)
  - Order book depth visualization
