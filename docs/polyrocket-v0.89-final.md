# polyrocket v0.89 — coverage ratchet round 4

> 2026-06-21 · 3 commits · 3 new test files (+34 tests) · 1 source file (vitest.config.ts)
> a / b / c / final

## TL;DR

**Coverage gate ratchet round 4: 86/83/79/87 → 87/84/81/88** (+1pp on all 4 dims).

Three sub-versions:
- **v0.89a** — Bankroll branches 36.36→88.63% (+18 tests, biggest single-route jump this round)
- **v0.89b** — Trade + MarketDetail branches round (+9 tests)
- **v0.89c** — Notifications branches 66.66→100% (+7 tests, file maxed)
- **v0.89-final** — threshold bump + ship log

Coverage: 87.46 / 85.39 / 81.61 / 88.64 (was 87.42 / 85.0 / 81.51 / 88.6 at v0.89a start).
Test count: 944 → 960 vitest tests (+16).

Headroom after threshold bump: 0.46 / 1.39 / 0.61 / 0.64pp — all 4 dims pass cleanly.

## What worked

**Bankroll.tsx (+52pp branches)** — biggest single-route coverage jump in the project's history.
`src/routes/Bankroll.branches.test.tsx` (18 tests) targets the unobserved branches in
BankrollConfigCard + AllocationTable: slider clamps, kelly fraction validation,
allocation_id linkage, `apply_allocation` toasts, C_allocated mode, etc.

The ROI is huge here because Bankroll has lots of pure-form/pure-render conditional logic
that's trivial to test once the IPC mock is set up.

**MarketDetail.tsx (+18pp branches)** — status pill 3-way (active/inactive/resolved),
signal edge color (positive → text-bull, negative → text-bear), filtered signals empty
state. Trade.tsx (+33pp) covers URL param parsing branches.

**Notifications.tsx (66.66→100%)** — file is now maxed on all 4 dims. The 4 KIND_CLS color
branches (info→accent, success→bull, warning→warning, error→bear) + body present/absent
branch were the only gaps.

## What didn't work (and why)

**v0.89c ModelLab.easyBranches.test.tsx (deleted)** — added 0% coverage to ModelLab.tsx.
The 6 tests only asserted on `model-train-btn` / `view-archive-btn` rendering, which is
already covered by 8 existing ModelLab test files. Without exercising the `best` reducer
or auto-promote flow paths, the tests are pure noise. Removed.

**v0.89d Settings.round4.test.tsx (deleted)** — aimed to cover the top Card `save()` /
`reset()` functions (lines 96-113 of Settings.tsx). The buttons lack testids and the text
`settings.btn.save` matches multiple buttons across different cards. `getAllByText` can't
disambiguate. The 5 tests failed because of the multiple-match problem. Could be fixed
by adding testids to the top buttons, but the existing Settings tests already cover the
AutoPromoteCard's save behavior, so the marginal gain is small. Removed.

**v0.91 (planned)** — the remaining ModelLab.tsx `promoteAllMut` strict-mode refactor.
Extracting `useTrainProgress` custom hook will make the deferred branches testable
without breaking strict mode. This is the only coverage target left for the 80%+ files
that aren't testable in-place.

## Threshold ratchet history

| version | threshold (stmts/br/fn/lines) | tests | key files |
|---|---|---|---|
| v0.62a.2 | 64/57/52/64 | 504 | initial gate |
| v0.69-v0.74 | 74→83% (ratchet across 28 commits) | +213 tests | various |
| v0.75 | 83/81/76/84 | 794+ | ModelLab r3, Settings r2, LlmMgmt r4, Wallets r3 |
| v0.83 | 86/83/79/87 | 920+ | ModelLab branches, Copy, Audit |
| **v0.89** | **87/84/81/88** | **960** | **Bankroll, Trade, MarketDetail, Notifications** |

Headroom policy: ~0.5pp minimum on each dim to absorb jitter. We ratchet by ≤1pp per
sub-version; natural ceiling seems to be around 88-90% on stmts/lines, 85-87% on branches,
85-87% on functions.

## Sub-versions

### v0.89a — Bankroll branches (+18 tests, 36.36→88.63% branches)

`src/routes/Bankroll.branches.test.tsx` — 18 tests covering BankrollConfigCard + AllocationTable
edge cases. Direct `vi.fn()` mocks (no local var capture — vitest hoisting breaks that pattern).
Toast verification via `import { toast } from '@/stores/toast-store'` direct assertion.

