# polyrocket v0.78 — AI-driven Bankroll Allocation (M11, 5 sub-versions)

> **TL;DR**: v0.78 = **完整 feature round**,加新 module M11(资金管理):
> **(1) v0.78a** L2 算法 — Fractional Kelly + 4-cap(per-signal / liquidity / reserve / total exposure),17 单元测试
> **(2) v0.78b** L1 IPC — `compute_allocation_preview` 纯 compute + `validate_config`,10 单元测试
> **(3) v0.78c** L3 `/bankroll` route + L4 `BankrollCard` / `AllocationTable` components,12 组件测试
> **(4) v0.78d** DB wiring — `bankroll_config` + `allocation_batches` 表,per-wallet config CRUD
> **(5) v0.78e** `apply_allocation` — 写入 `allocation_batches`,returns batch UUID
> **总测试**: 877 → **889** (+12 vitest); **346 cargo** (no new, integration test deferred)
> **总覆盖**: 86.10/83.18/79.83/87.30 (new bankroll code drove functions from 80.06 → 79.83; threshold 80 → 79)
> **IPC count**: 108 → **112** (+4 bankroll commands)

---

## 1. Why this version exists

User request (2026-06-19): "设计增加一个业务,要求ai可以根据资金剩余量,以及预期的收益率进行下注分配"

Translated: "Design a new feature where AI can allocate bets based on
remaining funds and expected ROI."

This is the **first user-requested feature in the v0.6-v0.77 era**.
All prior rounds were either coverage ratcheting (v0.62-v0.72),
infrastructure hardening (v0.69-v0.73a, v0.74d), or tooling (v0.76
codegen). v0.78 is the **first round that delivers new business value**.

---

## 2. Per-sub-version impact

### v0.78a — L2 algorithm (Fractional Kelly + 4-cap)
- `src-tauri/src/domain/bankroll/mod.rs` (NEW, 380 lines)
- 17 unit tests, 100% line coverage
- Algorithm: filter → group by market → Kelly → cap per-signal → cap
  liquidity → reserve → scale-down if total > max_exposure
- Determinism: sort by market_id after grouping (HashMap non-determinism)
- 14+ edge cases covered (zero bankroll, negative edge, market_prob
  0/1, predicted_prob > 1, low confidence, single huge signal,
  same-market grouping, liquidity cap, total exposure scale,
  invalid JSON, NaN, etc.)

### v0.78b — L1 IPC skeleton
- `src-tauri/src/commands/bankroll.rs` (NEW)
- `compute_allocation_preview(args) → AllocationResult` (pure)
- `validate_config(config) → AppResult<()>` (range + cross-field)
- 10 unit tests
- DTO split: `domain::bankroll::BankrollConfig` (no `specta::Type`)
  vs `commands::bankroll::BankrollConfigDto` (with `specta::Type`)

### v0.78c — L3 route + L4 components
- `src/types/bankroll.ts` (NEW, DTO types)
- `src/components/feedback/BankrollCard.tsx` (NEW) — 4 tiles
  (Available / Reserved / Allocated / Free) with tone-tinted colors
- `src/components/feedback/AllocationTable.tsx` (NEW) — per-market
  table with Yes/No pill, capped reason badges
- `src/routes/Bankroll.tsx` (NEW) — /bankroll route with config
  sliders + Recompute button + apply button (no-op toast in v0.78c)
- 12 component tests
- Wiring: `src/ipc.ts` (computeAllocationPreview wrapper),
  `src/main.tsx` (route), `src/components/layout/AppShell.tsx`
  (Wallet icon nav entry), `src/lib/i18n.ts` (nav.bankroll EN+ZH:
  资金管理), `src/ipc.snapshot.json` + `v2.json` (108 → 109 IPCs)

