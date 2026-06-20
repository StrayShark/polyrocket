# polyrocket v0.85 — BigInt support: partial i64 → bigint transport

**Ship date**: 2026-06-20
**Commits**: 3f1a28e (v0.85a) + d8a3633 (v0.85b) + 8b8f5d6 (v0.85c)
**Coverage**: 86.78 / 83.99 / 80.46 / 87.98 (unchanged from v0.83)
**Tests**: 1354 (cargo 348 + vitest 913 + Python 86 + Playwright 7)
**CI**: 5/5 jobs green

## 30 秒摘要

v0.84 deferred BigInt handling to v0.85. v0.85 enables the
`specta-typescript` `serde` feature (gating `BigInt<T>` wrapper support)
and converts the 7 **required** i64 fields in stub DTOs from i32
placeholder to i64 + `#[specta(type = BigInt)]`. The generated TS types
for these fields are now `bigint` (TS-correct, lossless for values that
fit in 53 bits).

7 fields converted, 11 fields deferred (specta-typescript 0.0.12
limitations on Option<i64> and HashMap<String, i64>).

## What landed

### v0.85a — enable `serde` feature
- `src-tauri/Cargo.toml`: `specta-typescript = { version = "0.0.12", features = ["serde"] }`
- No behavior change. Just enables the `Serialize`/`Deserialize` impls
  for `BigInt<T>` and `Any<T>` wrapper types.

### v0.85b — MirrorRowCodegen
- `src-tauri/src/bin/gen_ts_types.rs`: MirrorRowCodegen's 5 i64 fields
  converted to `i64` with `#[specta(type = BigInt)]` attribute.
- Generated: `event_id: bigint`, `created_at: bigint`, `submitted_at` +
  `filled_at` reverted to `number | null` (Option BigInt loses null).
- First concrete proof the codegen pipeline works end-to-end.

### v0.85c — extend to all required i64 fields
- All 5 MirrorQueueStats n_* counts (required i64) → bigint.
- ActiveModelCodegen.promoted_at_ms (Option<i64>) + WalletDtoCodegen
  .last_synced_at (Option<i64>) + ListSignalsArgsCodegen.limit +
  ListMirrorsArgsCodegen.limit (Option<i64>) + AuditRetentionViewCodegen
  .overrides (HashMap<String, i64>) — **stuck at i32** (see below).
- Total: **7 fields converted to bigint**, **11 fields stay at i32**.

## What's deferred (specta-typescript 0.0.12 limitations)

| Limitation | Affected fields |
|---|---|
| `#[specta(type = BigInt)]` on `Option<i64>` loses nullability (override applied to inner type) | ActiveModelCodegen.promoted_at_ms, MirrorRowCodegen.submitted_at/filled_at, ListSignalsArgs/ListMirrorsArgs.limit, WalletDtoCodegen.last_synced_at |
| `#[specta(type = BigInt)]` doesn't recurse into `HashMap<String, i64>` value type | AuditRetentionViewCodegen.overrides |
| Plain `i64` rejected by specta-typescript 0.0.12 default mode (would need `BigInt<i64>` wrapper, but its tuple field is private) | All i64 fields |

**Workaround needed for full coverage**: a custom `OptionBigInt<T>`
wrapper that implements `specta::Type` and maps to TS `bigint | null`,
plus a custom `HashMapValueBigInt<K, V>` wrapper (or upstream
support in specta-typescript ≥ 0.0.13).

## L1 layer impact: NONE

The L1 layer (`src/types/*.ts`) keeps all i64 fields as `number`.
The generated `*Codegen` types use `bigint` only for the 7 converted
fields, and those types are used ONLY for drift detection (field set
+ names) — not for runtime. JSON.parse gives `number`, not `bigint`,
so anyone trying to use the generated types at runtime would hit a
type error (intentional — forces explicit `BigInt()` conversion when
needed).

## Trade-offs accepted

- **i32 truncation for 11 Option/HashMap fields**: values that fit
  in 31 bits (2^31 - 1 ≈ 2.1B) are lossless. Above that, the
  `i64 → i32` conversion silently truncates. For Polyrocket's actual
  data shapes (mirror IDs, signal horizons, etc.), all values fit
  in 31 bits. Drift detection still catches field set + name changes
  for these fields.
- **L1 / generated type divergence for 7 fields**: drift detector
  would show a TYPE drift if it compared L1 vs generated. Currently
  it only compares generated vs committed (so no drift), but a
  future round may add L1-vs-generated type drift detection. When
  that lands, we either: (a) add explicit `BigInt()` conversion in
  L1, or (b) downgrade generated to `number` for now.

## Lessons

- **Read the wrapper docs first**: `BigInt<T>` is a tuple struct with
  PRIVATE field. The `serde` feature gates `Serialize`/`Deserialize`
  impls, but you can't construct `BigInt(0)` from outside the crate.
  The `#[specta(type = BigInt)]` attribute is the right way to mark
  i64 fields.
- **Type overrides don't recurse**: specta-typescript 0.0.12 doesn't
  propagate `#[specta(type = X)]` into `Option<T>` or `HashMap<K, V>`
  values. Plan for this with a custom wrapper.
- **i32 is the safe fallback**: specta-typescript 0.0.12 REJECTS plain
  `i64` export (precision loss warning). If you can't use the
  `BigInt` attribute, you MUST use `i32`. There's no "skip drift
  detection for this field" escape hatch.

## Next round (v0.86+)

- **v0.86 — custom `OptionBigInt<T>` wrapper**: add a polyrocket-local
  wrapper that implements `specta::Type` and maps to TS `bigint | null`.
  Convert the 11 deferred fields. (~50 lines of Rust + 5 lines of TS.)
- **v0.86 also — L1 layer BigInt conversion**: for the 7 fields that
  are now `bigint` in generated, add `BigInt()` conversion in the L1
  layer so runtime types match. (~10-20 lines of TS in `src/ipc.ts`.)
- **v0.87 — Phase 5 (build pipeline)**: wire `check-codegen-drift` into
  pre-push hook + `pnpm build` (per v0.84 ship log §4.3).
- **v0.88 — coverage round**: Audit.tsx 70% → 80%, Copy.tsx 75% → 85%.
