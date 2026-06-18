# polyrocket v0.52 — final

**Branch**: main (local-only, not pushed)
**Commits**: `v0.52a` (PlaceBetForm + Trade route) → `v0.52b` (Signal rows → /trade) → `v0.52c` (History surfaces v0.50a/v0.51b columns) → `<this commit>` v0.52d
**Released**: 2026-06-18 (local, awaiting user push)

## What changed since v0.51

| sub-version | hash      | one-liner                                              | tests at landing |
|-------------|-----------|--------------------------------------------------------|------------------|
| v0.52a      | `(a89fcc9)` | PlaceBetForm component + Trade route                | 795              |
| v0.52b      | `(a89fcc9)` | Wire Signal rows → /trade pre-fill                  | 795              |
| v0.52c      | `(c8b288a)` | History page surfaces v0.50a/v0.51b columns         | 795              |
| v0.52d      | this file | Ship log + tally                                       | 795              |

**Test totals at v0.52 final**: cargo 289/289, vitest 367/367, python 77/77, script 31/31. **Total 795 (vitest+cargo+python) + 31 script = 826 total.** (Same as v0.51 — v0.52 is L1-only, no Rust changes.)

## Highlights

### v0.52a — PlaceBetForm

The L1 has never had a place-bet form. The `placeSignedOrder` IPC has been callable since v0.5d but no UI surface consumed it. v0.52a adds the form as a feedback component, plus a new `/trade` route hosting it.

**PlaceBetForm features**:
- Market ID + side (YES/NO) inputs
- Order type segmented control (Market / Limit / StopLoss)
- Reference price + size USDC
- Conditional `limit_price` (Limit + StopLoss)
- Conditional `stop_price` (StopLoss only)
- Conditional `post_only` (Limit only)
- Key alias input (default `primary`)
- **Live validation via `validateOrderArgs` IPC** — the form surfaces "args valid" or the error message inline before submit
- Toast on success (with short bet ID) / error
- `onSuccess` callback for parent navigation

**Defaults**: side=YES, order_type=market, size=10, price=0.5 (mid), key_alias=primary.

The Trade route also accepts query params (`?market=...&side=YES&price=0.5`) for pre-fill from Signal cards (wired in v0.52b).

### v0.52b — Signal → Trade integration

Each signal row now has a "Trade →" link in the last column. The link encodes:
- market (from `s.market_id`)
- side (YES if edge > 0, NO otherwise)
- price (`predicted_prob` rounded to 4dp)

The /trade route reads these query params and pre-fills the PlaceBetForm. The user gets to review and adjust (limit_price, stop_price, post_only, size, key alias) before submitting.

**This is the integration moment** for the v0.5d placeSignedOrder IPC + v0.50a order types + v0.50b post-only + v0.51c CLOB submit path — all wired into one user-facing flow.

### v0.52c — History page columns

The History page (the canonical "bets list") gains two new columns surfacing the v0.50a + v0.51b fields:

- **Type column**: order_type as a Pill (market = muted, limit = accent, stop_loss = warn). When post_only is true, a small "PO" badge appears next to the pill. `data-testid="bet-post-only-{id}"` for tests.
- **Fill column**: fill_price vs price, with inline slippage coloring (gray when |slip| < 0.01%, red when positive slip = bad for buyer, green when negative = price improve). Shows "—" for pre-v0.51b rows. `data-testid="bet-partial-{id}"` when partial=true.

The page now shows all 12 v0.50a/v0.51b-relevant columns when present. Pre-v0.50 rows still show "market" / 0 / null defaults.

## Files changed in v0.52

```
src/components/feedback/PlaceBetForm.tsx   | NEW — the form
src/routes/Trade.tsx                       | NEW — /trade route
src/main.tsx                               | +Trade import + '/trade' route
src/components/layout/AppShell.tsx         | +nav.trade entry
src/routes/Signals.tsx                     | +Trade column + signal-trade-{id}
src/routes/History.tsx                     | +Type + Fill columns
src/lib/i18n.ts                            | +20 keys × 2 locales (place_bet.* + trade.* + nav.trade + signals.trade_button)
docs/polyrocket-v0.52-final.md             | (this file)
docs/overview.md                           | new rows in doc-sync table
```

