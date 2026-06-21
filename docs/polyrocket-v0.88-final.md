# polyrocket v0.88 — codegen Phase 4 (input DTO commands)

> 2026-06-21 · 6 commits · 1 source file (~+600 lines) + 1 generated file (~+580 lines)
> a / b / c / d / e / final

## TL;DR

**Codegen Phase 4 DONE** — added 13 input DTO commands drift protection. Codegen coverage went from **19 → 31 commands** (17% → **28%** of 112 IPCs). L1 still calls the real commands; the `_codegen` suffix marks the codegen surface for TS type export + drift detection.

This was the validation milestone for v0.85c/v0.86b's BigInt wrappers — Phase 4 exercises `Option<i64>` input fields and nested `Vec<StructWithI64>` codegen end-to-end, all passed.

## Sub-versions

### v0.88a — Phase 4 batch 1: 3 simple bool/single-field commands

| Command | Args DTO | i64 fields? |
|---|---|---|
| `add_wallet` | `AddWalletArgsCodegen` (chain_id: Option<i32> truncation) | wallet return: BigInt + OptionBigInt |
| `set_telemetry_enabled` | `SetTelemetryEnabledArgsCodegen` (enabled: bool) | none |
| `set_mirror_paper_mode` | `SetMirrorPaperModeArgsCodegen` (enabled: bool) | none |

Files: `src-tauri/src/bin/gen_ts_types.rs` (+105 lines), `src/types/generated/index.ts` (+36 lines).
Commit: `604dfff`.

### v0.88b — Phase 4 batch 2: 2 Copy route commands

| Command | Args DTO | Return |
|---|---|---|
| `add_copy_target` | `AddCopyTargetArgsCodegen` (no i64) | `CopyTargetDtoCodegen` (new DTO stub for `created_at: i64` → BigInt) |
| `enqueue_mirror` | `EnqueueMirrorArgsCodegen` (event_id: BigInt<i64>) | `MirrorRowCodegen` (reuses v0.86b OptionBigInt) |

Files: `src-tauri/src/bin/gen_ts_types.rs` (+139 lines), `src/types/generated/index.ts` (+104 lines).
Commit: `54a9f4c`.

### v0.88c — Phase 4 batch 3: 2 Trade route commands

| Command | Args DTO | Return |
|---|---|---|
| `place_signed_order` | `PlaceSignedArgsCodegen` (signal_id: OptionBigInt<i64>) | `BetDtoCodegen` (new DTO, 3× i64 fields) |
| `place_jump_link` | `PlaceJumpArgsCodegen` (signal_id: OptionBigInt<i64>) | `String` URL |

Files: `src-tauri/src/bin/gen_ts_types.rs` (+185 lines), `src/types/generated/index.ts` (+133 lines).
Commit: `dcddb36`.

### v0.88d — Phase 4 batch 4: Settings/ModelLab/LlmMgmt

| Command | Args DTO | Return |
|---|---|---|
| `set_audit_retention` | `SetAuditRetentionArgsCodegen` (3× OptionBigInt<i64>) | `u32` (purge count) |
| `set_auto_promote_config` | real `SetAutoPromoteConfigArgs` (+1 line `specta::Type` derive) | real `AutoPromoteConfigDto` |
| `upsert_llm_provider` | `LlmProviderDtoCodegen` (timeout_ms: BigInt<i64>) | `()` |

