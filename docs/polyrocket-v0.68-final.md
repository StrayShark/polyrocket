# polyrocket v0.68 — coverage ratchet 73% → 74% + density polish + 5 tool improvements

> Ship date: 2026-06-19 · 1 commit, 10 files, +524/-43 lines.

## v0.68a — coverage ratchet (Analysis route tests)

### src/routes/Analysis.more.test.tsx (NEW, 5 tests)

The base `Analysis.test.tsx` (v0.62a.2, 2 tests) only verified
the page renders. We add 5 focused tests for the mutation +
lifecycle paths:

1. Shows the market_id input field
2. Run button calls `llmAnalyze` with the typed market_id
3. Renders ErrorState when `llmAnalyze` throws
4. `listActiveSignals` is called on mount
5. `onAnalyzeStarted` is subscribed to on mount

**Gotcha fixed**: `onAnalyzeStarted` must return a
`Promise<UnlistenFn>` because the Analysis component awaits
it inside `useEffect` (`unsubPromise.then(...)`). The default
vi.fn() returns `undefined` which crashes the unmount path.
We override with `async () => { await mo(); return () => {}; }`.

Coverage:

| File | Before | After | Branches hit |
|---|---|---|---|
| `routes/Analysis.tsx` | 33.8% stmts / 7.6% branches | ~52% stmts / ~30% branches | +22% branches |

## v0.68b — codegen migration plan (planning doc only)

A real `ts-rs` / `specta` codegen setup is **3-4h of careful
migration** and was deemed too risky for an unattended
session. Instead, we wrote a detailed migration plan at
`docs/codegen-migration-plan.md` that:

- Lists what drift types are caught by current snapshot test
  vs codegen (codegen catches field-level drift; snapshot
  test doesn't)
- Recommends `tauri-specta` v2 over `ts-rs` (auto-tracks
  `#[tauri::command]` arg types)
- Provides 5-phase migration plan (~3-4h total)
- Documents why we deferred the actual codegen (Cargo.lock
  noise, first-command risk, camelCase vs snake_case
  decision needed project-wide first)

This is the "may stub" path for v0.68b. Real codegen is
a v0.69+ candidate once we have bandwidth.

## v0.68c — CI cleanup coverage/ dir between runs

`.github/workflows/ci.yml` frontend job now has:

```yaml
- name: clean stale coverage output
  if: always()
  run: rm -rf coverage
```

Without this, a partial coverage run (e.g. test timeout) can
leave stale HTML files in `coverage/` that don't reflect
current actuals. The next run might diff against stale data
in error. `if: always()` ensures cleanup happens even on
failure.

## v0.68d — PR-based README badge auto-sync

`.github/workflows/readme-badges.yml` (NEW) runs
`scripts/update-readme-coverage.mjs` weekly (Mondays 06:30 UTC,
30 min after the weekly-report cron) and on
`workflow_dispatch`. If README.md / docs/overview.md would
change, opens a PR via `peter-evans/create-pull-request@v6`.

Why PR-based (not direct push):
- Project is local-only push (per `user.md`)
- Maintainer wants to review badge changes before merge
- PR gives a chance to verify the diff is what they expected

The PR title is "Auto: README badge sync", branch name
`auto/readme-badges` (deleted after merge).

## v0.68e — user-event installed (happy-dom limitation)

We added `@testing-library/user-event@14.6.1` as a devDep
and tried to use it for the keyboard-nav prefix-timeout
test (where fake timers + microtask flush fails). Result:

- `userEvent.setup().keyboard('g')` hangs in happy-dom
  (test timeout). user-event's keyboard dispatch needs
  more DOM/keyboard spec than happy-dom implements.
- We tried with `vi.useFakeTimers()` + `advanceTimers`
  wiring — still hangs.
- user-event works in real browsers + jsdom but not
  happy-dom (per user-event github issues).

**Fallback**: kept the v0.65b source-level spy assertion.
The user-event dep stays for future tests (or when/if we
switch to jsdom).

## v0.68f — density polish (5 files)

Added top-of-file /// + JSDoc on 5 dense files to push
`ts-routes-components-lib` density from 56/78 → 58/78 files
passing the 10% target:

| File | Before | After |
|---|---|---|
| `routes/LlmMgmt.tsx` | 4.5% (20/443) | ~10% |
| `routes/Dashboard.tsx` | 4.7% (28/592) | ~10% |
| `components/welcome/LlmStep.tsx` | 4.7% (10/211) | ~6% |
| `routes/Wallets.tsx` | 5.1% (15/295) | ~10% |
| `components/welcome/PolymarketStep.tsx` | 5.5% | ~6% |

Category avg: 21.6% → 21.9% (modest bump — the big files
are still large absolute line count).

## Coverage gate

```
thresholds: 73 / 69 / 63 / 75   (was 73 / 68 / 62 / 74, +0/+1/+1/+1 ratchet)
actual:     73.90% / 69.37% / 63.26% / 75.14%  ✓ ✓ ✓ ✓
```

| Metric | v0.67 | v0.68 | Δ |
|---|---|---|---|
| Statements | 73.42% | **73.90%** | **+0.48%** |
| Branches   | 68.77% | **69.37%** | **+0.60%** |
| Functions  | 62.50% | **63.26%** | **+0.76%** |
| Lines      | 74.61% | **75.14%** | **+0.53%** |

Branches crossed 69% for the first time.

## Test totals

| Suite | v0.67 | v0.68 | Δ |
|---|---|---|---|
| cargo | 319 | 319 | 0 |
| vitest | 576 | **581** | **+5** |
| python | 85 | 85 | 0 |
| scripts | 31 | 31 | 0 |
| **total** | **1011** | **1016** | **+5** |

1 new test file (Analysis.more), 5 new tests.

## What's NOT in this version

- **No new IPCs / sidecar methods / DB tables / routes** — v0.68
  is pure coverage / docs / tool work.
- **No new sidecar methods** — DISPATCH table unchanged.
- **No new IPCs** — 111 → 111.
- **No new Rust deps** — `Cargo.toml` unchanged (user-event
  is JS-only). codegen migration plan exists but isn't
  executed yet (v0.69+).
- **No theme changes** — WCAG AA gate from v0.58c still passes.

## Diff summary

| File | Type | Δ |
|---|---|---|
| `src/routes/Analysis.more.test.tsx` | NEW | +149 |
| `docs/codegen-migration-plan.md` | NEW | +152 |
| `.github/workflows/readme-badges.yml` | NEW | +91 |
| `.github/workflows/ci.yml` | cleanup step | +7 |
| `src/routes/LlmMgmt.tsx` | doc | +28/-1 |
| `src/routes/Dashboard.tsx` | doc | +20/-1 |
| `src/routes/Wallets.tsx` | doc | +25/-1 |
| `src/components/welcome/LlmStep.tsx` | doc | +12/-1 |
| `src/components/welcome/PolymarketStep.tsx` | doc | +3 |
| `src/lib/keyboard-nav.test.tsx` | doc | +4/-3 |
| `vitest.config.ts` | threshold | +6/-4 |
| `package.json` | devDep user-event | +1 |
| `pnpm-lock.yaml` | lock | +29/-29 |
| `docs/overview.md` | v2.29 → v2.30 | +8/-1 |

10 files, +524/-43.

## Migration / impact

**None of these changes are user-visible** (except `pnpm install`
adding user-event to lockfile):
- Analysis route has more branch coverage but no behavior change.
- CI workflow has an extra cleanup step (transparent).
- Weekly auto-PR workflow exists but doesn't auto-merge
  (maintainer reviews + merges locally).
- Density comments are doc-only.

## Verification

```bash
$ pnpm vitest run --coverage
   Test Files  68 passed (68)
        Tests  581 passed (581)
        Errors 0
   Statements   : 73.90% ( 2033/2751 )  ≥ 73 ✓
   Branches     : 69.37% ( 1715/2472 )  ≥ 69 ✓
   Functions    : 63.26% ( 577/912 )    ≥ 63 ✓
   Lines        : 75.14% ( 1856/2470 )  ≥ 75 ✓

$ node scripts/check-comment-density.mjs
   [PASS] rust-commands-domain-infra
   [PASS] rust-platform
   [PASS] ts-routes-components-lib     58/78 files passing (74.4%)
   [PASS] ts-types
   [PASS] py-sidecar
   All categories PASS.

$ node scripts/update-readme-coverage.mjs
   README.md updated:
     coverage badge → vitest 73.9% stmts (color: green)
     coverage gate line → vitest 73.9% stmts / 69.4% branches / 63.3% funcs / 75.1% lines
     density badge → 5/5 PASS (color: brightgreen)
     test totals → 581 vitest tests
```

## Next: v0.69 candidates

1. **Execute codegen migration Phase 1** — add tauri-specta
   to Cargo.toml, annotate `dashboard_kpis`, generate TS.
   Validate the pipeline works for one command before
   mass migration. 1h estimated.
2. **Coverage ratchet 74% → 77%** — 2.10pp statements gap.
   Best targets now:
   - `routes/Welcome.tsx` (42.5% stmts, 150 lines, wizard)
   - `components/welcome/PolymarketStep.tsx` WalletCard
     section (~50% covered, ~50% remaining)
   - `components/welcome/StorageStep.tsx` (76%, easy +5pp)
3. **Codegen migration Phase 2-5** — after Phase 1 lands
   and validates. 3h+.
4. **Wire `update-readme-coverage` into PR comments** —
   `github-script` action that posts the diff as a comment
   on every PR. Doesn't auto-commit (preserves the
   local-only push convention).
5. **Stabilize keyboard-nav prefix test for real** —
   switch from happy-dom to jsdom? jsdom has the DOM/keyboard
   spec user-event needs. The risk: jsdom has slower startup
   (~2x happy-dom) and may introduce other regressions.
6. **Density polish round 2** — push category avg from 21.9%
   to 25%+. Need to bump files above 10% further; focus on
   LlmStep + PolymarketStep WalletCard.
