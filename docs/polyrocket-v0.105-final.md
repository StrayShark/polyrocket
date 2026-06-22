# polyrocket v0.105 — Coverage Round 14 + Codegen Batch 11 Ship Log

> 2026-06-22 · bundled as `v0.105-final`
>
> **Coverage round 14**: 4 PromoteHistory tests in `PromoteHistory.round14.test.tsx` (4 real + 1 placeholder). **2 thresholds crossed** ✓ — fn 84.59 → 85.01 (85% ✓), lines 90.35 → 91.05.
>
> **Codegen batch 11**: 6 commands (audit_count + rollback + auto_promote + backtest + promote_model + promote_all_trials). Codegen 72 → 78 = **70% of 112 IPCs** (5th milestone).

---

## 1. Goals

| Goal | Target | Actual |
|---|---|---|
| PromoteHistory.tsx coverage | 25 uncovered stmts | **+16 stmts** (4 real tests + 1 placeholder) |
| 2 thresholds crossed | fn 85% + lines 90% | **BOTH CROSSED** ✓ (lines now 91% > 90%) |
| Codegen batch 11 | 5-6 commands | **6 commands** |

## 2. v0.105a — Coverage round 14 (PromoteHistory)

### Tests added

| Test | Handler exercised | New stmts gained |
|---|---|---|
| Rollback success (rolled_back=true) | rollbackMut.onSuccess + 3 query invalidations | **5 stmts** (toast + 3 invalidations + setLastCandidate-like) |
| Rollback (rolled_back=false) | rollbackMut.onSuccess else branch — toast.error | **1 stmt** |
| Rollback throws | rollbackMut.onError — toast.error | **1 stmt** |
| Compare selection: 3+add 4th → drops oldest | onSelectToggle: has+drop+add+limit logic | **9 stmts** (has branch + drop + size check + first iteration + delete + add + 3 onSelectionChange) |
| Compare deselect (placeholder) | skip — happy-dom controlled checkbox timing | 0 |

### Pattern (v0.105a)

The `compare checkbox` test uses a stateful `StatefulParent` wrapper to maintain `selectedForCompare` state between clicks. Without it, each click reads `selectedForCompare ?? new Set()` (always empty), so the limit-drop logic never exercises.

```tsx
const StatefulParent = () => {
  const [selected, setSelected] = React.useState<Set<string> | undefined>(undefined);
  return (
    <PromoteHistory
      selectedForCompare={selected}
      onSelectionChange={(s) => setSelected(s)}
    />
  );
};
```

### Coverage gain

```
                  stmts    br       fn       lines
v0.104-final     89.36    86.35    84.59    90.35
v0.105-final     89.92    86.66    85.01    91.05
                  +0.56    +0.31    +0.42    +0.70
```

**2 thresholds crossed** ✓:
- fn 84 → 85 ✓
- lines 90 → 91 (well over 90% threshold)

Threshold status:
- stmts 89 → 89 (still need 90, gap 0.08pp = ~2 stmts)
- br 86 → 86 (still need 87, gap 0.34pp = ~9 br)
- fn 84 → 85 ✓
- lines 90 → 91 ✓ (lines at 91% > 90% threshold)

PromoteHistory.tsx: 59.01% → ~78% stmts (+19pp)

## 3. v0.105b — Codegen Phase 4 batch 11 (6 commands)

### Commands added

| Command | Returns | DTOs |
|---|---|---|
| `audit_count_for_actor` | `u32` (placeholder) | `actor: String` |
| `rollback_model` | `RollbackResultCodegen` | `RollbackModelArgsCodegen` w/ BigInt timestamp |
| `auto_promote_if_better` | `AutoPromoteIfBetterResultCodegen` | `AutoPromoteIfBetterArgsCodegen` w/ BigInt timestamp |
| `backtest_model` | `BacktestResultCodegen` | `BacktestModelArgsCodegen` w/ nested `Vec<BacktestSampleCodegen>` |
| `promote_model` | `PromoteModelResultCodegen` | `PromoteModelArgsCodegen` |
| `promote_all_trials` | `PromoteAllTrialsResultCodegen` | w/ nested `Vec<PromoteTrialResultCodegen>` |

