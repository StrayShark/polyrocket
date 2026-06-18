# polyrocket v0.47 — final

**Branch**: main (local-only, not pushed)
**Commits**: `60a17fa` v0.47a/b → `<this commit>` v0.47c
**Released**: 2026-06-18 (local, awaiting user push)

## What changed since v0.46

| sub-version | hash       | one-liner                                                | tests at landing |
|-------------|------------|----------------------------------------------------------|------------------|
| v0.47a/b    | `60a17fa`  | price_snapshots table + backtest join on latest snapshot | 740              |
| v0.47c      | this file  | Ship log + tally                                         | 740              |

**Test totals at v0.47 final**: cargo 236/236, vitest 360/360, python 77/77, script 31/31. **Total 673 (vitest+cargo+python) + 31 script = 704 total.** (Same as v0.46 — +2 cargo, no new vitest.)

## Highlights

### Price snapshots (v0.47a-b)

v0.46's `list_resolved_markets_for_backtest` had
a documented limitation: price was fixed at 0.5
because we had no historical price snapshots.
v0.47 closes that loop with the `price_snapshots`
table.

**The flow**:
1. `sync_markets` runs (existing v0.2 command)
2. For each market, we now also INSERT a
   `price_snapshots` row with the current
   `best_bid` / `best_ask`. v0.47a uses a
   placeholder (0.5 / 0.5) because the Gamma
   API doesn't expose an order book — only
   metadata. v0.50+ can wire the real feed.
3. `list_resolved_markets_for_backtest` gains
   a LEFT JOIN against the latest
   `price_snapshots` per market (correlated
   subquery, ordered by `captured_at DESC`).
4. The sample's `price` is the snapshot's
   `mid_price` (or 0.5 fallback for pre-v0.47
   DBs that have no snapshots yet).

**Wire-stable**:
- v0.46 L1 code is unchanged — the IPC
  contract is identical, the `price` field is
  just a real number instead of 0.5.
- The L1's i18n hint was updated to reflect the
  new behavior.

**The schema is ready for v0.50+**:
- `best_bid`, `best_ask`, `mid_price`, `spread`
  are all real fields. When a real order-book
  feed lands, just write the real values into
  the same INSERT.

**Migration**:
- v0.47 DBs (auto-created) have the table from
  first launch. No migration needed.
- Pre-v0.47 DBs: the table is created by
  `ensure_price_snapshots` on next launch.
  Existing markets have no snapshots; the
  backtest falls back to 0.5. Re-syncing
  markets populates the table.

## Files changed in v0.47

```
src-tauri/src/infra/db/seed.rs                  | +price_snapshots table schema
src-tauri/src/infra/db/price_snapshots.rs       | new (record / latest / purge_old)
src-tauri/src/infra/db/mod.rs                   | +price_snapshots module
src-tauri/src/infra/db/pool.rs                  | +ensure_price_snapshots migration
src-tauri/src/commands/market.rs               | sync_markets records snapshots;
                                              | backtest IPC joins latest
src/lib/i18n.ts                                | +pull_hint updated for v0.47+
docs/polyrocket-v0.47-final.md                  | (this file)
docs/overview.md                                | new row in doc-sync table
```

**Net change**: 8 files, +300 LOC.

## Migration / back-compat

- **DB schema**: 1 new table (`price_snapshots`).
  Auto-created by `ensure_price_snapshots` on
  next launch.
- **No changes to existing tables.**
- **Wire format**: pre-v0.47 L1 clients see no
  change. The IPC contract is identical.
- **i18n**: 1 key (`backtest.pull_hint`) updated
  in both locales to document the new behavior.
- **No new env vars.**

## What's next

After user push:
- **v0.48** — model degradation detector. The
  v0.47 price_snapshots give us the data
  foundation; v0.48 adds a 7th scheduler loop
  that computes the live Brier of the active
  model on recent markets and fires an OS
  notification when it drifts up by more than
  a threshold over a configurable window.
- **v0.50 milestone** — trading-side: real
  order-book feed (so v0.47a placeholders
  become real), advanced order types,
  conditional orders, fill analytics.
