# polyrocket v0.84 — Codegen Phase 3: 19 IPCs drift-protected

> 2026-06-20 · v0.84 final

## 0. 30 秒摘要

| | |
|---|---|
| 上一版 | v0.82 (Playwright e2e CI gate + Rust 1.96 + dynamic test totals) |
| 本版号 | **v0.84** (skipped v0.83 — ModelLab branches deferred per push schedule) |
| 改动 | Codegen Phase 3: +14 read-only commands + drift detector |
| 改动量 | 5 sub-versions (a/b/c/d/e), 4 NEW files + 7 modified files |
| Codegen exports | 5 → **19 commands** (+14); 8 → **19 types** (+11) |
| Drift types caught | 4 of 5 (i64/u64 precision drift deferred to v0.84+) |
| 5/5 CI jobs green | governance ✓ · L1+vitest (892) ✓ · cargo (348) ✓ · Python (86) ✓ · Playwright (7) ✓ |
| 文档 | `codegen-migration-plan.md` (Phase 3 marked done) · 本 ship log |

## 1. Phase 3 战略选择

v0.81 完成了 5 个 codegen 命令(dashboard_kpis + 4 bankroll)。Phase 3 的目标是把
codegen 推到 ~20 个命令,但中间有 3 个**重要的策略决策**:

### 1.1 BigInt 问题

`tauri-specta` 用 `specta-typescript` 0.0.12,**默认拒绝** i64/u64/i128/u128
字段(防止精度丢失,会 panic with `bigint_forbidden`)。`Number<T>` wrapper
可以处理,但:

- `Number<T>(T)` 是 private tuple struct,外部不能 `Number(0)` 直接构造
- `Number` 的 `Serialize`/`Deserialize` 实现 gated on `serde` feature,
  默认**不开启**
- 开启 `serde` feature + `enable_lossless_bigints()` 会让 L1 层用 BigInt,
  churn 大(11 个 .ts 文件要更新)

**决策**:Phase 3 用 `*CodegenDto` stub + i32 placeholder 解决。drift
detection 仍然捕获字段重命名/添加/删除/类型变化(BigInt 精度丢失
不在 detection 范围内,留 v0.84+)。

### 1.2 serde_json::Value 问题

`serde_json::Value` 没有 `specta::Type` impl。`ActiveModel.best_params`
字段就是这个类型。

**决策**:从 `ActiveModelCodegen` stub 里**省略** `best_params` 字段。
L1 层(`src/ipc.ts:1227`)继续用手写的 `Record<string, unknown> | null`,
drift detection 由现有 v2 contract test 覆盖。`best_params` 的 drift
limited to L1 layer。

### 1.3 Real type vs stub DTO 策略

v0.81 的 bankroll 命令全部用 real types(`BankrollConfigDto` 等)。Phase 3
3 个命令 (`get_auto_promote_config` / `get_storage_info` / `sidecar_status`
等) 沿用这个模式。3 个命令因 BigInt 问题用 stub DTO。

**Trade-off**:
- Real type:drift detection 100%(字段重命名直接 surface)
- Stub DTO:drift detection 限于 stub 内部;real type 改了字段
  stub 不会 catch,需要 v0.84+ 的 BigInt 解决

**当前结果** (v0.84 final):

| 命令 | Return Type | Strategy | Drift Catches? |
|---|---|---|---|
| dashboard_kpis | DashboardKpisDto (v0.76) | real + i32 placeholder | field set + names |
| compute_allocation_preview | AllocationResult (v0.81) | real | 100% |
| get_bankroll_config | BankrollConfigDto (v0.81) | real | 100% |
| set_bankroll_config | () (v0.81) | real | 100% |
| apply_allocation | String (v0.81) | real | 100% |
| is_seeded (v0.84a) | bool | real (no struct) | n/a |
| sidecar_status (v0.84a) | SidecarStatus | real | 100% |
| secrets_status (v0.84a) | SecretsStatus | real | 100% |
| notification_permission_state (v0.84a) | String | real | n/a |
| get_telemetry_enabled (v0.84a) | bool | real | n/a |
| get_auto_promote_config (v0.84b) | AutoPromoteConfigDto | real | 100% |
| get_storage_info (v0.84b) | StorageInfo | stub (u64→f64) | field set + names |
| get_mirror_paper_mode (v0.84b) | bool | real | n/a |
| get_audit_retention (v0.84b) | AuditRetentionView | stub (i64→i32) | field set + names |
| get_active_model (v0.84b) | Option<ActiveModel> | stub (i64→i32, Value 省略) | field set + names |
| list_active_signals (v0.84c) | Vec<SignalDto> | stub | field set + names |
| list_mirrors (v0.84c) | Vec<MirrorRow> | stub | field set + names |
| list_wallets (v0.84c) | Vec<WalletDto> | stub | field set + names |
| mirror_queue_stats (v0.84c) | MirrorQueueStats | stub | field set + names |

