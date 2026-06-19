# polyrocket v0.75 final — coverage ratchet round 4 (25 tests, 4 sub-versions)

> **TL;DR**: v0.75 = **5 sub-versions, 纯测试覆盖轮**:
> **(1) v0.75a** ModelLab round 3 (+8 tests, 62% stmts unchanged — 验证行为)
> **(2) v0.75b** Settings round 2 (+8 tests, 78.5→79.0% stmts, 72.9→74.5% br)
> **(3) v0.75c** LlmMgmt branches round 4 (+5 tests, 89.3→96% br) ⭐
> **(4) v0.75d** Wallets branches round 3 (+4 tests, 91.7→95.8% stmts, 96.7% br)
> **(5) v0.75e** threshold ratchet 82/81/76/84 → **83/81/76/84** (stmts only)
> **总测试**: 794 → **819** (+25)
> **总覆盖**: 83.01/81.32/76.68/84.38 → **83.19/81.64/76.90/84.54**

---

## 1. Why this version exists

v0.74 是 **治理 + UI 重构** round(不增加 coverage ratchet)。v0.75 = 把 v0.74 留下的 under-covered branches 收尾:

**Trigger**: v0.74d Visual Acceptance Gate + v0.74a/b safe* wrappers 都 ship 了,但 coverage 数字没动(stayed at 83.0% stmts)。Branch coverage 是 next bottleneck:
- 0.74d 加的 class-coverage lint 不影响 JS coverage
- LlmMgmt 89.3% br 还有 8 个 unreachable branches
- Wallets 96.7% br 有 4 个 lines uncovered(85/98/111/268)
- Settings 78.5% stmts 收尾 (AppearanceCard v0.74f / ExplainabilityCard / ClobFeedCard / PaperMode)
- ModelLab 62% stmts 收尾 (5 个 tab 大头未触达,留 v0.77+)

→ v0.75 = **branch-closing round** + Settings finalization。

---

## 2. Per-sub-version impact

### v0.75a — ModelLab round 3 (+8 tests)
- ModelLab.round3.test.tsx (8 new tests, 0.62→0.62% stmts, 0.57→0.57% br)
- 重点: 验证 dead-code-prone paths(空 perf data、sidecar health、trainJob rejection、auto-promote listener 注册)
- ModelLab 内部 5 个 tab 仍然是 under-covered 区域,目标 **v0.77+** 再深挖
- 顺带 commit v0.74 final 落下的 docs(coding-spec.md v1.9 bump + polyrocket-v0.74-final.md ship log)

### v0.75b — Settings round 2 (+8 tests)
- Settings.round2.test.tsx covers under-tested cards:
  - **AppearanceCard** (v0.74f new — theme + locale pickers)
  - **RetentionCard** (90d default + custom days branch)
  - **CLOB feed card** (v0.51a)
  - **ExplainabilityCard** (v0.55 + v0.59 SHAP/exact, with/without active model)
  - **PaperMode card** (env-mode branches)
  - **Scheduler empty branch** (0 loops)
  - **All-IPC throw path** (graceful degradation)
- 78.5→79.0% stmts (+0.5pp), 72.9→74.5% br (+1.6pp)

### v0.75c — LlmMgmt branches round 4 (+5 tests) ⭐
- LlmMgmt.round4.test.tsx targets the 8 remaining branches at lines 92, 144, 197, 208:
  - test failed: error_message populated (line 92 main branch)
  - test failed: error_message null → 'code N' fallback (line 92 else)
  - providers error → ErrorState with onRetry (line 144)
  - empty state Add button → opens AddKeyModal (line 197)
  - per-key Test button (line 208)
- **89.3→96% br (+6.7pp)** ⭐ — biggest single-round branch lift

### v0.75d — Wallets branches round 3 (+4 tests)
- Wallets.round3.test.tsx targets the 4 remaining uncovered lines at 85, 98, 111, 268:
  - refresh button click (line 85)
  - error state with onRetry (line 98)
  - empty state Add button → Add modal (line 111)
  - Add modal label input controlled state (line 268)
- 91.7→95.8% stmts (+4.1pp), 96.7% br (unchanged — branches were already at 96.7%)

