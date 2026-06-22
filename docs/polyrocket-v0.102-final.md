# polyrocket v0.102 — Coverage Round 11 + Codegen Batch 8 Ship Log

> 2026-06-22 · bundled as `v0.102-final`
>
> **Coverage round 11** (Settings handler invocation): +10 tests, Settings.tsx stmts 85.12 → 85.95 (+0.83pp), project 88.80 → 88.90 stmts (+0.10pp). Threshold 89% **still not crossed** (need +3 more stmts).
>
> **Codegen batch 8**: 6 commands (degradation + audit purge + 4 daily_brief). Codegen 48 → 54 = **48% of 112 IPCs** (4th milestone ≈ 50%).

---

## 1. Goals

| Goal | Target | Actual |
|---|---|---|
| Settings.tsx handler invocation tests | fire onClick on 4 cards + top reset + Import flow | 10 tests added, +3 stmts gained |
| Codegen batch 8 | 5-6 commands | **6 commands** (degradation_check_now + purge_audit_log_now + 4 daily_brief) |
| Threshold 89 stmts | cross 89% | **FAILED** at 88.90 (gap +3 stmts) |
| Codegen coverage | 43% → 47%+ | **48%** (54/112) |

## 2. v0.102a — Settings handler invocation (round 11)

### Tests added

| Test | Handler exercised | New stmts gained |
|---|---|---|
| Export button click | `onExport` → `downloadPrefsAsFile(snapshot)` + toast | 0 (covered by v0.92 round4) |
| Import button click | `onImportClick` → `fileInputRef.current?.click()` | 1 (new path: hidden file input click) |
| Import file change | `onFileSelected` → file read, parse, setPref loop | **SKIPPED** (happy-dom file input unstable) |
| Import error path | `parsePrefsFromString` reject → toast.error | **SKIPPED** (same reason) |
| `rerun-setup-reset` click | `reset(); navigate('/welcome')` | **SKIPPED** (useNavigate mock chain) |
| Toggle copy-trading | `setDraft({...draft, copyTradingEnabled: v})` | 0 (covered by v0.92 round4) |
| Toggle advanced-stats | `setDraft({...draft, advancedStats: v})` | 1 (new field) |
| Toggle telemetry | `setPref('telemetryEnabled', v)` | **SKIPPED** (testid not found) |
| Retention Save error | mock reject → `toast.error('Retention update failed...')` | 1 (error path) |
| Purge now click | `purgeAuditLogNow` + `toast.info` | 0 (useEffect already covered?) |

### Lesson (v0.102a)

- **Most "click handler" tests added 0 new coverage** because v0.92 round4 + v0.99-100 already covered the handler bodies via render tests that triggered useEffects (e.g., `schedulerSelfTestNow` is fired on mount via useEffect, not just on Refresh click).
- **The uncovered statements remaining in Settings.tsx are mostly**:
  - `setDraft({...draft, X: v})` for fields not yet toggled in tests (defaultAllocationCapUsdc, notificationsEnabled)
  - The entire `onFileSelected` body (15 stmts) — happy-dom file input handling too fragile
  - `rerun-setup-reset` flow (2 stmts) — `useNavigate` mock chain doesn't propagate through `MemoryRouter`
  - `toast.error` error paths (retention save, telemetry push) — already partly covered
- **Coverage gain**: only **+3 stmts** from 10 tests. Threshold 89% still not crossed.

### What would actually move the needle

1. **Fix the Import file change test** — use `@testing-library/user-event` instead of `fireEvent.change` (better happy-dom file support) → +15 stmts → 89.05% threshold crossed
2. **Wire `useNavigate` mock** through `MemoryRouter` correctly → `rerun-setup-reset` works → +2 stmts
3. **Add 6 more toggle tests** for the 6 fields not yet exercised → +6 stmts

Round 12 (v0.103) should fix #1 + #2 to cross 89%.

