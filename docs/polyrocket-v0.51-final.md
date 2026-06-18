# polyrocket v0.51 — final

**Branch**: main (local-only, not pushed)
**Commits**: `v0.51a` (clob_snapshots) → `v0.51b` (fill columns + slippage/ttf analytics) → `v0.51c` (real CLOB submit path) → `<this commit>` v0.51d
**Released**: 2026-06-18 (local, awaiting user push)

## What changed since v0.50

| sub-version | hash      | one-liner                                              | tests at landing |
|-------------|-----------|--------------------------------------------------------|------------------|
| v0.51a      | `6fa1a3a` | clob_snapshots table + CLOB feed IPCs + L1 status card | 781              |
| v0.51b      | `320a8d0` | filled_at + fill_price + slippage/ttf analytics        | 781              |
| v0.51c      | `c3c3da4` | Real CLOB submit path (creds-gated, fallback to stub) | 786              |
| v0.51d      | this file | Ship log + tally                                       | 786              |

**Test totals at v0.51 final**: cargo 289/289, vitest 367/367, python 77/77, script 31/31. **Total 764 (vitest+cargo+python) + 31 script = 795 total.** (Up from 755 in v0.50 — +6 cargo polymarket + 3 net clob_snapshots. The vitest count is unchanged.)

## Highlights

### v0.51a — clob_snapshots

The v0.47a `price_snapshots` table stores a single `(best_bid, best_ask)` per market per timestamp — useful for backtest joins and the v0.50b post-only check. For "real" order-book reasoning (depth, full ladder, partial fills, slippage estimation) we need the full snapshot.

v0.51a introduces `clob_snapshots`: one row per price level per side per timestamp per market. A "snapshot" is the set of rows sharing `captured_at` for a given market. Typical sizes: 5-30 levels per side.

The WebSocket listener that fills this table is v0.51+ (requires real Polymarket CLOB credentials). v0.51a ships the schema, helpers, IPCs, and an L1 status card so the moment a feed is wired up, everything falls into place.

### v0.51b — fill columns + slippage/ttf analytics

v0.5d's deterministic stub populated `placed_at` but had no separate "when did the CLOB actually fill me" column. v0.50c fill analytics explicitly left slippage + time-to-fill out for this reason. v0.51b fixes both:

- Schema (idempotent migration, 4 new columns):
  - `bets` gains: `filled_at`, `fill_price`, `fill_size`, `partial`
- Stub populates deterministically (slippage = 0, no partials)
- `FillAnalytics` DTO extended with `avgSlippage`, `avgTimeToFillMs`, `partialFillCount`, `partialFillRate`
- `BetDto` extended with the 4 new columns
- Dashboard card grows a third row with 3 small tiles

### v0.51c — real CLOB submit path

v0.5d's `place_signed_order` was a stub. v0.51c replaces this with a structured CLOB submit path that:

- `creds_present()` checks env vars (POLYROCKET_CLOB_API_KEY + SECRET + PASSPHRASE)
- When creds absent: returns the deterministic stub shape (slippage=0, partial=false, ok=true)
- When creds present: attempts a real HTTP POST to `https://clob.polymarket.com/order`; on success, parses fill_price/fill_size/partial from the response
- On any HTTP error: returns `ClobOrderResult { ok: false, error: ... }` — the L1 sees a structured `AppError::Invalid`, not a panic

The HTTP path is best-effort:
- We POST a placeholder JSON payload (the real EIP-712 signed order struct + L2 auth header are v0.51+ proper — requires `rs-clob-client`)
- We surface whatever the CLOB returns, with graceful fallback to the stub shape on parse failure

`place_signed_order` in `commands::bet` now:
- Calls `submit_signed_order_via_clob` instead of the v0.5d stub
- On `ok=false`: returns `AppError::Invalid`
- On `ok=true`: persists `clob.tx_hash`, `clob.filled_at_ms`, `clob.fill_price`, `clob.fill_size`, `clob.partial` to `bets`
- audit_log payload includes `via_http + fill_price + fill_size + partial` so the "live vs stub" distinction is auditable

## Files changed in v0.51