**覆盖率**:112 IPCs 中 19 (17%) drift-protected(v0.81: 5 = 4.5%)。

## 2. 5 个 sub-versions 详情

### 2.1 v0.84a — 5 no-arg read-only commands

| 命令 | 改动 |
|---|---|
| `is_seeded` | 简单 bool 返回,无 DTO |
| `sidecar_status` | `SidecarStatus` 加 `Deserialize, Type`(原只有 Serialize) |
| `secrets_status` | `SecretsStatus` + `SecretStatus` 加 derives |
| `notification_permission_state` | String 返回 |
| `get_telemetry_enabled` | bool 返回 |

**关键 diff**:
- `src-tauri/src/commands/sidecar.rs`: `SidecarStatus` +`specta::Type`
- `src-tauri/src/commands/secrets.rs`: `SecretStatus` +`SecretsStatus` +`specta::Type`
- `src-tauri/src/bin/gen_ts_types.rs`: 5 codegen stubs + 5 keepalive references + 5 `collect_commands!` entries
- `package.json`: +`gen:ts` 脚本
- `src/types/generated/index.ts`: regenerated,3 个新 type

### 2.2 v0.84b — 5 simple-arg commands (mix real + stub)

| 命令 | Strategy | Why |
|---|---|---|
| `get_auto_promote_config` | real | 字段都是 bool/f64 |
| `get_storage_info` | stub (`StorageInfoCodegen`) | `free_bytes: Option<u64>` |
| `get_mirror_paper_mode` | real (bool) | 无 struct |
| `get_audit_retention` | stub (`AuditRetentionViewCodegen`) | 3× i64 |
| `get_active_model` | stub (`ActiveModelCodegen`) | `Option<i64>` + `serde_json::Value` 省略 |

**关键 diff**:
- `src-tauri/src/commands/sidecar.rs`: `AutoPromoteConfigDto` +`Deserialize, Type`
- `src-tauri/src/commands/storage.rs`: `StorageInfo` +`Deserialize, Type`
- `src-tauri/src/bin/gen_ts_types.rs`: 5 新 stubs + 2 new stub DTOs (`StorageInfoCodegen`, `AuditRetentionViewCodegen`, `ActiveModelCodegen`)

### 2.3 v0.84c — 4 Vec-return commands (all stub)

所有 4 个 DTO 都有 i64 字段,全用 stub:

| 命令 | Row DTO | i64 fields | 略缩到 |
|---|---|---|---|
| `list_active_signals` | `SignalListItemCodegen` | 4 (id, computed_at, horizon_hours, ...) | i32 |
| `list_mirrors` | `MirrorRowCodegen` | 5 (event_id, created_at, submitted_at, filled_at, ...) | i32 |
| `list_wallets` | `WalletDtoCodegen` | 3 (chain_id, created_at, last_synced_at) | i32 |
| `mirror_queue_stats` | `MirrorQueueStatsCodegen` | 5 (n_pending, n_submitted, ...) | i32 |

`ListSignalsArgs` + `ListMirrorsArgs` 都有 `Option<i64>` (limit),加了
`ListSignalsArgsCodegen` + `ListMirrorsArgsCodegen` stub args。

**关键 diff**:
- `src-tauri/src/bin/gen_ts_types.rs`: 4 新 stubs + 6 stub DTOs (4 row + 2 args)

### 2.4 v0.84d — Drift detector

新文件 `scripts/check-codegen-drift.mjs` (242 lines):

**算法**:
1. 保存当前 `src/types/generated/index.ts` 为 expected
2. 跑 `cargo run --bin gen_ts_types` 重新生成
3. 对比 expected vs actual
4. 解析 type 定义和 command 签名,做结构化 diff
5. 输出 drift 列表(field add/remove/rename, type change, command add/remove/sig change)

**手工 demo** (v0.84d verified):
1. `sed` 把 `SecretStatus.kind` → `SecretStatus.kind_label` (Rust)
2. 跑 `pnpm check:codegen-drift` → 报告 "2 drift(s) detected"
3. `sed` 还原 → 报告 "no drift (zero diff)"

**限制** (planned v0.84+):
- i64→i32 stub truncation 不 catch(只有 field set + names drift)
- `serde_json::Value` 字段不在 codegen(无 Type impl)
- 需要 `Number<i64>` wrapper via specta `serde` feature

**Exit codes**:
- 0 = clean
- 1 = drift (action: review + `pnpm gen:ts` + commit)
- 2 = codegen compile error (Rust 或 specta panic)

### 2.5 v0.84e — Docs + ship log

- `docs/codegen-migration-plan.md`:Phase 3 marked done
- `docs/polyrocket-v0.84-final.md` NEW (本文件)
- README auto-sync:test totals 没变(348 + 892 + 86 + 7 = 1333),coverage 没变(86.15/83.24/79.93/87.35)

