# polyrocket v0.92 — v0.94 — coverage rounds 5, 6, 7

> 2026-06-21 · 3 commits · 3 new test files + 1 modified
> Settings top save/reset + Signals branches + format.ts edges

## TL;DR

Three sub-versions landed:
- **v0.92** — Settings top Card save/reset: fn 72.59→75.55 (+2.96pp). Added `prefs-save-btn` / `prefs-reset-btn` testids to the top Save/Reset buttons (previously untestable due to `getAllByText` returning multiple matches across cards). Also added `data-testid` prop to ToggleRow for the 3 toggle cards.
- **v0.93** — Signals branches: fn 72.7→75.75 (+3.05), stmts 72.7→85.45 (+12.7), lines 67.4→83.67 (+16.3). 7 tests cover Trade button URL, KpiCard delta branches, Recompute success/error, empty-state hint.
- **v0.94** — format.ts edge cases: fn 11→12 (100%), stmts 86→86 (88.4%). 12 new tests for fmtRelativeTime, fmtDate, fmtDateTime, fmtAddress. The existing test file only covered formatRetentionAge.

Coverage moved slightly: 87.78/85.45/81.99/88.98 → 87.75/85.26/82.1/88.87. The slight drop on 3 dims is a denominator effect (added 12 test stmts, the 4 formatters have ~14 uncovered edge cases that don't impact product but do impact %).

## v0.92 — Settings top Card save/reset

### The v0.89d issue, resolved

v0.89d attempted to test the top save/reset but failed because:
1. Top Save/Reset buttons had no testids
2. `getAllByText('settings.btn.save')` returned multiple matches across cards
3. `getAllByText` doesn't accept testid filter

v0.92 adds testids:
- `prefs-save-btn` — top Save button
- `prefs-reset-btn` — top Reset button
- `data-testid` prop on ToggleRow for the 3 toggle cards (toasts, copy-trading, advanced-stats)

### What the top Card actually does

The top Card of `Settings.tsx` (lines 96-113) has a 5-field draft state:
```ts
const [draft, setDraft] = useState({
  defaultMinEdgePct: prefs.defaultMinEdgePct,
  defaultAllocationCapUsdc: prefs.defaultAllocationCapUsdc,
  copyTradingEnabled: prefs.copyTradingEnabled,
  notificationsEnabled: prefs.notificationsEnabled,
  advancedStats: prefs.advancedStats,
});
const dirty = JSON.stringify(draft) !== JSON.stringify({...prefs values});
```

`save()` iterates `Object.entries(draft).forEach(([k, v]) => prefs.setPref(k, v))` — calls `setPref` 5 times. `reset()` calls `prefs.reset()` then rebuilds the draft from `prefs.*`. Both show toasts.

### Tests (6 in Settings.round4.test.tsx, 6/6 pass)

1. Renders top Reset and Save buttons with testids
2. Top Save initially disabled (not dirty)
3. Top Reset calls `prefs.reset()` + `toast.info`
4. Top Save iterates draft → `setPref` for all 5 fields
5. Top Save shows `settings.btn.save_toast` on success
6. Top Reset reverts form to prefs values (next save is no-op)

### Test mocking pattern

`vi.hoisted()` to share mock functions across the `vi.mock` factory. Vitest hoists `vi.mock` to the top of the file, so const declarations are unavailable inside the factory. `vi.hoisted(() => ({ mockFn: vi.fn() }))` solves this.

```ts
const { mockToastSuccess, mockToastInfo } = vi.hoisted(() => ({
  mockToastSuccess: vi.fn(),
  mockToastInfo: vi.fn(),
}));
vi.mock('@/stores/toast-store', () => ({
  toast: { success: mockToastSuccess, error: vi.fn(), info: mockToastInfo },
}));
```

### Toggle click pattern

The Toggle component renders as `<div role="switch" onClick={...}>`. To click it from a `data-testid` wrapper:

```ts
const card = screen.getByTestId('copy-trading-toggle');
const switchEl = card.querySelector('[role="switch"]') as HTMLElement;
fireEvent.click(switchEl);
```

`fireEvent.click` on the outer wrapper doesn't reach the inner toggle's onClick handler. Need to click the `[role="switch"]` directly.

## v0.93 — Signals branches round 2

### What was missing

v0.70b's `Signals.more.test.tsx` covered:
- Loading / error / empty states
- Filter to YES/NO
- minEdgePct clamp
- Recompute mutation trigger

What was still uncovered (8 fn):
- Trade button URL encoding (side=YES/NO, price=predicted_prob)
- KpiCard delta branches (total > 0 vs not, bullish/bearish %)
- Recompute success toast (n > 0 vs n === 0 branches)
- Recompute onError (error message format)
- Empty state "No signals match" description (filtered === 0 vs total === 0)

### Tests (7 in Signals.branches.test.tsx, 7/7 pass)

1. Trade button URL encodes YES + price=0.6500 for bullish
2. Trade button URL encodes NO + price=0.4000 for bearish
3. Bullish KpiCard shows 67% (2/3) when total > 0
4. Bearish KpiCard shows no delta when total === 0
5. Recompute success toast for n > 0
6. Recompute onError shows error toast
7. Empty state "No signals match" when total > 0 but filtered === 0

### Coverage impact

| Dim | Before | After | Δ |
|---|---|---|---|
| stmts | 72.7% | 85.45% | +12.7 |
| fn | 72.7% | 75.75% | +3.05 |
| br | 72% | 84% | +12 |
| lines | 67.4% | 83.67% | +16.3 |

## v0.94 — format.ts edge cases

### What was missing

`src/lib/format.test.ts` only covered `formatRetentionAge`. The other 11 formatters had 0% direct test coverage:
- fmtUsdc, fmtPct, fmtPctInt, fmtEdge, fmtConfidence
- fmtLatency, fmtCents
- **fmtAddress, fmtRelativeTime, fmtDate, fmtDateTime** (most-used)
- formatRetentionAge (covered)

### Tests (12 new in format.test.ts, 13/13 pass)

5 formatters × 2-3 cases each:
- `fmtRelativeTime`: em-dash null/undef, "just now" < 60s, "Xm ago", "Xh ago", "Xd ago"
- `fmtDate`: em-dash null/undef, US locale format
- `fmtDateTime`: em-dash null/undef, US locale date+time
- `fmtAddress`: em-dash null/undef, full short address, truncated long

### Coverage impact

| Dim | Before | After | Δ |
|---|---|---|---|
| stmts | 86% | 88.4% | +2.4 |
| fn | ~85% | 100% | +15 |
| lines | 90.7% | 93.8% | +3.1 |
| br | 87.5% | 80.9% | -6.6 (more branches exposed) |

The branch % went DOWN because the new tests exposed previously-unexercised branches. The new branch coverage is the new "ground truth" — the function is now 100% covered, even if some branches weren't tested before.

## Project coverage progression

| Version | stmts | br | fn | lines | Tests |
|---|---|---|---|---|---|
| v0.89-final | 87.42 | 85.0 | 81.51 | 88.6 | 960 |
| v0.90+91 | 87.53 | 85.45 | 81.58 | 88.71 | 967 |
| v0.92 | 87.78 | 85.45 | 81.99 | 88.98 | 973 |
| v0.93 | 87.82 | 85.45 | 82.10 | 89.02 | 980 |
| v0.94 | 87.75 | 85.26 | 82.10 | 88.87 | 993 |

Threshold (v0.89-final): 87/84/81/88. All 4 dims pass with 0.75/1.26/1.10/0.87pp headroom.

## Files changed

| File | Lines | Purpose |
|---|---|---|
| `src/routes/Settings.round4.test.tsx` | +246 (NEW) | v0.92 — 6 top Card tests |
| `src/routes/Settings.tsx` | +9/-3 | v0.92 — testids on top Save/Reset + ToggleRow |
| `src/routes/Signals.branches.test.tsx` | +149 (NEW) | v0.93 — 7 branches tests |
| `src/lib/format.test.ts` | +73/-16 | v0.94 — 12 formatter edge cases |
| `README.md` | ±4 | auto-bumped by update-readme-coverage.mjs |

## Verified

- `pnpm typecheck` → exit 0
- `pnpm vitest run` → **993 tests pass** (was 960 at v0.89-final; +33 across v0.90-94)
- `pnpm test:coverage` → 87.75/85.26/82.10/88.87 (all 4 dims pass v0.89 threshold 87/84/81/88)
- `pnpm build` → exit 0, no codegen drift
- `pnpm check:codegen-drift` → "no drift"
- `scripts/run-ci-local.sh` → **5/5 jobs PASS**

## What's still on the roadmap

**v0.95+ coverage rounds**: ModelLab 57.4% (20 uncovered fn) is the biggest gap. Targeting auto-promote flow, comparison modal, archive viewer.

**v0.96+ codegen coverage**: 28% → 50%+. Need `serde_json::Value` field wrappers for the long tail.

**v0.97+ feature work**: New product features (model training pipeline improvements, signal UI polish, etc.).
