# polyrocket v0.83 — ModelLab branches round 4 (final)

**Ship date**: 2026-06-20
**Commits**: af57b67 (v0.83c) + f10093f (v0.83a) + 90682d6 (v0.83b)
**Coverage**: stmts 86.78% / branches 83.99% / funcs 80.46% / lines 87.98%
**Tests**: 1354 total (cargo 348 + vitest 913 + Python 86 + Playwright 7)
**Threshold**: 86/83/79/87 — all 4 pass (headroom 0.78/0.99/1.46/0.98pp)

## What landed

### v0.83a — ModelLab eventListeners (branches 57.2% → 61.3%)
7 new tests for the 2 main useEffect listeners (`onTrainStarted` +
`onAutoPromoteFinished`) and their callbacks. Each test captures the
listener registration callback at `vi.mock` setup time, then invokes
it directly with crafted events. Covers: listener registration,
cleanup races (cancelled guard), notification permission paths.

### v0.83b — ModelLab autoPromote mutations (branches 61.3% → 71.8%)
7 new tests for `autoPromoteMut.onSuccess` 4-way branch:
- `r.promoted=true` (with/without both briers) → toast.success
- `r.skipped=true` (with/without margin) → toast.info
- `r.promoted=false` (else branch) → toast.error
- `onError` (mock throws) → toast.error

Plus 3 query invalidation tests confirm llm-performance +
sidecar-active-model + promote-history refresh on every code path.

### v0.83c — ModelLab trainMut mutations (branches 71.8% → 72.6%)
7 new tests for `trainMut.onSuccess` 4-way branch:
- `r.status=completed` + `best_brier != null` → toast with brier detail
- `r.status=completed` + `best_brier == null` → toast with empty detail
- `r.status=failed` → toast.error + lastCandidate cleared
- `onError` (mock throws) → toast.error

Plus 3 invalidation tests confirm llm-performance + sidecar-active-model
queries refresh on all 3 response paths.

## What's deferred

- **v0.83d (planned) — `promoteAllMut.onSuccess` 4-way branch** in
  TrainProgress. Skipped because TrainProgress's `onTrainStarted`
  listener has a `e.job_id !== jobId` guard that returns early when
  `activeTrainJobId` is null. Triggering TrainProgress mount requires
  firing ModelLab's `onTrainStarted` listener from a test, but
  TrainProgress's later listener registration overwrites the captured
  cb in the mock factory. Two paths forward: (1) refactor
  `promoteAllMut` into a custom hook (testable without TrainProgress
  mount); (2) extract the 4 branches into a pure function. Either
  approach is a v0.85+ refactor — out of scope for v0.83.
- **Modal-flow branches** (ModelComparison + BacktestReport IIFEs):
  covered by 11 lines of `weightsByJobid` / `entries` IIFE bodies
  that only execute when `compareOpen=true` and `selectedForCompare`
  has 2+ entries. Reaching these requires clicking 2 checkboxes +
  the compare-models-btn, which is complex flow setup. Lines 102,
  702-712, 737, 752 are still red. Marginal coverage gain for
  significant test complexity. Defer to v0.85+.

## Coverage ratchet policy

- **Branches** is the tightest dimension (0.99pp headroom at v0.83 final).
  Branches naturally lag stmts as coverage approaches 80%+ because:
  - 3-way/4-way branch conditions in mutations (autoPromote, promoteAll,
    trainMut) need 4+ tests each to fully cover
  - React Query invalidation lists add a branch per query
  - IIFEs in JSX props are reported as missed by v8 even when executed
- **Threshold bump policy**: only bump a dimension when actual ≥ new
  threshold AND ≥ 0.5pp headroom from current. v0.83 maintains 86/83/79/87
  (no bumps) — branches has 0.99pp which is the minimum safe headroom.

## Next round (v0.84+)

- **v0.85 — codegen Phase 4**: enable `serde` feature on `specta-typescript`,
  use `Number<i64>` wrapper for lossless transport, L1 layer BigInt churn
  (~6 .ts files). Then Phase 5 (build pipeline: codegen + drift-check
  wired into `pnpm build`).
- **v0.86 — refactor `promoteAllMut` into a custom hook** so its 4
  branches are testable without TrainProgress mount dance. Add 7 tests
  for the 4-way `onSuccess` branch (r.ok/partial/all-failed/onError)
  + 3 query invalidations. Should push ModelLab branches past 75%.
- **v0.87 — coverage round**: Audit.tsx 70% → 80%, Copy.tsx 75% → 85%,
  Settings.tsx 77% → 80%. These are pure-render / pure-form routes
  with many branch-rich UI states (similar to v0.78-v0.81 round).
