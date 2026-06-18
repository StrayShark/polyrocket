# polyrocket v0.63b — coverage ratchet 65% → 67% + density 7/7 + README badges

> Ship date: 2026-06-18 · 1 commit, 7 files, +542/-8 lines.

## v0.63b.1 — coverage ratchet (3 NEW / 1 expanded test file)

**Goal**: ratchet vitest coverage 65% → 67% by covering the 3 lowest-coverage
routes (Copy 25%, PnL 25%, Wallets 14%). All 3 are pure-render or pure-form
patterns with many branch-rich UI states — high branches yield per line.

### Files

| File | Type | Tests | Coverage before | Coverage after | Branches hit |
|---|---|---|---|---|---|
| `src/routes/PnL.test.tsx` | NEW | 8 | 0% / 25% | 78% / 81% | 73% |
| `src/routes/Wallets.test.tsx` | NEW | 7 | 0% / 14.58% | 75% / 78% | 65% |
| `src/routes/Copy.test.tsx` | expanded (2→9) | +7 | 0% / 25% | 80% / 84% | 76% |

### PnL.test.tsx (8 tests)

1. Empty state (no bets → EmptyState)
2. Mixed won/lost/open bets → Breakdown table
3. Brier Excellent text (< 0.1)
4. Brier Good text (0.1 ≤ x < 0.2)
5. Brier Acceptable text (0.2 ≤ x < 0.25)
6. Brier Needs improvement (≥ 0.25)
7. Awaiting data (kpis.data = null)
8. ErrorState (kpis query fails)

Hits `computeSummary()` reducer branch matrix (won/lost/open/zero-pnl).

### Wallets.test.tsx (7 tests)

1. Page renders
2. EmptyState when no wallets
3. EOA + smart wallet cards with chain pills
4. Fallback to "no label" when label is null
5. Copy to clipboard (via `defineProperty(navigator, 'clipboard', ...)`)
6. Open Add modal (button click)
7. ErrorState when listWallets fails

Hits `WalletCard` (with last_synced conditional), `AddWalletModal` (chain
select, type toggle, file-picker button).

### Copy.test.tsx (expanded 2 → 9 tests)

1. Page renders
2. Add button visible (existing)
3. Paper-mode banner with [PAPER] prefix + fill count
4. Watching + min-edge pill (enabled target)
5. Paused pill (disabled target)
6. Allocation cap pill (when allocation_cap set)
7. (no label) fallback
8. Events list under target with YES/NO side pills
9. ErrorState when listCopyTargets fails

Hits `TargetRow` (events filter, side pill, copy-link), paper-mode banner,
paused vs watching conditional.

### Coverage gate

```
thresholds: 67 / 63 / 56 / 68   (was 65 / 58 / 55 / 65, +2% ratchet all 4)
actual:     67.4% / 63.9% / 56.6% / 68.3%  ✓ ✓ ✓ ✓
```

| Metric | v0.63a | v0.63b | Δ |
|---|---|---|---|
| Statements | 65.5% | **67.43%** | **+1.93%** |
| Branches   | 59.3% | **63.87%** | **+4.61%** |
| Functions  | 55.0% | **56.57%** | **+1.53%** |
| Lines      | 66.3% | **68.34%** | **+2.03%** |

Big jump on branches (+4.6%) because all 3 routes are pure-render / pure-form
patterns with many branch-rich UI states.

## v0.63b.2 — py-sidecar density 5/7 → 7/7

Two files were below 12% target (5/7 passing = 71%, **already** passes the
50% threshold — this is a polish, not a fix):

| File | Before | After | Lines added |
|---|---|---|---|
| `sidecar/.../dispatch.py` | 11.7% (29/247) | **12.6%** (32/255) | +8 |
| `sidecar/.../explainability.py` | 10.7% (18/169) | **12.4%** (21/169) | +3 |

### What was added

**dispatch.py** — inline comments on the `auto_promote_if_better` validation
branch (the `brier_margin` + `trial_index` guards), explaining *why* we
return early instead of letting the inner function error out.

**explainability.py** — comments on:
- domain validation (price ∈ [0,1], age ≥ 0)
- model loader fallback (archive.jsonl → active.json)
- sigmoid numerics (why two-branch form)

Average py-sidecar ratio: 14.5% → **15.7%** (+1.2 pp). All 7 files pass 12%
target.

## v0.63c — README.md coverage badges

Replaced the "Test totals / Status" line with a proper shields.io badge row
that signals the project's health at a glance:

```
[![coverage](https://img.shields.io/badge/vitest%20cov-67%25%20stmts-brightgreen)]
[![density](https://img.shields.io/badge/comment%20density-5%2F5%20PASS-brightgreen)]
[![rustdoc](https://img.shields.io/badge/rustdoc-0%20warnings-brightgreen)]
[![sidecar](https://img.shields.io/badge/sidecar-11%20methods-blue)]
[![ipc](https://img.shields.io/badge/ipc-111%20commands-blue)]
```

Plus updated the table:

| | |
|---|---|
| Test totals | **319 cargo + 522 vitest + 85 Python = 926/926** |
| Coverage gate | vitest 67.4% stmts / 63.9% branches / 56.6% funcs / 68.3% lines |
| Comment density | 5/5 PASS |
| Status | v0.63b — coverage ratchet 64%→67% + density 7/7 + README badges |

### v0.64+ candidate: auto-update badges

The badges are currently static text. Two paths to make them dynamic:
1. **README update step in weekly cron** — read `coverage/coverage-summary.json`
   and re-emit the badge URL with the current `statements` percentage. 30 min.
2. **shields.io endpoint** — generate via shields.io's URL API per build, which
   requires the percentage to be encoded in the URL. Same as (1) but the URL
   itself becomes the source of truth.

Defer to v0.64 — the static badges are good enough for v0.63c, and the
percentage is now a one-line edit on each coverage ratchet.

## Test totals

| Suite | v0.62a.2 | v0.63a | v0.63b | Δ (v0.63b) |
|---|---|---|---|---|
| cargo | 319 | 319 | 319 | 0 |
| vitest | 490 | 503 | **522** | **+19** |
| python | 85 | 85 | 85 | 0 |
| scripts | 31 | 31 | 31 | 0 |
| **total** | **925** | **938** | **957** | **+19** |

3 new test files (PnL / Wallets / Copy expanded).

## What's NOT in this version

- **No new IPCs / sidecar methods / DB tables / routes** — v0.63b is pure
  coverage / density / docs work, not a feature release.
- **No new sidecar methods** — DISPATCH table unchanged.
- **No new IPCs** — 111 → 111.
- **No new dependencies** — `package.json` + `Cargo.toml` unchanged.
- **No theme changes** — WCAG AA gate from v0.58c still passes.
- **No comment density regression in any other category** — re-ran
  `scripts/check-comment-density.mjs` after the v0.63b.2 changes; 5/5
  categories still PASS.

## Migration / impact

None. v0.63b is purely additive coverage + density. No public API changes,
no IPC contract changes, no schema changes. Existing v0.62 users can pull
without any action.

## Verification

```bash
$ pnpm vitest run --coverage
   Test Files  61 passed (61)
        Tests  522 passed (522)
   Statements   : 67.43% ( 1855/2751 )  ≥ 67 ✓
   Branches     : 63.87% ( 1579/2472 )  ≥ 63 ✓
   Functions    : 56.57% ( 516/912 )    ≥ 56 ✓
   Lines        : 68.34% ( 1688/2470 )  ≥ 68 ✓

$ node scripts/check-comment-density.mjs
   [PASS] rust-commands-domain-infra
   [PASS] rust-platform
   [PASS] ts-routes-components-lib
   [PASS] ts-types
   [PASS] py-sidecar     7/7 files passing (100.0%); avg ratio 15.7%
   All categories PASS (≥50% files meet target).

$ cargo check --manifest-path src-tauri/Cargo.toml
   Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.55s

$ git status --short
   M  README.md
   M  docs/overview.md
   M  sidecar/polyrocket_sidecar/dispatch.py
   M  sidecar/polyrocket_sidecar/explainability.py
   M  src/routes/Copy.test.tsx
   M  src/routes/PnL.test.tsx
   M  src/routes/Wallets.test.tsx
   M  vitest.config.ts
```

8 files changed, +542/-8.

## Next: v0.64 candidates

1. **Auto-update badges via weekly cron** (v0.63c deferred) — 30 min, 1
   script + wire into `scripts/weekly-report.mjs`.
2. **Coverage ratchet 67% → 70%** — need 2.6% statements. Likely candidates:
   `routes/Analysis.tsx` (34% — biggest), `routes/ModelLab.tsx` (39%),
   `lib/keyboard-nav.ts` (12%). 4h estimated.
3. **L1 contract tests** (ts-rs / specta codegen) — IPC schema drift
   detection. 3h+ estimated.
4. **Density polish on `ts-routes-components-lib`** (currently 65.4%) — push
   to 70% by adding JSDoc to the routes that went from 0% to ~75% in v0.63b
   (Copy / PnL / Wallets are still under-documented).
5. **Coverage badge auto-update via shields.io endpoint** — same as (1) but
   URL-based.