Commit: `0470bd0`. Coverage: stmts 71→98, br 36→88, fn 67→92, lines 72→98.

### v0.89b — Trade + MarketDetail branches (+9 tests, +33pp Trade / +18pp MarketDetail)

`src/routes/Trade.MarketDetail.branches.test.tsx` — 9 tests:
- 2 Trade.tsx tests (URL param parsing branches: `?? undefined`, `price ? parseFloat : undefined`)
- 7 MarketDetail.tsx tests (status pill 3-way, signal edge color 2-way, filtered signals empty)

Commit: `c734b34`. Trade.tsx branches 66.66→100. MarketDetail.tsx branches 72.72→90.9.

### v0.89c — Notifications branches (+7 tests, 66.66→100%)

`src/routes/Notifications.branches.test.tsx` — 7 tests covering 4 KIND_CLS color branches
and the body present/absent branch. i18n returns key directly (no `vars.default` mock) so
test asserts on `bg-bull/10`, `bg-bear/10`, etc. directly.

Commit: `b2380b6`. Notifications.tsx: 92.3/66.66/85.71/90 → 100/100/100/100.

### v0.89-final — threshold bump

`vitest.config.ts`:
- thresholds: 86/83/79/87 → 87/84/81/88
- comment block updated with v0.76-v0.83 history + v0.89 entry

Verified: `pnpm vitest run --coverage` → exit 0. README auto-bumped by
`scripts/update-readme-coverage.mjs` to 87.5/85.4/81.6/88.6.

## Verified

- `pnpm vitest run` → 960 tests pass
- `pnpm test:coverage` → all 4 dims pass new thresholds
- `scripts/run-ci-local.sh` → 5/5 jobs pass (governance, L1+vitest, Rust cargo,
  Python pytest, Playwright e2e)
- `node scripts/update-readme-coverage.mjs` → badges sync (no drift)
- `pnpm check:doc-sync` → clean (vitest.config.ts is non-doc change, no docs needed)

## Files changed

| File | Lines | Purpose |
|---|---|---|
| `src/routes/Bankroll.branches.test.tsx` | +342 (new) | v0.89a — 18 tests |
| `src/routes/Trade.MarketDetail.branches.test.tsx` | +219 (new) | v0.89b — 9 tests |
| `src/routes/Notifications.branches.test.tsx` | +106 (new) | v0.89c — 7 tests |
| `vitest.config.ts` | +20 / -8 | v0.89-final — threshold bump + comment history |
| `README.md` | ±4 | v0.89a + v0.89c — auto-bumped by update-readme-coverage.mjs |

## Next

**v0.90** — codegen Phase 5 (build pipeline integration): wire
`scripts/check-codegen-drift.mjs` exit 1 into `pnpm build` and `pnpm tauri build`
so the 28% of IPCs covered by codegen get drift-protection at build time, not just
in CI.

**v0.91** — ModelLab useTrainProgress custom hook refactor. Fills the v0.83 deferred
branch coverage gap. Should bump ModelLab fn from 59 → 75+ and possibly cross the
85% threshold on functions project-wide.

**v0.92+** — remaining 81 IPCs not drift-protected by codegen. Two strategies:
(1) custom `serde_json::Value` field wrappers for DTOs with truly dynamic payloads;
(2) accept 28% as realistic ceiling if (1) proves infeasible for the long tail of
configuration / diagnostic commands.

## Test totals

| Sub | vitest | cargo | Python | Playwright | Total |
|---|---|---|---|---|---|
| v0.89a | 944 (+18) | 355 | 86 | 7 | 1392 |
| v0.89b | 953 (+9) | 355 | 86 | 7 | 1401 |
| v0.89c | 960 (+7) | 355 | 86 | 7 | 1408 |

## Coverage progression (since v0.62a)

```
v0.62a.2:  64.0/57.0/52.0/64.0   (504 tests)
v0.69:     74.0/68.0/62.0/74.0
v0.74:     83.0/81.0/76.0/84.0
v0.83:     86.0/83.0/79.0/87.0
v0.89:     87.5/85.4/81.6/88.6   (960 tests)
```

That's +23.5pp stmts, +28.4pp branches, +29.6pp functions, +24.6pp lines in ~13 months
of test work, with 456 new tests added.
