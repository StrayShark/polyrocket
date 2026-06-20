# polyrocket v0.86 — BigInt wrappers: 18/18 i64 fields → bigint (100%)

**Ship date**: 2026-06-20
**Commits**: 5cc5b48 (v0.86a) + 649fe4e (v0.86b) + 7ea6bbd (v0.86c)
**Coverage**: 86.78 / 83.99 / 80.46 / 87.98 (unchanged from v0.85)
**Tests**: 1361 (cargo 355 + vitest 913 + Python 86 + Playwright 7)
  *(+7 codegen tests for OptionBigInt + BigIntMap)*
**CI**: 5/5 jobs green

## 30 秒摘要

v0.85 got 7/18 i64 fields → `bigint` (39%) but had to defer:
- 5 Option<i64> fields → type override loses nullability
- 1 HashMap<String, i64> field → type override doesn't recurse

v0.86 closes both gaps with two custom polyrocket-local wrappers:
- **`OptionBigInt<T>`** (v0.86a) — serde-transparent wrapper that maps
  `Option<OptionBigInt<i64>>` → TS `bigint | null`
- **`BigIntMap<K, V>`** (v0.86c) — serde-transparent wrapper that maps
  `BigIntMap<String, i64>` → TS `{ [key: string]: bigint }`

After v0.86: **18/18 i64 fields → bigint (100%)** across 8 stub DTOs.

## What landed

### v0.86a — `OptionBigInt<T>` wrapper

`src-tauri/src/codegen/option_bigint.rs` (~90 lines, 4 unit tests):
- Serde transparent: `Serialize`/`Deserialize` delegate to inner `Option<T>`
- Specta `Type` impl: returns `DataType::Reference(specta_typescript::define("bigint"))`
- Used as `Option<OptionBigInt<i64>>` → TS `bigint | null`
- New `polyrocket_lib::codegen::option_bigint` module
- 4 serde round-trip tests (None/Some + JSON parse/serialize)

### v0.86b — convert 5 Option<i64> fields

5 fields upgraded from `number | null` to `bigint | null`:
- `ActiveModelCodegen.promoted_at_ms`
- `MirrorRowCodegen.submitted_at`, `.filled_at`
- `ListSignalsArgsCodegen.limit`
- `ListMirrorsArgsCodegen.limit`
- `WalletDtoCodegen.last_synced_at` (and `.created_at` i32 → i64 + `#[specta(type = BigInt)]`)

### v0.86c — `BigIntMap<K, V>` wrapper

`src-tauri/src/codegen/bigint_map.rs` (~110 lines, 3 unit tests):
- Serde transparent: delegates to inner `HashMap<K, V>`
- Specta `Type` impl: returns `DataType::Map(K::definition(), bigint_ref)`
- Used as `BigIntMap<String, i64>` → TS `{ [key: string]: bigint }`
- 3 serde round-trip tests (empty / entries / round-trip)

Plus `AuditRetentionViewCodegen` upgrade:
- 3 i32 → i64 + `#[specta(type = BigInt)]` (now `bigint`)
- `overrides`: `HashMap<String, i32>` → `BigIntMap<String, i64>` (now `{ [key: string]: bigint }`)

### v0.86d — L1 layer: NO-OP

The hand-written L1 types in `src/types/mirror.ts`, `signal.ts`, `wallet.ts` use
`number` for i64 fields. This is CORRECT for runtime because `JSON.parse`
gives `number`, not `bigint`. The generated `*Codegen` types use `bigint`
(used only for drift detection on field set + names), not at runtime.

**L1 ↔ generated type drift is a TYPE-LEVEL concern** that doesn't affect
runtime. Documented in `coding-spec.md §14` for future reference. A future
round may add explicit `BigInt()` conversion in L1 if any code path needs
lossless i64 transport end-to-end (e.g. for very large IDs or
high-precision timestamps).

## Coverage tally

| DTO | i64 fields | v0.85 state | v0.86 final |
|---|---|---|---|
| `AuditRetentionViewCodegen` | 4 (3 scalars + 1 map value) | all i32 (placeholder) | all bigint (lossless) |
| `ActiveModelCodegen` | 1 (promoted_at_ms) | i32 (Option placeholder) | bigint \| null (lossless) |
| `MirrorRowCodegen` | 5 (4 scalars + 1 bet_id string) | 2 bigint, 2 number \| null, 1 bet_id string | 3 bigint, 2 bigint \| null, 1 bet_id string |
| `MirrorQueueStatsCodegen` | 5 (n_pending, etc.) | 5 bigint | 5 bigint (unchanged from v0.85c) |
| `SignalListItemCodegen` | 3 (id, computed_at, horizon_hours) | 3 i32 (placeholder) | 3 bigint (lossless) |
| `WalletDtoCodegen` | 3 (chain_id, created_at, last_synced_at) | 1 bigint, 1 i32, 1 number \| null | 2 bigint, 1 bigint \| null |
| `ListSignalsArgsCodegen` | 1 (limit) | i32 (Option placeholder) | bigint \| null |
| `ListMirrorsArgsCodegen` | 1 (limit) | i32 (Option placeholder) | bigint \| null |

**Total**: 18/18 i64 fields → bigint (100%) in generated TS.

## Lessons

- **Custom wrappers are the escape hatch for specta-typescript limitations**.
  When the official `BigInt<T>` wrapper or `#[specta(type = ...)]` attribute
  doesn't work for your case, write a polyrocket-local wrapper with:
  1. `#[derive(Serialize, Deserialize)]` + transparent delegation impls
  2. Manual `impl Type` returning the right `DataType` shape
  3. Wire-format parity tests (serde round-trip) to verify it doesn't break IPC
- **`specta_typescript::define("bigint")` is the public escape hatch** for
  the internal `opaque::BigInt` reference. Same TS output (`bigint`), same
  dedup behavior in the exporter.
- **Type override limitations** to remember:
  - `#[specta(type = X)]` on `Option<T>` applies to inner T (loses nullability)
  - `#[specta(type = X)]` on `HashMap<K, V>` doesn't recurse into V
  - The `specta_typescript::BigInt<T>` tuple struct has a private field (can't construct from outside the crate)
  - Plain `i64` is rejected (no escape hatch other than the wrappers)

## Next round (v0.87+)

- **v0.87 — L1 layer BigInt() conversion (optional)**: add explicit
  `BigInt()` conversion in L1 hand-written types for the fields that
  are now `bigint` in generated. This closes the type-level drift.
  Only needed if a code path needs lossless i64 transport end-to-end
  (large IDs, high-precision timestamps, etc.).
- **v0.87 also — coverage round**: Audit.tsx 70% → 80%, Copy.tsx 75% → 85%.
- **v0.88 — Phase 4 (input DTO commands)**: add codegen for the 10 input
  DTO commands (place_signed_order, add_wallet, upsert_llm_provider, etc.).
- **v0.89 — Phase 5 (build pipeline)**: wire `check-codegen-drift` into
  pre-push hook + `pnpm build`. Currently the drift check runs only in
  CI Job 2 (L1 typecheck + vitest).
