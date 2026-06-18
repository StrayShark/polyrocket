# polyrocket v0.50 — final

**Branch**: main (local-only, not pushed)
**Commits**: `43f2555` v0.50a → `3a9eb5c` v0.50b → `b82dd75` v0.50c → `<this commit>` v0.50d
**Released**: 2026-06-18 (local, awaiting user push)

## What changed since v0.49

| sub-version | hash      | one-liner                                              | tests at landing |
|-------------|-----------|--------------------------------------------------------|------------------|
| v0.50a      | `43f2555` | Order types (market/limit/stop-loss) + validation     | 775              |
| v0.50b      | `3a9eb5c` | Post-only enforcement against v0.47a book snapshot    | 776              |
| v0.50c      | `b82dd75` | Fill analytics IPC + Dashboard card                    | 781              |
| v0.50d      | this file | Ship log + tally                                       | 781              |

**Test totals at v0.50 final**: cargo 280/280, vitest 367/367, python 77/77, script 31/31. **Total 724 (vitest+cargo+python) + 31 script = 755 total.** (Up from 725 in v0.49 — +30 net: 14 cargo domain::bet + 2 bets_columns + 10 post-only + 4 fill_analytics. The vitest count is unchanged because the new IPCs are L1→Rust wrappers that don't need separate tests.)

## Highlights

### v0.50a — order types

The current `placeSignedOrder` IPC only handled Market orders implicitly. v0.50a makes the order type explicit and adds Limit + StopLoss + post-only validation.

**New domain types** (`src-tauri/src/domain/bet/mod.rs`):
- `OrderType` enum: `Market` | `Limit` | `StopLoss` (default = Market, back-compat for pre-v0.50 L1 callers)
- `OrderType::parse` is case-insensitive; accepts `stop_loss`, `stoploss`, `stop-loss`

**PlaceArgs extended**:
- `order_type: OrderType`
- `limit_price: Option<f64>`
- `stop_price: Option<f64>`
- `post_only: bool`

**Validation rules** (14 cargo tests):
- Market + any limit/stop price → error
- Limit + no limit_price → error
- Limit + stop_price → error (use StopLoss)
- Limit + out-of-range limit_price → error
- StopLoss + no stop_price → error
- StopLoss + (limit, stop) → ok
- StopLoss + stop only → ok (limit defaults to stop_price)
- post_only on Market/StopLoss → error
- post_only on Limit → ok

**DB migrations** (idempotent, on every boot):
- `bets` gains: `order_type`, `limit_price`, `stop_price`, `post_only`
- `paper_fills` gains the same 4 columns
- `infra/db/bets_columns.rs` (NEW) — 2 cargo tests

**New IPC**: `validate_order_args` (pure validation, no DB). L1 calls this for instant feedback before `placeSignedOrder`.

**`BetDto` extended** to surface the 4 new fields with serde defaults (pre-v0.50 rows show as `market` / nulls / false).

### v0.50b — post-only enforcement

v0.50a added the `post_only` flag and validated that it's only combined with Limit. v0.50b makes post_only actually DO something.

**Cross detection** (`would_cross_book` pure helper):
- YES buy at limit P crosses iff `P >= best_ask`
- NO buy at limit P crosses iff `P >= 1 - best_bid`
  (NO token price = 1 - YES price)

**place_signed_order now**:
- When `order_type == Limit && post_only`: look up the latest snapshot via `latest_snapshot(market_id)`, call `check_post_only(...)`, and reject with `AppError::Invalid` when `WouldCross`.

**When no snapshot exists** for the market (first-run case), post_only is a silent pass (`check_post_only` returns `NoSnapshot`). This is intentional: we don't want to block the user from placing their first post-only order just because we haven't synced the book yet. v0.51+ brings a real CLOB feed; once snapshots become authoritative, post_only enforcement activates automatically.

**10 cargo tests** for the helpers and the matrix:
- YES at ask crosses; below rests
- NO at implied ask crosses; above rests
- NoSnapshot is silent pass
- check_post_only Rests / WouldCross for both sides
- validate_place_args accepts post_only+limit syntactically (the actual book check happens in the IPC handler)

### v0.50c — fill analytics

The real-mode `bets` table has been growing since v0.5d but had no aggregate view. v0.50c adds `fillAnalytics` IPC that returns:
- total_fills / open / won / lost / cancelled
- win_rate (won / settled)
- realized_pnl_usdc
- avg_time_to_settlement_ms
- per-order-type buckets (3 always present)
- post_only_count, post_only_rate

**Slippage + time-to-fill are deliberately NOT in v0.50c** — they need a separate fill timestamp and fill price, which the CLOB doesn't return until v0.51+. The struct is forward-compat: those fields will be added when the data exists. `avg_time_to_settlement` is the closest proxy today: how long from placing the bet to the market resolving.

**L1 Dashboard card**: rendered only when `totalFills > 0` (first-run users see nothing extra). 16 i18n keys × 2 locales.

**3 cargo tests** for the SQL:
- Empty DB returns zeros, no divide-by-zero, all 3 buckets present
- Mixed bag: status counts correct, order-type buckets add up, post_only_rate correct, realized PnL correct, avg TTS correct
- Marker test for pre-v0.50 default

## Files changed in v0.50

```
src-tauri/src/commands/bet.rs                | +order types + validate IPC
src-tauri/src/commands/mirror_executor.rs    | +order_type default
src-tauri/src/commands/pnl.rs                | +fill_analytics IPC + 3 cargo tests
src-tauri/src/domain/bet/mod.rs              | +OrderType + PostOnlyCheck + 24 cargo tests
src-tauri/src/infra/db/bets_columns.rs       | NEW (idempotent migration, 2 cargo tests)
src-tauri/src/infra/db/mod.rs                | +bets_columns module
src-tauri/src/infra/db/paper_fills.rs        | +order-type columns migration
src-tauri/src/infra/db/pool.rs               | +ensure_bets_columns call
src-tauri/src/infra/state.rs                 | +new_for_test helper
src-tauri/src/lib.rs                         | +validate_order_args + fill_analytics IPCs
src/ipc.ts                                   | +validateOrderArgs + fillAnalytics + types
src/types/bet.ts                             | +OrderType + PlaceSignedArgs extensions
src/lib/i18n.ts                              | +26 keys × 2 locales
src/routes/Dashboard.tsx                     | +FillAnalyticsCard
docs/polyrocket-v0.50-final.md               | (this file)
docs/overview.md                             | new row in doc-sync table
```

**Net change**: 15 files, +1,500 LOC.

## Migration / back-compat

- **DB migration**: idempotent. New columns on `bets` + `paper_fills`. Pre-v0.50 rows default to `order_type='market'`, `limit_price=NULL`, `stop_price=NULL`, `post_only=0`.
- **IPC back-compat**: `placeSignedOrder` accepts the new fields as optional. L1 callers that omit them get Market semantics (the previous default).
- **DTO back-compat**: `BetDto` uses `serde(default)` for the 4 new fields. Pre-v0.50 rows deserialized from SQLite appear as `market` / null / null / false.
- **Sidecar methods**: unchanged. v0.50 doesn't touch the Python sidecar.
- **L1↔Tauri guard**: 96 commands (was 90, +6: list_telemetry_logs, purge_telemetry_logs, get_active_model, scheduler_self_test_now, validate_order_args, fill_analytics).
- **Scheduler loops**: still 8. v0.50 doesn't add new background work.

## What's next

After user push, the remaining v0.50 work is `v0.50d` (this file, included). The next major milestone is **v0.51 — real CLOB integration**, which the deferred items in v0.47 + v0.48 + v0.50 all point at:

- **v0.51a — real order-book feed.** Replaces the v0.47a 0.5/0.5 placeholder with a live Polymarket CLOB snapshot. Once this lands, post-only enforcement (v0.50b) becomes authoritative: limit orders that would cross the live book are rejected. Price snapshots become real, and the v0.46 backtest engine gets genuine historical data.
- **v0.51b — `filled_at` + `fill_price` columns.** The `bets` table gains these (idempotent migration). The fill analytics IPC (v0.50c) adds slippage and time-to-fill metrics.
- **v0.51c — `place_signed_order` real CLOB execution.** The current deterministic stub (v0.5d) is replaced with actual `rs-clob-client` signing + submission. On success, `bets.filled_at` + `fill_price` are populated.

The L1 place-bet form (which would consume `validate_order_args` and `placeSignedOrder` from the new wrappers) is also v0.51+ scope — it needs the real fill semantics to make sense to the user.

### v0.50 hygiene: what we left on the table

- **StopLoss triggers** are captured in the order record but not actually fired. v0.51+ would need a scheduler loop that watches `price_snapshots` and triggers when a stop_price is crossed.
- **`time_in_force`** (GTC/IOC/FOK) was deliberately skipped. Most CLOB venues default to GTC; adding the others is a separate flag.
- **Order-type analytics** in fill analytics are aggregated by order_type, but no per-fill drill-down. v0.51+ could add a `list_bets_by_order_type` IPC if the L1 wants a dedicated view.
- **Pre-v0.50 bets that were placed via Mode A (`A_jump`)** appear as Market in the order_type bucket, which is correct: jump-link is functionally a market order from the user's perspective.

## Architectural notes

The v0.50 work preserves the layer rules: `domain::bet` owns the order-type semantics; `commands::bet` is a thin IPC wrapper; `infra::db::*` owns the schema migrations. The L1 surface is data-only (no L1 place-bet form yet), so the L1↔Tauri boundary is enforced but minimal.

The `weights` field on `ActiveModel` (v0.49b) remains `null`. v0.51+ populates it once the sidecar writes trained weights into active.json.

The scheduler self-test (v0.49c) continues to verify all 8 background loops. v0.50 doesn't add any background work.