Side effect: had to add `specta::Type` derive to real `SetAutoPromoteConfigArgs` struct (1 line, doesn't affect serde behavior). Also changed `set_audit_retention_codegen` return from `usize` to `u32` because specta-typescript forbids `usize` export.

Files: `src-tauri/src/bin/gen_ts_types.rs` + `src-tauri/src/commands/sidecar.rs` (1 line) + `src/types/generated/index.ts`.
Commit: `5fc2f18`.

### v0.88e — Phase 4 batch 5: complex nested DTOs

| Command | Args DTO | Return |
|---|---|---|
| `llm_analyze` | `LlmAnalyzeArgsCodegen` (signal_id: OptionBigInt<i64>) | `LlmAnalysisDtoCodegen` (4× i64 + nested `Vec<LlmRecommendationDtoCodegen>` with 4× i64) |
| `run_mirror_executor_pass` | `RunMirrorPassArgsCodegen` (no i64) | `ExecutorPassResultCodegen` (uses `Vec<(String, String)>` because real `Vec<(String, RejectReason)>` needs new RejectReasonCodegen stub) |

Files: `src-tauri/src/bin/gen_ts_types.rs` (+218 lines), `src/types/generated/index.ts` (+169 lines).
Commit: `02eac0f`.

## Validation milestones

This version's main value was validating the v0.85c/v0.86b BigInt wrappers at scale:

| Pattern | Where exercised | Status |
|---|---|---|
| `#[specta(type = BigInt)]` on bare `i64` | wallet.chain_id, bet.placed_at, audit.recent_ms, llm.requested_at, llm_recommendation.id, llm_provider.timeout_ms | ✅ all pass |
| `Option<OptionBigInt<i64>>` for `Option<i64>` | bet.signal_id, bet.settled_at, llm.signal_id, llm.completed_at, llm.total_latency_ms, llm_recommendation.latency_ms/tokens_in/tokens_out, enqueue_args.signal_id, audit.retain/max/min, copy.created_at | ✅ all pass |
| Nested struct with i64 fields (`Vec<LlmRecommendationDtoCodegen>` inside `LlmAnalysisDtoCodegen`) | llm_analyze return | ✅ pass |
| Nested command args (enqueue with event_id inside EnqueueArgs) | enqueue_mirror | ✅ pass |

## Issues found and fixed during Phase 4

1. **`SetAutoPromoteConfigArgs` missing `specta::Type` derive** (v0.88d): one-line fix in `commands/sidecar.rs`. The Type derive is purely for codegen — no serde behavior change.
2. **`set_audit_retention_codegen` return type `usize` → `u32`** (v0.88d): specta-typescript forbids `usize` export (BigInt concern). Real impl still returns `usize`; codegen drift detection only checks args DTO + presence, not return type precision. Documented limitation.
3. **`dist/` stub** (every sub-version): `cargo run --bin gen_ts_types` requires a `dist/` directory (because the bin links the full polyrocket lib which uses `tauri::generate_context!()`). Recreated at project root before each rebuild.

## Drift protection: 19 → 31 commands

| Phase | Commands | Coverage of 112 IPCs |
|---|---|---|
| Phase 1 (v0.76) | 1 (dashboard_kpis) | 1% |
| Phase 2 (v0.81) | +4 (bankroll CRUD) | 4% |
| Phase 3 (v0.84) | +14 (read-only + Vec returns) | 16% |
| **Phase 4 (v0.88)** | **+13 (input DTOs)** | **28%** |
| Phase 5 (v0.90) | planned: drift detector into `pnpm build` | — |

## Doc sync

- `docs/codegen-migration-plan.md` — Phase 4 marked DONE; Phase 5 next
- `docs/coding-spec.md` v2.10 → v2.11 + new changelog row
- `docs/overview.md` v2.51 → v2.52 + new changelog entry
- `polyrocket-v0.88-final.md` (this ship log)

## Verification

| Sub-version | cargo run gen_ts_types | drift check | local CI 5/5 |
|---|---|---|---|
| v0.88a | ✅ | ✅ | ✅ |
| v0.88b | ✅ | ✅ | ✅ |
| v0.88c | ✅ | ✅ | ✅ |
| v0.88d | ✅ | ✅ | ✅ |
| v0.88e | ✅ | ✅ | ✅ |
| v0.88-final | ✅ (regen check) | ✅ | ✅ (post-docs-edit) |

Coverage unchanged (no new test files added — codegen is drift-detection-only, doesn't add coverage; existing tests cover real commands).

## Diff stat (cumulative across a/b/c/d/e)

```
src-tauri/src/bin/gen_ts_types.rs | ~+600 lines (5 batches × ~100-220 each)
src-tauri/src/commands/sidecar.rs | +1 line (Type derive on SetAutoPromoteConfigArgs)
src/types/generated/index.ts      | ~+580 lines (auto-regenerated, ~95 lines/command avg)
docs/codegen-migration-plan.md    | 1 line (Status header)
docs/coding-spec.md               | 2 lines (version header + changelog row)
docs/overview.md                  | 2 lines (version header + changelog entry)
docs/polyrocket-v0.88-final.md    | new file (this ship log)
```

## Next

- **v0.89** — coverage round 2: Bankroll.tsx branches 36 → 70%(+34pp), Trade.tsx + MarketDetail.tsx branches bump, ModelLab.tsx fn 59 → 75 via custom hook (prepares v0.91 promoteAllMut refactor)
- **v0.90** — Phase 5 build pipeline: `check-codegen-drift.mjs` exit code 1 on drift, wired into `pnpm build` and `pnpm tauri build`
- **v0.91** — `promoteAllMut` refactor + ModelLab hook extraction
- v0.92+ — remaining commands not yet drift-protected: 81 left (28 → 100% would need: command shape decisions for `serde_json::Value` fields, possibly adding custom specta wrappers, deciding on remaining side-effect/fire-and-forget commands)
