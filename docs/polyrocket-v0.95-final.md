# polyrocket v0.95 — coverage round 8 + threshold bump

> 2026-06-21 · 1 commit · 1 new test file + 3 modified + 1 source
> Hook tests + format/i18n/retry-policy + threshold 87/84/81/88 → 88/85/82/88

## TL;DR

**Threshold bumped: 87/84/81/88 → 88/85/82/88** (+1pp on stmts/br/fn).

After v0.92-94 the project was at 87.75/85.26/82.10/88.87, just below
the next-ratchet target. v0.95 added 4 small test rounds to push
3 of 4 dims above their new targets:

| Dim | Before | After | Δ | New threshold | Headroom |
|---|---|---|---|---|---|
| stmts | 87.75 | 88.03 | +0.28 | 88 | +0.03 |
| br | 85.26 | 85.53 | +0.27 | 85 | +0.53 |
| fn | 82.10 | 82.41 | +0.31 | 82 | +0.41 |
| lines | 88.87 | 88.98 | +0.11 | 88 | +0.98 |

The lines bump attempt (89) failed: 88.98 < 89.00. The 0.02pp gap
is from closing-brace lines at the end of multiple files (i18n.ts,
Bankroll.tsx, welcome-store.ts, prefs-store.ts) that v8 counts as
uncovered but aren't testable. Will need either a different
threshold strategy or a test that adds a real coverage line.

**Test count: 993 → 1008 vitest (+15)**

## What was added

### 1. `src/hooks/useInvoke.test.tsx` (NEW, +2 tests)

useInvoke was added to coverage include list in v0.91 but had 0
direct tests. The file is only 18 lines (a thin React Query
wrapper). Two tests cover the main fn call + args pass-through:

```ts
useInvoke(['test', 'key'], fn, undefined as void)
useInvoke(['args-test'], fn, { id: 42 })
```

### 2. `src/lib/format.test.ts` (+17 tests, 13→30)

All 11 numeric formatters covered (was 0% for most):
- fmtUsdc: em-dash null/undef/non-finite, "123.45" for number
- fmtPct: em-dash null/undef/Infinity, "15.0%", "+15.0%" signed,
  "-15.0%" negative (no +)
- fmtPctInt: "50%" no decimal, em-dash null
- fmtEdge: always +/- prefix
- fmtConfidence: em-dash null, "85.0%"
- fmtLatency: "123ms" < 1000, "1.5s" >= 1000, em-dash null
- fmtCents: "100 → $1.00" cents to dollars conversion

format.ts: stmts 88.37→94.18 (+5.81pp), br 80.89→88.76 (+7.87pp),
fn 100%.

### 3. `src/lib/retry-policy.test.ts` (+1 test, 6→7)

Added `applyRetryPolicy` test — was 0% direct coverage. The
function sets QueryClient defaults:
- queries.refetchOnWindowFocus = false
- queries.retry = shouldRetry
- queries.retryDelay = retryDelayMs
- queries.staleTime = 30_000
- mutations.retry = false (no auto-retry on user actions)

Uses a stub `client` object with a `setDefaultOptions` spy
(avoids depending on TanStack Query internals).

### 4. `src/lib/i18n.test.ts` (+2 tests, 17→19)

Added `useT` hook tests — was 0% direct coverage. Tests:
- Returns `{ locale, t }` shape
- `t('page.title')` returns a non-empty string in zh locale

The "1 uncovered line" in i18n.ts (L1881) is the trailing newline
that v8 counts as a line but isn't testable.

## Threshold ratchet history

| Version | stmts | br | fn | lines | Tests | Date |
|---|---|---|---|---|---|---|
| v0.62a.2 | 64 | 57 | 52 | 64 | 504 | 2026-05 |
| v0.74 | 83 | 81 | 76 | 84 | 794 | 2026-05 |
| v0.83 | 86 | 83 | 79 | 87 | 920 | 2026-06 |
| v0.89 | 87 | 84 | 81 | 88 | 960 | 2026-06 |
| **v0.95** | **88** | **85** | **82** | **88** | **1008** | 2026-06 |

The lines threshold is intentionally at 88 (not 89) because the
0.02pp gap to 89 is unreachable without testing closing braces.
Future v0.96+ rounds can try to add a real coverage line to push
lines to 89 (e.g. a 1-line conditional in some file).

## Files changed

| File | Lines | Purpose |
|---|---|---|
| `src/hooks/useInvoke.test.tsx` | +41 (NEW) | v0.95 — 2 useInvoke tests |
| `src/lib/format.test.ts` | +62/-3 | v0.95 — 17 numeric formatter tests |
| `src/lib/retry-policy.test.ts` | +20/-0 | v0.95 — applyRetryPolicy test |
| `src/lib/i18n.test.ts` | +22/-0 | v0.95 — 2 useT tests |
| `vitest.config.ts` | +3/-3 | v0.95 — threshold 87/84/81/88 → 88/85/82/88 |
| `README.md` | ±2 | auto-bumped by update-readme-coverage.mjs |

## Verified

- `pnpm typecheck` → exit 0
- `pnpm vitest run --coverage` → exit 0 (all 4 dims pass 88/85/82/88)
- `pnpm build` → exit 0, no codegen drift
- `pnpm check:codegen-drift` → "no drift"
- `scripts/run-ci-local.sh` → **5/5 jobs PASS**

## What's still on the roadmap

**v0.96** — more coverage. ModelLab 57.4% (20 uncovered fn) is the
biggest remaining gap. Targets: auto-promote flow, comparison
modal, archive viewer. Each is a complex state machine with
several testable branches.

**v0.97** — try lines threshold 89. Need to find a testable 1-2
line gap. Maybe add a test for a new branch in some file, or
write a test that exercises a previously-uncovered `if` line.

**v0.98+** — codegen coverage expansion (28% → 50%+). The
remaining 81 IPCs need `serde_json::Value` field wrappers or
acceptance that 28% is the realistic ceiling.

**v0.99** — feature work. The v0.69-v0.95 era has been coverage
+ tooling. Time to focus on actual product features.
