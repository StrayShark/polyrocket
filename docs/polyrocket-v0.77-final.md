# polyrocket v0.77 — branches-focused coverage plateau (9 sub-versions)

> **TL;DR**: v0.77 = **9 sub-versions, branches-focused coverage plateau**:
> **(1) v0.77a** SidecarHealthBadge (+7 tests, 0%→84.6% br) — topbar pill
> **(2) v0.77b** CommandPalette (+11 tests, 0%→86.4% br) — keyboard nav + filter
> **(3) v0.77c** MarketDetail (+5 tests, 31.8%→72.7% br) — single-market page
> **(4) v0.77d** welcome/theme stores + FinishStep (+10 tests, 46%→76% br) — store API
> **(5) v0.77e** prefs-io (+6 tests, 73.6%→77.8% br) — import/export validation
> **(6) v0.77f** Settings (+6 tests, 74.5%→76.1% br) — RetentionCard clamps
> **(7) v0.77g** PlaceBetForm (+5 tests, 75.9% br, +6.9pp stmts) — form
> **(8) v0.77h** threshold ratchet 83/81/76/84 → **86/83/80/87**
> **(9) v0.77i** welcome-store migrate (+8 tests, 46.1%→88.5% br) ⭐ — legacy key migration
> **总测试**: 819 → **877** (+58)
> **总覆盖**: 83.19/81.64/76.90/84.54 → **86.34/83.99/80.06/87.57**

---

## 1. Why this version exists

v0.75 是 coverage ratchet round 4 (25 tests, +0.18/+0.32/+0.22/+0.16pp).
v0.76 是 codegen Phase 1 (tool work, no coverage ratchet).
v0.77 = **branches-focused round** — v0.75e 已经发现 branches 是 next bottleneck
(0.5pp headroom 都不够 bump threshold to 82).

从 v0.75 final 之后 uncovered branches 最多的文件:

| file | uncovered br | 备注 |
|---|---|---|
| `SidecarHealthBadge.tsx` | 13 | 0% 全未测,简单 component |
| `CommandPalette.tsx` | 22 | 0% 全未测,148 行 |
| `MarketDetail.tsx` | 15 | 31.8% — 1 个 test |
| `welcome-store.ts` | 14 | 46.1% — migrateLegacy 路径 |
| `FinishStep.tsx` | 2 | 50% — 最后 welcome step |
| `theme-store.ts` | 2 | 50% — 主题切换 |
| `ModelLab.tsx` | 53 | 57.2% — 5 tab 全未触达 (留 v0.78) |
| `Settings.tsx` | 48 | 74.5% — 已有 38 tests |
| `BacktestReport.tsx` | 21 | 73.1% — backtest modal |
| `prefs-io.ts` | 19 | 73.6% — 已有 10 tests |
| `PromoteHistory.tsx` | 16 | 74.6% — promote history table |
| `PlaceBetForm.tsx` | 14 | 75.9% — 已有 9 tests |

→ v0.77 目标: 把前 7 项全部 lift 到 75%+ branches,留下 ModelLab 53 + Settings 48 + BacktestReport 21 给 v0.78+。

---

## 2. Per-sub-version impact

### v0.77a — SidecarHealthBadge (+7 tests)
- 82-line pill,13 branches (3 status × success/fail + click handler)
- 0%→91.3% stmts, 0%→84.6% br, 100% fn/lines
- 覆盖: unknown / ok (success only) / ok (success > failure) / failed (failure > success) / failed (failure only) / click probe / busy guard

### v0.77b — CommandPalette (+11 tests)
- 148-line modal,22 branches
- 0%→100% stmts, 0%→86.4% br
- 覆盖: closed / open / filter / empty / ArrowDown / ArrowUp / Enter / Escape / mouse enter / click / useCommandPalette hook

### v0.77c — MarketDetail (+5 tests)
- 182-line /markets/:id page,15 uncovered branches
- 31.8%→72.7% br, 100% stmts/fn/lines
- 覆盖: loading skeleton / error banner / success state / signal filter (only this market) / missing signals