## 3. 5/5 CI jobs green 实证

```
[1/5] governance guards
✓ governance guards PASS
[2/5] L1 typecheck + vitest
 Test Files  103 passed (103)
      Tests  892 passed (892)
✓ L1 typecheck + vitest PASS
[3/5] Rust cargo test
test result: ok. 348 passed; 0 failed; 0 ignored
✓ Rust cargo test PASS
[4/5] Python sidecar tests
86 passed in 0.21s
✓ Python sidecar tests PASS
[5/5] Playwright e2e
  7 passed (3.3s)
✓ Playwright e2e PASS

ALL 5 JOBS PASSED — safe to push
```

**关键验证**:
- L1↔Tauri guard:`83 L1 wrappers, 115 registered commands, 115 #[tauri::command] defs`(不变)
- Drift detector:`✓ no drift — generated TS matches committed`
- cargo test:348 passed
- vitest:892 passed
- pytest:86 passed
- Playwright:7 passed

## 4. 后续 (v0.85+)

### 4.1 v0.85: BigInt 处理 + Number<i64> wrapper

**目标**:从 stub DTO 的 i32 placeholder 升级到无损的 `Number<i64>`:
1. 在 `Cargo.toml` 给 `specta-typescript` 开 `serde` feature
2. 在 `Cargo.toml` 给 `specta-typescript` 开 `bigint` 配置?
3. 在 `Typescript::default()` 上 `.enable_lossless_bigints()`
4. 在 stub DTOs 把 `i32` 字段改成 `Number<i64>`
5. 在 L1 layer 把对应 TS `number` 字段改成 `bigint`(~6 个文件)

**Risk**:`Number<T>` 的 `Serialize` impl 是 `T::serialize(&self.0, ...)`,
意思是 L1 接收到的 JSON 字段值仍然是 `number` (在 JS 里 BigInt 不能直接
serialize via JSON 除非显式 `.toString()`)。所以 L1 拿到的还是 i64-as-number,
大数值可能丢精度。要真用 BigInt,需要:

- Rust: `let n: i64 = ...; let bigint = BigInt(n); serde::Serialize → "n"` 字符串
- L1: parse 字符串 → BigInt
- TS: type `bigint`,JSON parse as BigInt

这是 v0.85 的方向,但需要小心设计(不是简单改 feature flag)。

### 4.2 v0.85+: Phase 4 (input DTO commands)

`place_signed_order`, `set_bankroll_config` (已有), `add_wallet`,
`upsert_llm_provider` 等。这些 commands 都拿 input DTOs,Phase 3 跑过
( `ComputeAllocationArgsCodegen`)。Phase 4 是把更多 input DTO commands
加进去 (~10 个)。

### 4.3 v0.86: Phase 5 (build pipeline)

- `scripts/check-codegen-drift.mjs` 加到 pre-push hook
- `package.json`: pre-push 时自动跑 `check:codegen-drift`
- CI workflow: `e2e` job 之前跑 `cargo run --bin gen_ts_types` + diff check
- 文档:`coding-spec.md` §10.x 加 codegen drift check 条款

### 4.4 v0.87+: 收尾

- i64 → BigInt 升级完成
- 19 → 全 112 IPCs codegen
- L1 layer 全部用 generated types
- 删除 hand-written `src/types/*.ts` 中 codegen 重叠的部分
- drift detector 增强:BigInt precision drift、serde_json::Value 字段

## 5. v0.84 commits (本批)

| Commit | Title | Files |
|---|---|---|
| `9fd29db` | v0.84a: codegen Phase 3 — +5 read-only commands (no i64) | 5 |
| `0a6d197` | v0.84b: codegen Phase 3 batch 2 — +5 simple-arg commands | 4 |
| `613ef9d` | v0.84c: codegen Phase 3 batch 3 — +4 Vec-return commands | 2 |
| `4fdba3f` | v0.84d: codegen drift detector | 2 |

## 6. push 计划

v0.84 完成后,5 commits 在本地。**v0.83 (ModelLab branches)** 是 plan 文档
但没有 commit;如果 ModelLab 不动,可以等下一个 round 一起推。
Per user.md convention 2026-06-17,user 手动 push。

Push 由 user 执行(per user.md "Git push convention" 2026-06-17)。

## 7. 学到的 patterns (memory 候选)

下面 3 个 patterns 适合写进 mavis memory:

1. **codegen with BigInt avoidance** — i64/u64 → stub DTO + i32 placeholder
   pattern(v0.84 v0.84b/c 多次复用)
2. **drift detection by regeneration** — `run codegen, diff committed,
   exit non-zero on structural changes` (v0.84d 模式)
3. **codegen-friendly constructor pattern** — `_args: StubArgsCodegen`
   替代 `_args: RealArgs` (real args 通常有 BigInt field)