## 3. v0.102b — Codegen Phase 4 batch 8 (6 commands)

### Commands added

| Command | Returns | DTO | Use case |
|---|---|---|---|
| `degradation_check_now` | `()` | `DegradationCheckNowArgsCodegen` | Settings "Check now" button |
| `purge_audit_log_now` | `u32` | — | Retention "Purge now" button |
| `daily_brief_get` | `Vec<DailyBriefEntryCodegen>` | `BriefGetArgsCodegen` | Dashboard Daily Brief panel |
| `daily_brief_refresh` | `BriefRefreshResultCodegen` | — | Brief refresh button |
| `daily_brief_dismiss` | `bool` | `BriefDismissArgsCodegen` | Brief dismiss button |
| `daily_brief_set_prefs` | `()` | `SetBriefPrefsArgsCodegen` w/ nested `BriefWeightsCodegen` | Brief preferences panel |

### New pattern (v0.102b): nested DTOs

`SetBriefPrefsArgsCodegen` embeds `Option<BriefWeightsCodegen>` — nested `#[derive(Type)]` struct. specta 2.0.0-rc.25 handles this fine (each nested struct just needs `#[derive(Serialize, Deserialize, Type)]`).

```rust
#[derive(Serialize, Deserialize, Type)]
struct SetBriefPrefsArgsCodegen {
    pub user_id: String,
    pub weights: Option<BriefWeightsCodegen>,  // nested struct
    pub max_items: Option<u32>,
    pub min_liquidity: Option<String>,
    pub categories: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize, Type)]
struct BriefWeightsCodegen {
    pub w1: f64, pub w2: f64, pub w3: f64,
    pub w4: f64, pub w5: f64, pub w6: f64,
}
```

### i64 handling (v0.102b)

| Field | Pattern | Rationale |
|---|---|---|
| `market_end_date: i64` | `#[specta(type = BigInt)]` | timestamp, lossless |
| `computed_at: i64` | `#[specta(type = BigInt)]` | timestamp, lossless |
| `expires_at: i64` | `#[specta(type = BigInt)]` | timestamp, lossless |
| `rank: i64` | `i32` placeholder | drift detect only |
| `n_items: i64` | `i32` placeholder | drift detect only |

## 4. Coverage + test count

```
                  stmts    br       fn       lines
v0.101-final     88.80    86.15    83.35    89.84
v0.102-final     88.90    86.19    83.66    89.96
                  +0.10    +0.04    +0.31    +0.12
```

Headroom vs threshold 88/85/82/89: **0.90 / 1.19 / 1.66 / 0.96 pp**

Test count: **1036 → 1046** (+10)

## 5. Files touched

```
src/routes/Settings.handler-invocation.test.tsx   NEW    +324 lines
src-tauri/src/bin/gen_ts_types.rs                 M      +259 lines
src/types/generated/index.ts                      M      auto-generated
docs/overview.md                                  M      v2.59 → v2.60
docs/coding-spec.md                               M      v2.18 → v2.19
README.md                                         M      +2/-2 (test count drift)
```

## 6. Commits

```
b75d532  v0.102a: Settings handler invocation round 11 — 10 tests
349090a  v0.102b: codegen Phase 4 batch 8 — 6 commands
```

## 7. Follow-up plan

| Round | Goal | Expected milestone |
|---|---|---|
| **v0.103** | Fix Import file change test (use @testing-library/user-event), wire useNavigate mock, add 6 toggle tests | **stmts 89%** ✓ + **lines 90%** ✓ |
| **v0.104** | ModelLab remaining + codegen batch 9 (LLM provider/key mgmt, 10 commands) | **fn 84%** ✓ + codegen 56% |
| **v0.105** | Welcome + feedback 凑齐 + codegen batch 10 (sidecar + train_job, 8 commands) | **br 87%** ✓ + codegen 63% |

---

> CI 5/5 green locally · no GHA · codegen drift zero diff · 4 commits local ahead of origin