### v0.77d — Stores + FinishStep (+10 tests)
- welcome-store (7 tests): setStep / setDone / setLocale / setConfigured / reset + WELCOME_STEPS
- theme-store (2 tests): dark default / setTheme to light/matrix
- FinishStep (1 test): smoke render

### v0.77e — prefs-io (+6 tests)
- 6 per-field validation throw paths + readFileAsText error path + downloadPrefsAsFile blob URL flow
- 73.6%→77.8% br, +6.9pp stmts

### v0.77f — Settings (+6 tests)
- RetentionCard clamp paths (days input) + all-IPC reject + empty telemetry
- 74.5%→76.1% br (+1.6pp), 79.0%→79.6% stmts

### v0.77g — PlaceBetForm (+5 tests)
- 5 smoke tests covering empty listMarkets / listMarkets reject / no active wallet / success / error
- 75.9% br (unchanged — smoke tests don't hit branchy paths), +6.9pp stmts

### v0.77h — threshold ratchet 83/81/76/84 → **86/83/80/87**
- All 4 dimensions have headroom
- Functions headroom tight (0.06pp) — re-run risk on jitter

### v0.77i — welcome-store migrate (+8 tests) ⭐
- 8 tests for the lazy `migrateLegacy()` function (14 uncovered branches)
- Covers: no legacy key, step 0/1/2/3 mappings, new key exists, invalid JSON, WELCOME_STEPS constant
- **46.1%→88.5% br (+42.4pp)** ⭐ — biggest single-round branch lift
- 77.5%→95.0% stmts (+17.5pp)

---

## 3. Test pattern: lazy migration testing

The `migrateLegacy()` function only runs ONCE (on first store init). To test
the migration paths, I had to:
1. Set `localStorage` BEFORE importing the store
2. Use `await import('@/stores/welcome-store')` to get a fresh module
3. Use `vi.resetModules()` to force module re-evaluation between tests

This is a common pattern for "module init side effects" testing. The key
insight: `vi.resetModules()` + dynamic import gives you a fresh module
instance, which triggers `migrateLegacy()` again.

---

## 4. Coverage deltas

| file | v0.75e | v0.77 | Δ |
|---|---|---|---|
| SidecarHealthBadge.tsx | 0% | 91.3/84.6/100/100 | +91.3/+84.6/+100/+100 |
| CommandPalette.tsx | 0% | 100/86.4/100/100 | +100/+86.4/+100/+100 |
| MarketDetail.tsx | 31.8% | 100/72.7/100/100 | +68.2/+40.9/+0/+0 |
| welcome-store.ts | 46.1% | 95.0/88.5/92.3/97.0 | +17.5/+42.4/+0/+17.5 |
| FinishStep.tsx | 50% | 100/100/100/100 | +50/+50/+40/+33 |
| theme-store.ts | 50% | 100/100/100/100 | +50/+50/+0/+50 |
| prefs-io.ts | 73.6% | 91.3/77.8/100/91.1 | +6.9/+4.2/+0/+9.2 |
| Settings.tsx | 74.5% | 79.6/76.1/71.5/80.9 | +0.6/+1.6/+1.5/+1.0 |
| PlaceBetForm.tsx | 75.9% | 82.8/75.9/70.0/83.9 | +6.9/+0.0/+0.0/+8.2 |
| **GLOBAL** | **83.19/81.64/76.90/84.54** | **86.34/83.99/80.06/87.57** | **+3.15/+2.35/+3.16/+3.03** |

Threshold: 83/81/76/84 → **86/83/80/87**.

---

## 5. Tests added (58 total, all PASS)

| file | tests | what they cover |
|---|---|---|
| SidecarHealthBadge.test.tsx | 7 | 3 status branches + click probe + busy guard |
| CommandPalette.test.tsx | 11 | full keyboard + mouse + hook + filter + empty |
| MarketDetail.round2.test.tsx | 5 | loading + error + success + signal filter + missing |
| v0.77d-stores.test.tsx | 10 | store API + FinishStep smoke |
| prefs-io.round2.test.ts | 6 | per-field validation + readFile error + download |
| Settings.round3.test.tsx | 6 | RetentionCard clamps + reject + empty |
| PlaceBetForm.round2.test.tsx | 5 | empty / reject / no wallet / success / error |
| welcome-store.migrate.test.ts | 8 | lazy migration of 4 legacy step values + invalid JSON + WELCOME_STEPS |

Total: 58 new tests, 0 unhandled errors, 0 regressions.

---

## 6. Patterns locked

### 6a. Lazy migration testing

```ts
beforeEach(() => {
  window.localStorage.clear();
  vi.resetModules();
});

it('migrateLegacy: legacy step 1 → theme', async () => {
  window.localStorage.setItem(LEGACY_KEY, JSON.stringify({state: {step: 1}}));
  const {useWelcomeStore: fresh} = await import('@/stores/welcome-store');
  expect(fresh.getState().step).toBe('theme');
});
```

`vi.resetModules()` + dynamic import = fresh module instance, triggers
module init side effects (like `migrateLegacy()`).

### 6b. happy-dom env pragma

Every new test file with DOM rendering needs:
```ts
// @vitest-environment happy-dom
```

The `vitest.config.ts` default is `node` env, which has no `document`.

### 6c. validateOrderArgs in PlaceBetForm mocks

PlaceBetForm calls `validateOrderArgs` (separate IPC) before `placeSignedOrder`.
Mock both:
```ts
vi.mock('@/ipc', () => ({
  placeSignedOrder: ...,
  validateOrderArgs: () => Promise.resolve({ok: true, errors: []}),
}));
```

---

## 7. Files changed

- `src/components/feedback/SidecarHealthBadge.test.tsx` (NEW, 7 tests)
- `src/components/feedback/CommandPalette.test.tsx` (NEW, 11 tests)
- `src/routes/MarketDetail.round2.test.tsx` (NEW, 5 tests)
- `src/v0.77d-stores.test.tsx` (NEW, 10 tests)
- `src/lib/prefs-io.round2.test.ts` (NEW, 6 tests)
- `src/routes/Settings.round3.test.tsx` (NEW, 6 tests)
- `src/components/feedback/PlaceBetForm.round2.test.tsx` (NEW, 5 tests)
- `src/stores/welcome-store.migrate.test.ts` (NEW, 8 tests)
- `vitest.config.ts` (threshold 83/81/76/84 → 86/83/80/87)
- `docs/polyrocket-v0.77-final.md` (this file)
- `docs/overview.md` (v2.40 → v2.41)
- `docs/coding-spec.md` (v2.1 → v2.2)

---

## 8. What's next

**v0.78** — model-coverage plateau (next bottleneck):
- ModelLab.tsx 57.2% br (53 uncovered) — biggest single-file opportunity
- 5 tabs (Train / Sweep / Promote / Backtest / Archive) all under-tested
- Tab-switching tests + modal lifecycle
- 8-10 sub-versions, 50+ tests

**v0.79** — other under-tested files:
- BacktestReport 73.1% br (21 uncovered)
- PromoteHistory 74.6% br (16 uncovered)
- Notifications 66.7% br (4 uncovered)
- StorageStep / env-file / WelcomeStep / toast-store

**v0.80** — threshold ratchet (target 87/84/80/88 if achievable)

**codegen Phase 2-5** — independent of coverage, can be parallel:
- Phase 2: snake_case config + verify dashboard_kpis matches
- Phase 3: ~10 read-only commands
- Phase 4: input DTO commands
- Phase 5: build pipeline integration

---

## 9. Test status

- Total: **877 tests**, 95 files, all PASS
- Cargo tests: 319 (unchanged)
- TypeScript tests: 877 (was 819, +58)
- Python tests: 86 (unchanged)
- **Grand total: 1282** (was 1199, +83)
- Typecheck: 0 errors
- Coverage: **86.34/83.99/80.06/87.57** (was 83.19/81.64/76.90/84.54, +3.15/+2.35/+3.16/+3.03pp)