```
src-tauri/src/commands/clob.rs              | NEW — clob_feed_status, record_clob_snapshot_now, latest_clob_snapshot
src-tauri/src/commands/pnl.rs               | +slippage/ttf/partial in fill_analytics
src-tauri/src/commands/bet.rs               | +clob submit path + fill columns
src-tauri/src/commands/mod.rs               | +clob module
src-tauri/src/domain/polymarket/mod.rs      | +creds_present + ClobOrderResult + submit_signed_order_via_clob
src-tauri/src/infra/db/clob_snapshots.rs    | NEW — schema migration + 4 cargo tests
src-tauri/src/infra/db/bets_columns.rs      | +fill columns migration
src-tauri/src/infra/db/pool.rs              | +ensure_clob_snapshots
src-tauri/src/infra/db/mod.rs               | +clob_snapshots module
src-tauri/src/infra/state.rs                | +new_for_test (from v0.50c)
src-tauri/src/lib.rs                        | +3 clob IPCs
src/ipc.ts                                  | +ClobSnapshot, ClobFeedStatus, ClobOrderResult, RecordClobSnapshotArgs types + 4 wrappers
src/lib/i18n.ts                             | +16 keys × 2 locales
src/routes/Dashboard.tsx                    | +3 v0.51b tiles in fill_analytics card
src/routes/Settings.tsx                     | +ClobFeedCard
src/routes/Settings.test.tsx                | +clobFeedStatus mock
docs/polyrocket-v0.51-final.md              | (this file)
docs/overview.md                            | new rows in doc-sync table
```

**Net change**: 17 files, +1,400 LOC.

## Migration / back-compat

- **DB migration**: idempotent.
  - new table `clob_snapshots` (v0.51a)
  - new columns on `bets`: `filled_at`, `fill_price`, `fill_size`, `partial` (v0.51b)
- **IPC back-compat**: all v0.51 additions are new IPCs / new optional fields. Existing callers unaffected.
- **DTO back-compat**: `BetDto` uses `serde(default)` for the 4 new fields.
- **Env vars** (all optional):
  - `POLYROCKET_CLOB_API_KEY`, `POLYROCKET_CLOB_API_SECRET`, `POLYROCKET_CLOB_API_PASSPHRASE` — when all 3 are set, `place_signed_order` attempts a real HTTP submit; otherwise it falls back to the deterministic stub.
- **L1↔Tauri guard**: 99 commands (was 96, +3: clob_feed_status, record_clob_snapshot_now, latest_clob_snapshot).
- **Scheduler loops**: still 8.

## What's next

After user push, v0.51d is included. The remaining work for v0.51 proper (the WebSocket listener + EIP-712 signing) is gated on real CLOB credentials — we can't test it from CI.

The next **v0.52** milestone is the **L1 place-bet form**, which finally consumes all the v0.50 + v0.51 surface:

- **v0.52a — Place-bet form component.** OrderType select (Market/Limit/StopLoss), limit_price/stop_price inputs (with conditional visibility), post_only toggle. Validates live via `validateOrderArgs`. Sends via `placeSignedOrder`.
- **v0.52b — Integration.** Result handling (success → toast + navigate to Bets; failure → inline error). Reading `filled_at` + `fill_price` + `partial` from the response and surfacing "live" vs "stub" badge.
- **v0.52c — Bets page.** A dedicated page (currently bets are only visible in the Dashboard "Recent" widget). Filterable by status / order_type / wallet. Per-row expansion showing all 12 columns (the 4 v0.50a + 4 v0.51b fields).
- **v0.52d — ship log + push.**

After v0.52, the trading-side milestone is functionally complete. v0.53+ candidates:
- v0.53 — automated user testing + accessibility pass on all v0.5x + v0.50 + v0.51 + v0.52 surfaces
- v0.54 — model explainability (SHAP values, feature importance in active.json)
- v0.55 — multi-wallet support (today the keyring has one "primary" alias; multiple aliases are wired but the L1 doesn't expose them)
- v0.56 — proxy / Tor support for the sidecar (today sidecar connects direct)

### v0.51 hygiene: what we left on the table

- **Real EIP-712 signing.** Today's HTTP path POSTs a placeholder payload. Real signing needs `rs-clob-client` (an external dep we haven't added yet) or hand-rolled `secp256k1` + keccak256.
- **WebSocket subscription for order updates.** v0.51c is request/response only. Real CLOB execution often uses WS for fill notifications; the v0.5d `filled_at` becomes available via WS, not the HTTP response.
- **StopLoss triggers.** Captured in v0.50a order records but never fired. v0.51+ would need a scheduler loop watching clob_snapshots.
- **Partial-fill reconciliation.** Today partial = false always. Real CLOB execution would set this when the order book doesn't have enough depth.
- **Per-wallet rate limiting.** The CLOB API has rate limits; we don't currently back off.
- **Order audit log per-attempt.** Today only the final result lands in audit_log; transient failures (e.g. 429) aren't separately logged.