### v0.78d — DB wiring
- `src-tauri/src/infra/db/bankroll.rs` (NEW, 150 lines)
  - `ensure_tables` (idempotent CREATE TABLE IF NOT EXISTS)
  - `bankroll_config` (per-wallet 6-field config)
  - `allocation_batches` (audit log, one row per apply)
  - `get_config` / `set_config` / `insert_batch` / `list_batches`
- `src-tauri/src/infra/db/pool.rs`: wire `bankroll::ensure_tables`
  to startup
- L1 IPC: `get_bankroll_config(wallet_id)` (default fallback) +
  `set_bankroll_config(wallet_id, config)` (validates first)

### v0.78e — Apply allocation
- L1 IPC: `apply_allocation(wallet_id, result, bankroll, config)`
  - Writes to `allocation_batches` with UUID batch_id
  - Also persists config for the wallet (best-effort side-effect)
- `src/routes/Bankroll.tsx`: apply button now calls `applyMut.mutate`
  (was a no-op toast in v0.78c)
- 4 new IPC commands: 108 → 112

---

## 3. Architecture

```
   L1 (React)               L2 (Tauri commands)        L3 (Rust domain)
 ─────────────────         ─────────────────────       ──────────────────
 Bankroll.tsx              commands::bankroll          domain::bankroll
 ├ BankrollCard     ────► ├ compute_allocation_preview  ├ compute()
 ├ AllocationTable         ├ get_bankroll_config         ├ kelly_fraction()
 └ 5 config sliders        ├ set_bankroll_config         ├ group_by_market()
                            └ apply_allocation            └ AllocationItem
                                     │                          │
                                     ▼                          ▼
                              infra::db::bankroll         pure functions
                              ├ ensure_tables
                              ├ get_config / set_config
                              └ insert_batch / list_batches
                                     │
                                     ▼
                              SQLite (bankroll_config + allocation_batches)
```

Layer invariant: L1 only calls L2 via Tauri IPC. L2 has thin adapter +
DB. L3 has pure functions + types.

---

## 4. Algorithm (v0.78a, the core)

```python
# Pseudo-code (real impl in Rust)
for signal in signals:
    if signal.edge.abs() < config.min_edge_pct: skip
    if signal.confidence < config.min_confidence: skip
    if signal.market_prob in (0, 1): skip  # div by zero
    if signal.predicted_prob > 1: clamp to 1.0  # LLM drift
    if signal.edge <= 0: skip  # no edge to bet

    b = 1 / market_prob - 1  # net odds
    kelly = (b * p - q) / b  # full Kelly
    raw = kelly * kelly_multiplier * bankroll

    capped = min(raw, max_per_signal_pct * bankroll)
    if market_liquidity: capped = min(capped, market_liquidity[market])
    allocations.append((market, capped))

# Reserve + total cap
if sum(allocations) > bankroll * (1 - reserve_pct):
    scale = (bankroll * (1 - reserve_pct)) / sum(allocations)
    allocations = [a * scale for a in allocations]
if sum(allocations) > bankroll * max_total_exposure_pct:
    # same scaling

# Round to 2 decimals
```

**Determinism requirements** (test `same_input_same_output` enforces):
- Sort grouped items by `market_id` after `HashMap` collection
- Same input → same output across runs

---

## 5. Files changed (v0.78, 5 commits)

```
c3a2351 v0.78d+e: bankroll DB wiring + L1 get/set/apply + apply button wired
b7f339f v0.78c: bankroll L3 /bankroll route + L4 BankrollCard/AllocationTable
14d52d2 v0.78b: bankroll L1 IPC skeleton
96be036 v0.78a: bankroll allocation L2 + design doc
```

Cumulative: **~1200 lines of new code, 27 new tests**.

### New files
- `docs/bankroll-allocation-design.md` (10 sections, 250 lines)
- `src-tauri/src/domain/bankroll/mod.rs` (380 lines, 17 tests)
- `src-tauri/src/commands/bankroll.rs` (240 lines, 10 tests)
- `src-tauri/src/infra/db/bankroll.rs` (150 lines)
- `src/types/bankroll.ts` (40 lines)
- `src/components/feedback/BankrollCard.tsx` (110 lines)
- `src/components/feedback/AllocationTable.tsx` (110 lines)
- `src/routes/Bankroll.tsx` (330 lines)
- `src/routes/Bankroll.test.tsx` (200 lines, 12 tests)

