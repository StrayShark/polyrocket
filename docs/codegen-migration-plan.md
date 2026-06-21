# L1 ↔ L2 Type Generation Migration Plan

> Status: **v0.88 — Phases 1+2+3+4 DONE**. Phase 5 (build pipeline) is next.
> The project uses **hand-written** TS types in `src/types/*.ts` for
> the broader surface, PLUS the v0.76 codegen bin
> (`src-tauri/src/bin/gen_ts_types.rs`) which generates
> `src/types/generated/index.ts` for 19 read-only commands (5 from
> v0.76+v0.81, +14 from v0.84 a/b/c).
>
> Drift is caught by:
>   - `src/lib/ipc-contract-v2.test.ts` (snapshot-based) for the
>     hand-written surface (function names, param types, return types)
>   - `scripts/check-codegen-drift.mjs` (v0.84d) for the codegen
>     surface (field-level rename / type / optional drift on the 19
>     commands)
>   - v0.84+ will close the BigInt (i64/u64) gap via Number<i64> wrapper

## Why migrate to codegen

| Drift type | Hand-written + snapshot test | Codegen |
|---|---|---|
| Function added/removed | ✓ caught | ✓ caught |
| Param count changed | ✓ caught | ✓ caught |
| Param names changed | ✓ caught | ✓ caught |
| Return type changed | ✓ caught | ✓ caught |
| DTO type imports changed | ✓ caught | ✓ caught |
| **DTO field added/removed** | ✗ NOT caught | ✓ caught |
| **Field type changed** | ✗ NOT caught | ✓ caught |
| **Optional → required** | ✗ NOT caught | ✓ caught |
| **Field renamed** | ✗ NOT caught | ✓ caught |

The bottom 5 rows are the real value. Manual sync misses
these constantly — Rust devs add a field, forget to add it
to the TS DTO, and the L1 layer silently sends stale types
to the UI.

## Recommended approach: tauri-specta

`tower-rs/tower` → `oscartbeaumont/specta` → `oscartbeaumont/tauri-specta`.

`tamu/tauri-specta` v2 is the production-grade option for Tauri
v2 apps. It:

1. Provides `#[tauri_specta::specta]` macro that wraps
   `#[tauri::command]` to register the command in a
   `specta::TypeMap`.
2. Provides `specta_typescript::Typescript::default()` +
   `tauri_specta::ts::ExportConfig` to convert the TypeMap
   to TS code.
3. Outputs a `.ts` file that exports `invoke<T>(name, args)`
   wrappers with full type safety.

### Why tauri-specta over ts-rs

- **ts-rs** is `serde`-derived but doesn't auto-track
  `#[tauri::command]` arg types. You'd annotate structs AND
  write TS commands separately.
- **tauri-specta** auto-tracks commands via macro magic.
  One annotation = one source of truth.

## Migration steps (5 phases, ~3-4h total)

### Phase 2: verify generated TS matches hand-written (30min) — **v0.76 + v0.81 DONE**

v0.76 Phase 1: Generated `DashboardKpis` interface. Confirmed match
with hand-written `src/types/shared.ts`.

v0.81 Phase 2: Added 4 bankroll commands to the bin. Now generates
5 commands + 8 types. Verified `BankrollConfigDto` ↔
`src/types/bankroll.ts` match (snake_case fields preserved).

**Found during Phase 2**:
- `thiserror::Error` enums don't derive `specta::Type` — added
  hand-rolled `impl Type` for `AppError` mapping to `Primitive::str`.
- i64 fields in `Signal` block codegen (BigInt-forbidden) — use
  local `*CodegenDto` stub in the bin with i32 placeholders.
- Tauri State can't be used in standalone bin — use stub `*_codegen`
  commands that return hardcoded data of the right shape.

### Phase 1: deps + first command (45min) — **v0.76 DONE**

1. Add to `src-tauri/Cargo.toml`:
   ```toml
   [dependencies]
   specta = { version = "=2.0.0-rc.20", features = ["derive"] }
   specta-typescript = "0.0.9"
   tauri-specta = { version = "=2.0.0-rc.20", features = ["derive", "typescript"] }
   ```

2. Pick the simplest command (`dashboard_kpis`). Add `Deserialize`
   to `DashboardKpis` (already has Serialize).

3. Annotate:
   ```rust
   #[tauri::command]
   #[tauri_specta::specta]
   pub async fn dashboard_kpis(state: State<'_, AppState>) -> AppResult<DashboardKpis> { ... }
   ```

4. Create `src-tauri/src/bin/gen_ts_types.rs`:
   ```rust
   use specta::collect_types;
   use tauri_specta::{collect_commands, ts::ExportConfig};
   use polyrocket_lib::*;

   fn main() {
     let mut types = collect_types![DashboardKpis, /* ... */];
     let commands = collect_commands![dashboard_kpis /*, ... */];
     tauri_specta::ts::export_with_dir(
       &commands,
       Default::default(),
       &out_dir("src/types/generated"),
     ).unwrap();
   }
   ```

