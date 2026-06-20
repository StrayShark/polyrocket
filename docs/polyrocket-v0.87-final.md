# polyrocket v0.87 — coverage round: Audit + Copy branch expansion

**Ship date**: 2026-06-20
**Commits**: 08f76de (v0.87a) + 78185da (v0.87b)
**Coverage**: 87.00 / 84.10 / 80.88 / 88.17 (was 86.78 / 83.99 / 80.46 / 87.98)
**Tests**: 1374 total (cargo 355 + vitest 926 + Python 86 + Playwright 7)
**CI**: 5/5 jobs green

## 30 秒摘要

v0.86 ship log mentioned a planned "Audit.tsx 70→80%, Copy.tsx 75→85%" round
for v0.87. Actual baseline was higher (88.37 / 79.54), so the round
expanded to:
- **Audit.tsx**: 88.4 → 90.7% stmts (+2.3pp)
- **Copy.tsx**: 79.5 → 88.6% stmts (+9.1pp)
- **Project**: 86.78 → 87.00 stmts / 83.99 → 84.10 branches

## What landed

### v0.87a — Audit.tsx cell renderer tests (7 tests)

7 new tests targeting the inline cell renderers in the DataTable column
definitions. v0.62a + v0.70a + v0.72c tests render the table with SAMPLE
data but don't explicitly assert each cell's content type; v8 coverage
was missing the cell arrow bodies.

- `at` column cell → fmtDateTime output
- `actor` column cell → <Pill kind="muted">
- `action` column cell → <span> with action text
- `result` column cell → <Pill kind="bull"|"bear"> (line 109)
- `target` column cell: null target → em-dash fallback
- `target` column cell: non-null target → <code>
- ErrorState on listAuditLog rejection + retry (line 174)

### v0.87b — Copy.tsx additional tests (6 tests)

6 new tests targeting the remaining uncovered branches:

- `addTargetMut.onError` → toast.error (line 249)
- `minEdgePct` clamping: non-numeric input → 0 (line 300)
- Paper mode banner with n=0 (empty paper_fills)
- Paper mode banner with n>0 (3 hypothetical fills)
- TargetRow clipboard copy button writes address + success toast
- addTargetMut success → invalidates copy-targets query + closes modal

## Coverage delta

| File | v0.86 final | v0.87 final | Δ stmts | Δ branches |
|---|---|---|---|---|
| `Audit.tsx` | 88.37 / 95.23 | 90.69 / 95.23 | +2.3pp | unchanged |
| `Copy.tsx` | 79.54 / 86.84 | 88.63 / 92.10 | +9.1pp | +5.3pp |
| **Project** | 86.78 / 83.99 | 87.00 / 84.10 | +0.2pp | +0.1pp |

All 4 dimensions still pass thresholds (86/83/79/87):
- stmts:    87.00% (headroom 1.0pp)
- branches: 84.10% (headroom 1.1pp) — bumped from 0.99pp at v0.86
- funcs:    80.88% (headroom 1.88pp)
- lines:    88.17% (headroom 1.17pp)

**Branches bumped from 0.99 to 1.1pp** (just over the 1.0 safe threshold).

## Threshold bump decision

We could bump branches from 83 → 84 (actual 84.10, 0.1pp headroom) but
that's below the 0.5pp safe minimum (per the "Coverage ratchet policy"
memory). v0.87 maintains 86/83/79/87.

## Next round (v0.88+)

- **v0.88 — Phase 4 (input DTO commands)**: add codegen for the 10 input
  DTO commands (place_signed_order, add_wallet, upsert_llm_provider, etc.).
  The 5 codegen commands in v0.81 already use the input DTO pattern
  (ComputeAllocationArgsCodegen, etc.).
- **v0.89 — coverage round 2**: Bankroll.tsx 71→80%, History.tsx 85→90%.
  These are pure-render / pure-form routes with branch-rich UI states.
- **v0.90 — Phase 5 (build pipeline)**: wire `check-codegen-drift` into
  pre-push hook + `pnpm build`. Currently the drift check runs only in
  CI Job 2.
- **v0.91 — refactor promoteAllMut into a custom hook**: fill the deferred
  v0.83 branch coverage gap (TrainProgress 4-way onSuccess branches).