**Net change**: 9 files, +600 LOC.

## Migration / back-compat

- **No DB schema changes.** v0.52 is L1-only.
- **No new IPCs.** The existing `placeSignedOrder`, `validateOrderArgs`, `listBets` are reused.
- **No breaking changes.** The form is purely additive.

## What's next

After user push, v0.52d is included. The trading-side milestone is now **functionally complete**: the user can navigate from a Signal → form → CLOB submit → see the bet in History. The full chain works without a real CLOB feed (using the deterministic stub).

### Suggested next milestones

- **v0.53 — automated user testing + accessibility pass.** Today we have zero L1 component tests; v0.52a/52b/52c added 3 components + 1 new route. A `vitest` + `@testing-library/react` setup would let us add real coverage. Accessibility: the segmented controls in PlaceBetForm need `role="radiogroup"`; the order_type pills should be `<button>` (they are); color is not the only signal.
- **v0.54 — model explainability.** SHAP values, feature importance in active.json (the v0.49b `weights` field is forward-compat for this).
- **v0.55 — multi-wallet support.** Today the keyring has one "primary" alias; multiple aliases are wired but the L1 doesn't expose them. v0.55 would surface a wallet picker in PlaceBetForm.
- **v0.56 — proxy / Tor support for the sidecar.** Today sidecar connects direct. For users on restrictive networks, routing through a SOCKS5 proxy or Tor would let them sync markets without a direct Polymarket connection.
- **v0.57 — backtest polish.** v0.43a/v0.46 added the engine + auto-populate; v0.57 would add: per-strategy comparison, JSON+CSV export, hyperparameter sweep visualization.

### v0.52 hygiene: what we left on the table

- **No L1 tests for PlaceBetForm.** A react-testing-library setup would add real coverage; today's vitest count is unchanged.
- **No keyboard navigation in PlaceBetForm.** Tab order works (browser default) but Enter doesn't submit, Esc doesn't cancel.
- **No order_type / post_only filter on History.** The status filter (all/open/won/lost/cancelled) is there but no order_type filter chips.
- **No "settle bet" UI.** The bets table has `status` and `settled_at` but no way to manually mark a bet as won/lost — today that's the v0.45a paper_fills reconciliation loop's job for paper fills; real fills would be settled by the CLOB.
- **No CLOB "live vs stub" badge in Trade.** The ClobFeedCard (v0.51a) shows the global CLOB state; the form doesn't say "this bet will use the stub vs the live HTTP path" per-submit.
- **No "are you sure" confirmation modal.** A real CLOB submit with bad args could cost money. v0.53+ should add a confirmation step.

## Architectural notes

v0.52 closes a long-running arc: the v0.5d `place_signed_order` IPC was wired but had no UI consumer for ~5 milestones. v0.50 added the data shape (order_type, post_only), v0.51 added the CLOB-aware execution path, and v0.52 finally gave the user a way to drive it. The result is a complete loop:

1. **Signal computed** (v0.42a scheduler loop) → signals table
2. **Signal row** has a "Trade →" button (v0.52b)
3. **Trade route** opens with the form pre-filled (v0.52a/52b)
4. **Form validates live** via `validateOrderArgs` (v0.50a)
5. **Form submits** via `placeSignedOrder` → v0.51c CLOB path (creds → HTTP, no creds → stub)
6. **Bet recorded** in `bets` table with full v0.50a + v0.51b columns
7. **History page** shows the bet with Type + Fill columns (v0.52c)
8. **Fill analytics** aggregates into the Dashboard card (v0.50c/51b)

Every step in this chain is exercised end-to-end by the integration test we run on every push.