### v0.75e — threshold ratchet (stmts only)
- 82/81/76/84 → **83/81/76/84** (only stmts bumped)
- Bump criteria: actual ≥ new threshold + ≥0.5pp headroom from current threshold
- Branches 81.64% < 82 (failed ratchet — only 0.64pp above 81, can't go to 82)
- Functions 76.9% < 77 (only 0.9pp above 76, can't go to 77)
- Lines 84.54% < 85 (only 0.54pp above 84, can't go to 85)
- Only stmts 83.19% ≥ 83 (with 1.19pp headroom) → bumpable
- **Branch coverage** is the next focus for v0.77+ round

---

## 3. Test pattern: shape-aware mock

Two findings during v0.75:

### 3a. `mockListPromoteHistory.mockResolvedValue([])` ≠ expected shape

The ModelLab code does `historyQuery.data?.entries ?? []` — expects `{entries: [...]}` shape.
A bare array `[]` works (because `data?.entries` is `undefined` → `?? []` gives `[]`),
BUT when used as the entries prop for `ModelComparison`, it triggers an "allEntries is not iterable"
unhandled error on certain re-renders (the model component receives `data` directly, not the unwrapped array).

**Fix**: Use `mockResolvedValue({entries: []})` to match the actual Rust response shape.

### 3b. `toast` mock needs to be a callable function, not a `{toast: ...}` object

The original Settings test mocks `@/stores/toast-store` as `{toast: {success: ..., error: ...}}`.
But some code paths (LlmMgmt) call `toast.error(...)` directly via `import {toast}`, requiring the import
to resolve to the actual function. Using `vi.mock` with a function works:

```ts
vi.mock('@/stores/toast-store', () => ({
  toast: {
    success: vi.fn(),
    error: vi.fn(),
    info: vi.fn(),
  },
}));
```

Importing the mock back into the test file requires using `import * as toastStore from '@/stores/toast-store'`
because `require()` inside vi.mock factories can't resolve the alias.

---

## 4. Coverage deltas

| file | v0.74 | v0.75 | Δ |
|---|---|---|---|
| ModelLab.tsx | 62.0% / 57.2% / 50.0% / 62.6% | 62.0% / 57.2% / 50.0% / 62.6% | (no change) |
| Settings.tsx | 78.5% / 72.9% / 70.0% / 79.9% | 79.0% / 74.5% / 70.0% / 80.3% | +0.5/+1.6/+0.0/+0.4 |
| LlmMgmt.tsx | 94.7% / 89.3% / 91.7% / 94.5% | 96.0% / 96.0% / 91.7% / 95.9% | +1.3/+6.7/+0.0/+1.4 |
| Wallets.tsx | 91.7% / 96.7% / 83.3% / 91.7% | 95.8% / 96.7% / 91.7% / 95.8% | +4.1/+0.0/+8.4/+4.1 |
| **GLOBAL** | **83.01/81.32/76.68/84.38** | **83.19/81.64/76.90/84.54** | **+0.18/+0.32/+0.22/+0.16** |

Threshold: 82/81/76/84 → **83/81/76/84** (stmts only).

---

## 5. Tests added (25 total, all PASS)

| file | tests | what they cover |
|---|---|---|
| ModelLab.round3.test.tsx | 8 | empty perf, sidecar health, sidecar predict fail, trainJob reject, backtest modal, sections, listener, perf throw |
| Settings.round2.test.tsx | 8 | Appearance, Retention, CLOB feed, Explainability x2, PaperMode, scheduler empty, all-IPC throw |
| LlmMgmt.round4.test.tsx | 5 | test failed w/ message, test failed fallback, providers error, empty Add, per-key test |
| Wallets.round3.test.tsx | 4 | refresh, error state, empty Add, label input controlled |

Total: 25 new tests, 0 unhandled errors, 0 regressions.

---

## 6. Patterns locked

### 6a. happy-dom env pragma mandatory

Every new test file (`.test.tsx` that uses DOM APIs) MUST include:
```ts
// @vitest-environment happy-dom
```

Without it, vitest defaults to `node` env and `document` is undefined.
The existing pattern in `ErrorBoundary.test.tsx` and `KbdHelpDialog.test.tsx`
shows this. New test files that mount React components rendering to DOM
need this pragma at the top.

### 6b. usePrefsStore selector mock

```ts
const fn: any = (sel?: any) => (sel ? sel(state) : state);
fn.getState = () => state;
return { usePrefsStore: fn };
```

This handles BOTH:
- `usePrefsStore(selector)` (with selector) → calls selector
- `usePrefsStore()` (no selector) → returns whole state
- `usePrefsStore.getState()` (toast-store / non-React access) → returns state

### 6c. Promise-wrapped IPC mocks

```ts
setAutoPromoteConfig: (...args: unknown[]) => Promise.resolve(mockSetAutoPromoteConfig(...args)),
```

NOT `mockSetAutoPromoteConfig` directly. The Rust side returns a Promise;
the source code calls `.catch(() => {...})` on it, which fails on `undefined`.

---

## 7. Files changed

- `src/routes/ModelLab.round3.test.tsx` (new, 8 tests)
- `src/routes/Settings.round2.test.tsx` (new, 8 tests)
- `src/routes/LlmMgmt.round4.test.tsx` (new, 5 tests)
- `src/routes/Wallets.round3.test.tsx` (new, 4 tests)
- `vitest.config.ts` (threshold 82→83 stmts, comment v0.68 entry)
- `docs/coding-spec.md` (v1.9 — was uncommitted from v0.74 final)
- `docs/polyrocket-v0.74-final.md` (was uncommitted from v0.74 final)
- `docs/polyrocket-v0.75-final.md` (this file)
- `docs/overview.md` (v2.37 → v2.39, v0.75 final)

---

## 8. What's next

**v0.76** — codegen Phase 1 (tauri-specta). 5-phase plan from `docs/codegen-migration-plan.md`
(v0.68b stub, ~3-4h total work, field-level drift detection). Step 1: add `tauri-specta` + `specta` deps.
Step 2: enable on a single command (e.g. `list_wallets`) for pilot. Step 3: verify drift table
produces same output as `ipc.snapshot.v2.json`. Step 4: extend to all 108 IPCs. Step 5: replace
hand-written `src/ipc.ts` types with generated ones.

**v0.77+** — coverage plateau:
- Branch coverage is the new bottleneck (81.64% vs stmts 83.19%)
- Markets.tsx, Analysis.tsx, History.tsx, LlmPerf.tsx — all have under-tested conditional branches
- ModelLab 5 tabs (Train / Sweep / Promote / Backtest / Archive) need tab-switching tests
- Density round 3 (ts-routes-components-lib 15% → 20%) — deferred since v0.70

**Visual regression** — Playwright toHaveScreenshot pipeline (v0.74d §12 roadmap). Manual
visual review continues to be the safety net until pipeline is in place.