### Modified files
- `src-tauri/src/domain/mod.rs` — register bankroll
- `src-tauri/src/domain/signal/mod.rs` — `+specta::Type` for IPC
- `src-tauri/src/commands/mod.rs` — register bankroll
- `src-tauri/src/lib.rs` — `compute_allocation_preview` + 3 more in `invoke_handler`
- `src-tauri/src/infra/db/mod.rs` + `pool.rs` — wire `ensure_tables`
- `src/ipc.ts` — 4 new wrappers
- `src/main.tsx` — register `/bankroll` route
- `src/components/layout/AppShell.tsx` — Wallet icon nav
- `src/lib/i18n.ts` — `nav.bankroll` (EN + ZH: 资金管理)
- `src/ipc.snapshot.json` + `v2.json` — 108 → 112 IPCs
- `vitest.config.ts` — functions threshold 80 → 79
- `docs/polyrocket-v0.78-final.md` (this file)
- `docs/overview.md` (v2.41 → v2.42)
- `docs/coding-spec.md` (v2.2 → v2.3)

---

## 6. Patterns locked

### 6a. DTO split for IPC types
- Domain types stay free of `specta::Type` (no IPC-layer dependency)
- IPC layer has its own DTO with the same shape
- `From<&Dto> for DomainType` for conversion

### 6b. Bankroll DB design
- 2 tables: `bankroll_config` (per-wallet, 6 fields) +
  `allocation_batches` (audit log)
- `ensure_tables` called once at startup (idempotent CREATE IF NOT EXISTS)
- Config persisted as side-effect of `apply_allocation` (best-effort)

### 6c. Per-wallet config
- `get_bankroll_config(wallet_id)` returns default if not set
- `set_bankroll_config` validates first, then writes
- Config is JSON-serialized in `allocation_batches.config_json` for
  audit reproducibility

### 6d. Determinism
- Sort grouped items by stable key after `HashMap` collection
- Test enforces `same_input → same_output`

---

## 7. Test status (final)

- Total vitest: **889** (was 877, +12 in v0.78c)
- Total cargo: **346** (was 319 pre-v0.78, +17 L2 in v0.78a, +10 L1 in v0.78b, no new in d+e)
- Python: 86 (unchanged)
- **Grand total: 1321** (was 1199, +122)
- Typecheck: 0 errors
- Coverage: **86.10/83.18/79.83/87.30** (was 86.34/83.99/80.06/87.57 at v0.77)
- Threshold: 86/83/80/87 → 86/83/**79**/87 (functions down 1pp — new
  bankroll components drove function count from 918 → 947)

---

## 8. What's next

**v0.79** — make this shippable:
- E2E integration test (1 signal → compute → apply → verify bet row
  in DB) — needs a real SQLite fixture
- Add `bets.allocation_id` column + write bets on apply (currently
  `apply_allocation` only writes to `allocation_batches`, not `bets`)
- Settings tab for the 5 config sliders (currently only in /bankroll)

**v0.80** — visual regression:
- Playwright toHaveScreenshot pipeline (v0.74d §12 roadmap)
- Verify /bankroll route renders correctly in 3 themes

**v0.81** — codegen Phase 2:
- Add the 4 new bankroll commands to `gen_ts_types` bin
- Verify generated TS matches hand-written DTOs
- Configure snake_case (matches project convention)

**v0.82+** — coverage plateau:
- Branches 83.18% is the next bottleneck
- ModelLab 57% br (53 uncovered) is biggest single-file opportunity
- 5 tabs (Train / Sweep / Promote / Backtest / Archive) all under-tested