5. Add `[[bin]]` entry in `Cargo.toml` for `gen_ts_types`.

6. Run `cargo run --bin gen_ts_types`. Generates
   `src/types/generated/index.ts`.

### Phase 2: verify generated TS matches hand-written (30min)

Compare generated `DashboardKpis` interface with the
hand-written one in `src/types/shared.ts`. They should be
identical. If not, file an issue and figure out why.

For the polyrocket project, the generated TS uses camelCase
(by default for tauri-specta) while the hand-written uses
snake_case (Rust serde default). We'd need to either:
- Switch all DTOs to `#[serde(rename_all = "camelCase")]` (large change)
- Configure tauri-specta to emit snake_case (custom export config)

The snake_case path is simpler — fewer source changes.

### Phase 3: migrate a batch of read-only commands (1h) — **v0.84 DONE**

v0.84 added 14 read-only commands to the codegen bin (3 sub-versions
a/b/c + drift detector in d):

**v0.84a (5 commands, no BigInt)**:
- `is_seeded` (bool)
- `sidecar_status` (real `SidecarStatus` DTO)
- `secrets_status` (real `SecretsStatus` DTO with nested `SecretStatus`)
- `notification_permission_state` (String)
- `get_telemetry_enabled` (bool)

**v0.84b (5 commands, mix real + stub DTOs)**:
- `get_auto_promote_config` (real `AutoPromoteConfigDto`)
- `get_storage_info` (`StorageInfoCodegen` stub — real has `u64 free_bytes`)
- `get_mirror_paper_mode` (bool)
- `get_audit_retention` (`AuditRetentionViewCodegen` stub — 3× i64)
- `get_active_model` (`ActiveModelCodegen` stub — Option<i64> + omitted `best_params: serde_json::Value`)

**v0.84c (4 commands, all stub DTOs)**:
- `list_active_signals` (`SignalListItemCodegen`)
- `list_mirrors` (`MirrorRowCodegen`)
- `list_wallets` (`WalletDtoCodegen`)
- `mirror_queue_stats` (`MirrorQueueStatsCodegen`)

**v0.84d**: drift detector `scripts/check-codegen-drift.mjs` catches:
- Type added/removed
- Field added/removed/renamed
- Field type changed
- Command added/removed
- Command signature changed

**Total codegen exports (v0.84 final)**: 19 commands + 19 types
(was 5 + 8 at v0.81).

**Known limitations (planned v0.84+)**:
- i64/u64 fields use i32/f64 in stub DTOs (precision loss; planned
  `Number<i64>` wrapper via specta-typescript's `serde` feature)
- `serde_json::Value` fields are omitted from stubs (no `specta::Type`
  impl; L1 keeps hand-written `Record<string, unknown>`)
- `u64 free_bytes` in `StorageInfo` uses `Option<f64>` (lossy above 2^53)

**Drift demo (v0.84d manual)**:
1. Renamed `SecretStatus.kind` → `SecretStatus.kind_label` in Rust
2. Ran `pnpm check:codegen-drift` → "2 drift(s) detected"
3. Reverted → "no drift (zero diff)"

**Why this matters**:
- Hand-written types catch "function added/removed" but miss
  "field added/removed" (manual DTO sync misses constantly)
- Codegen + drift detector catches 4 of 5 drift types in the table
  below; the 5th (BigInt precision) is a v0.84+ roadmap item
- 19 of 112 IPCs are now drift-protected (was 5 at v0.81)

### Phase 4: migrate commands with input DTOs (1h)

The harder ones (e.g. `place_signed_order` takes a
`PlaceSignedArgs`). Need to:
- Ensure `PlaceSignedArgs` derives both `Serialize` AND
  `Deserialize`
- Annotate with `#[tauri_specta::specta]`
- Test that the codegen produces a matching TS interface

### Phase 5: make it part of the build (30min)

- `package.json`: add `"gen:ts": "cd src-tauri && cargo run --bin gen_ts_types"`
- `scripts/check-doc-sync.mjs`: check that `src/types/generated/` is committed
- CI: run `pnpm gen:ts` + check no diff in generated/

## What's NOT in v0.68b

This v0.68b commit **does NOT add the deps or any `#[tauri_specta]` annotations**. It's a planning doc + a stub for the first-command proof-of-concept that I'll do in v0.69. The reason for the deferral:

- Adding `tauri-specta` 2.0.0-rc.20 to Cargo.toml requires a
  non-trivial Cargo.lock update. The rc version is stable
  but the lock file changes are noisy.
- The first migration (Phase 1) touches `DashboardKpis`
  which is used by the `/dashboard` page (covered by
  tests). The risk of regression is non-zero.
- The codegen output uses camelCase by default but the
  project uses snake_case. Need to make a project-wide
  decision (rename DTOs vs configure codegen) before
  mass migration.

## When to actually do it

Once any of the bottom 5 drift types in the table above
catches us (a field rename that breaks the UI silently),
that's the trigger. v0.69+ candidate.