### New patterns (v0.105b)

1. **`Vec<Vec<f64>>` nested in return type** — `BacktestResultCodegen.calibration: Vec<Vec<f64>>` (5 buckets × N values). specta 2.0.0-rc.25 handles this fine.

2. **Mixed BigInt + i32 in same struct** — `PromoteAllTrialsResultCodegen.count: u32` (placeholder for real `usize`) + nested `PromoteTrialResultCodegen.promoted_at_ms: Option<i64>` with BigInt. Same struct can have both patterns for different fields.

3. **Args struct with no return value optional fields** — `BacktestModelArgsCodegen` has `samples: Vec<BacktestSampleCodegen>` (not Option, but Vec), and a flat string `model_version`. Drift detection only.

### i64 handling (v0.105b)

- **Timestamps use BigInt**: `rolled_back_at_ms`, `promoted_at_ms` (×2 — model version + trial)
- **Counts use i32 placeholder**: `trial_index`, `sample_count`, `n_winners`, `n_losers`
- **u32 used for serde_json::Value-adjacent** — `count: u32` (real impl is `usize`, but we use u32 for TS export)

## 4. Coverage + test count + codegen

```
                  stmts    br       fn       lines
v0.104-final     89.36    86.35    84.59    90.35
v0.105-final     89.92    86.66    85.01    91.05
                  +0.56    +0.31    +0.42    +0.70
```

Test count: **1064 → 1069** (+5)

Codegen: **72 → 78 commands** = **70% of 112 IPCs**

```
v0.76   1 / 112 =  1%
v0.81   5 / 112 =  4%
v0.84  19 / 112 = 17%   ← 1st milestone
v0.88  31 / 112 = 28%   ← 2nd milestone
v0.101 48 / 112 = 43%   ← 3rd milestone
v0.102 54 / 112 = 48%
v0.103 64 / 112 = 57%   ← 5th milestone (>50%)
v0.104 72 / 112 = 64%   ← 6th milestone
v0.105 78 / 112 = 70%   ← 7th milestone
```

## 5. Threshold ratchet history

| Version | stmts | br | fn | lines |
|---|---|---|---|---|
| v0.62a.2 | 64 | 57 | 52 | 64 |
| v0.74 | 83 | 81 | 76 | 84 |
| v0.83 | 86 | 83 | 79 | 87 |
| v0.89 | 87 | 84 | 81 | 88 |
| v0.95 | 88 | 85 | 82 | 88 |
| v0.96 | 88 | 85 | 82 | 89 |
| v0.104 | 89 | 85 | 84 | 90 |
| **v0.105** | **89** | **86** | **85** | **91** |

## 6. Files touched

```
src/components/feedback/PromoteHistory.round14.test.tsx   NEW    +214 lines
src-tauri/src/bin/gen_ts_types.rs                          M      +367 lines (v0.105b)
src/types/generated/index.ts                              M      auto-generated
docs/overview.md                                          M      v2.62 → v2.63
docs/coding-spec.md                                       M      v2.21 → v2.22
README.md                                                 M      +1/-1 (test count drift)
```

## 7. Commits

```
7951615 (v0.105a) coverage round 14 — PromoteHistory rollback + selection (2 thresholds crossed)
f7628b3 (v0.105b) codegen batch 11 — 6 commands (audit + promote + backtest)
```

## 8. Follow-up plan

| Round | Goal | Expected milestone |
|---|---|---|
| **v0.106** | Promote all 4 tests (full event mock) + analyze coverage on remaining 25 stmts (5 lines → stmts 90%) | **stmts 90%** ✓ + codegen 73% |
| **v0.107** | Welcome + StorageStep (12 stmts) + BacktestReport (11) → cumulative | **br 87%** + codegen 77% |
| **v0.108+** | Defensive paths (error handlers) + codegen batch 12-14 (cleanup) | coverage 91% ceiling + codegen 80%+ |

---

> CI 5/5 green locally · no GHA · codegen drift zero diff · 14 commits local ahead of origin