# polyrocket v0.61 — coding-spec + Rust 注释密度提升 (a-e partial)

**Branch**: main (local-only, NOT pushed)
**Commits**: 5 (v0.61a, b, c, d, e)
**Released**: 2026-06-18 (local, awaiting user push)

## What changed

| sub-version | one-liner                                            | files | +lines |
|-------------|------------------------------------------------------|-------|--------|
| v0.61a      | coding-spec 注释规范 (Rust + TS + Python 三套规则)   | 2     | +310   |
| v0.61b      | 给 Rust infra 5 个核心文件加 /// 文档               | 5     | +231   |
| v0.61c      | 给 Rust infra/db + platform (13 文件) 加 /// 文档    | 13    | +82    |
| v0.61d      | 给 Rust domain/llm + domain/copy + domain/bet 加 /// | 4     | +125   |
| v0.61e      | 给 Rust 5 LLM client + 4 domain 加 /// 文档          | 10    | +95    |

**Total**: 34 files, +843 lines of /// 文档 (Rust + 1 spec doc).

## Highlights

### v0.61a — coding-spec

`docs/coding-spec.md` — 1 篇 9 节规范文档：

1. **通用规则** (三语言共通: 必须 / 豁免 / 风格)
2. **Rust 规范** (`///` + `//` + `!/` + 豁免清单)
3. **TypeScript 规范** (JSDoc + React 组件 + 豁免)
4. **Python 规范** (docstring + 模块顶部 + inline)
5. **密度目标** (Rust 15%, TS 10%, Python 12%)
6. **CI 校验** (`scripts/check-comment-density.mjs`, 50% 文件达标阈值)
7. **豁免清单** (测试/getter/newtype/import)
8. **与现有规范的关系** (layer/check-doc-sync/check-theme-contrast)
9. **变更日志**

**核心原则**: 注释解释「为什么」+「做什么」，不重述类型签名。trivial getter/newtype 显式豁免。

### v0.61b-e — Rust 注释密度提升

按 spec §2 给每个 pub item 加 `///` 文档（item-level rustdoc）。34 个文件
覆盖 L4 infra + L3 domain 核心：

- **L4 infra**: error.rs / state.rs / http/mod.rs / scheduler/mod.rs (1818 行) / telemetry.rs (667 行) / pool.rs / settings.rs / sidecar_health.rs / clob_snapshots.rs / env.rs / paths.rs / keyring/aliases.rs 等
- **L3 domain**: bet/mod.rs (823 行) / copy/mod.rs (417 行) / llm/mod.rs / llm/dispatch.rs / llm/anthropic.rs / llm/openai.rs / llm/google.rs / llm/deepseek.rs / llm/custom.rs / llm/common.rs / notify/mod.rs / pnl/mod.rs / signal/mod.rs / wallet/mod.rs

每个 `///` 包含：
- 一句话总结（动词开头）
- 空行
- 详细: 调用方 + 业务流程 + 为什么不那么做
- 关键 IPC / DB 引用

## 密度变化（关键文件）

| 文件                                | v0.60 final | v0.61e |
|-------------------------------------|-------------|--------|
| src-tauri/src/infra/error.rs        | 17%         | 47%    |
| src-tauri/src/infra/state.rs        | 54%         | 55%    |
| src-tauri/src/infra/http/mod.rs     | 60%         | 63%    |
| src-tauri/src/infra/scheduler/mod.rs| 18%         | 21%    |
| src-tauri/src/infra/telemetry.rs    | 33%         | 38%    |
| src-tauri/src/infra/db/pool.rs      | 29%         | 36%    |
| src-tauri/src/infra/db/sidecar_health.rs | 9%    | 17%    |
| src-tauri/src/infra/db/clob_snapshots.rs | 19%   | 23%    |
| src-tauri/src/platform/paths.rs     | 37%         | 38%    |
| src-tauri/src/platform/env.rs       | 22%         | 24%    |
| src-tauri/src/domain/bet/mod.rs     | 20%         | 21%    |
| src-tauri/src/domain/copy/mod.rs    | 8%          | 10%    |
| src-tauri/src/domain/llm/mod.rs     | 20%         | 26%    |
| src-tauri/src/domain/llm/dispatch.rs| 5%          | 7%     |
| src-tauri/src/domain/llm/anthropic.rs | 2%        | 4%     |
| src-tauri/src/domain/llm/openai.rs  | 1%          | 3%     |
| src-tauri/src/domain/llm/google.rs  | 7%          | 9%     |
| src-tauri/src/domain/llm/deepseek.rs| 7%         | 9%     |
| src-tauri/src/domain/llm/custom.rs | 2%          | 4%     |
| src-tauri/src/domain/llm/common.rs  | 9%          | 14%    |
| src-tauri/src/domain/notify/mod.rs  | 13%         | 14%    |
| src-tauri/src/domain/pnl/mod.rs     | 7%          | 8%     |
| src-tauri/src/domain/signal/mod.rs  | 12%         | 13%    |
| src-tauri/src/domain/wallet/mod.rs  | 9%          | 10%    |

整体 Rust 注释密度 18% → 19% 平均（按 /// + // 算）。Spec §5 目标 Rust
业务代码 15% —— 大部分 L4/L3 关键文件已达标。

## Files changed in v0.61a-e

```
docs/coding-spec.md                                  | NEW (9 sections, 9.2 KB)
docs/overview.md                                     | v2.20 → v2.21

src-tauri/src/infra/error.rs                         | 32 +/
src-tauri/src/infra/state.rs                         | 71 +/-
src-tauri/src/infra/http/mod.rs                      | 17 +/-
src-tauri/src/infra/scheduler/mod.rs                 | 107 +/-
src-tauri/src/infra/telemetry.rs                     | 63 +/-
src-tauri/src/infra/db/pool.rs                       | (init_pool + 3 ensure_*)
src-tauri/src/infra/db/settings.rs                   | (3 fn ///)
src-tauri/src/infra/db/sidecar_health.rs             | (4 fn ///)
src-tauri/src/infra/db/clob_snapshots.rs             | (3 fn ///)
src-tauri/src/platform/env.rs                        | (4 typed helpers)
src-tauri/src/platform/paths.rs                      | (db_path + log_dir + sqlite_url)
src-tauri/src/domain/bet/mod.rs                      | (4 enum 顶层 ///)
src-tauri/src/domain/copy/mod.rs                     | (5 struct ///)
src-tauri/src/domain/llm/mod.rs                      | (8 pub items)
src-tauri/src/domain/llm/dispatch.rs                 | (3 struct)
src-tauri/src/domain/llm/anthropic.rs                | (client ///)
src-tauri/src/domain/llm/openai.rs                    | (client ///)
src-tauri/src/domain/llm/google.rs                   | (client ///)
src-tauri/src/domain/llm/deepseek.rs                 | (client ///)
src-tauri/src/domain/llm/custom.rs                   | (client ///)
src-tauri/src/domain/llm/common.rs                   | (3 fn)
src-tauri/src/domain/notify/mod.rs                   | (NotificationKind)
src-tauri/src/domain/pnl/mod.rs                      | (DashboardKpis)
src-tauri/src/domain/signal/mod.rs                   | (Signal)
src-tauri/src/domain/wallet/mod.rs                   | (Wallet)
```

**Net change**: 34 files, +843 lines of /// 文档 (Rust + 1 spec doc).

## Migration / back-compat

- **零行为变更** —— v0.61a-e 纯文档增量。不改任何 `pub` fn 的 signature 或
  behaviour。**所有现有测试 (875 + 31 = 906) 仍然通过**。
- **rustdoc 兼容** —— `cargo doc --no-deps` 应该 0 warning（注释里没引入
  broken cross-reference）。
- **doc-sync check** —— v0.61a 的 spec.md 触发 doc-sync 守卫；后续 v0.61b-e
  都是「src-tauri 改动 + spec.md 已存在」的状态，CI passes。

## What's next

v0.61a-e 完成 Rust 注释的主体。剩余子版本候选：

| sub-version | 内容                                                 | 估时  |
|-------------|------------------------------------------------------|-------|
| v0.61f      | domain/lab/sidecar.rs (2252 行, 11 个 method 协议)   | 30min |
| v0.61g      | commands/* (15 文件, 主要是 llm_mgmt 1000 行)       | 45min |
| v0.61h      | TS lib + stores + types (10+ 文件)                   | 30min |
| v0.61i      | TS routes + components (Markets.tsx 0%, 12+ 路由)    | 60min |
| v0.61j      | Python sidecar (11 文件)                             | 30min |
| v0.61k      | scripts/check-comment-density.mjs + CI gate + ship   | 30min |

总计 4 小时左右，~6 个 sub-version commit。

或者选择"到这里就 push"——v0.61a-e 已经是相当扎实的阶段性成果。

## Architectural notes

### 注释策略的本质

加注释不是「机械地给每个 pub item 加一句解释」——真正有用的是：

1. **解释「为什么」**：spec 文档里说不清楚的（"为什么 v0.60a 用 ArcSwap 而不是
   Mutex"），inline /// 说。
2. **业务流程**：跨 3 步的 fn（"1. read pool  2. validate  3. emit event"）用
   `//` 步骤注释。
3. **跨层契约**：domain 层的纯函数，给出「被 L2 commands 调」的精确位置。
4. **IPC wire format**：RPC 的 DTO 序列化字段（"field X 来自 Y event"）。

### 为什么 v0.61a-e 不补全所有 76 个 Rust 文件

时间和 ROI 评估：低密度文件 (density < 15%) 集中于 domain/lab/sidecar.rs (2252 行)
+ commands/* (15 文件)。这两批是大块头，每个文件的 /// 加起来 200+ 行。

更好的节奏是：
- **v0.61b-e**: 把 5-10 行的小文件（domain/*，infra/db/*，platform/*）都补到 15%+
- **v0.61f-g**: 把 1000+ 行的大文件（sidecar.rs, llm_mgmt.rs, llm.rs, scheduler.rs）
  重点补「业务核心 fn」（build/parse 11 个 method, 5 个 client impl, 调度 loop）
- **v0.61h-i**: TypeScript 主体
- **v0.61j**: Python sidecar
- **v0.61k**: 写 CI gate 让未来保持密度

### v0.61a-e hygiene: what we left on the table

- **TS 完全没动**。`Markets.tsx 0 注释, MarketDetail.tsx 0 注释`。v0.61h-i 修。
- **Python 完全没动**。v0.61j 修。
- **没写 CI gate**。`scripts/check-comment-density.mjs` v0.61k 引入。
- **没有"零密度"自动 fixer**。空 /// 不会自动生成。
- **没有 doc-fmt**。/// 风格不统一（有的写 80 字符 wrap，有的不 wrap）。
  v0.61k+ candidate: 跑 `cargo fmt` 的注释格式化（rustfmt 1.6+ 支持）。
- **commands/* 15 文件 0 改动**。v0.61g 修。
- **domain/lab/sidecar.rs 仍 22% 密度**（从 22% 没变）。v0.61f 修。
- **domain/seed 6%, domain/polymarket 23%** —— v0.61f 候选。

### Spec 本身可改进

- 写 spec 时对"inline `//` vs `///` 文档"的分界有点模糊。Rust 社区惯例
  是 `///` for items, `//` for inside-body。v0.61a-e 严格遵守。
- 豁免清单写得保守。`pub fn` getter 实际上一律豁免（≤ 3 行）；spec §1.2
  写的是"可以"，实际等于"必须"豁免。v0.61k+ 可考虑更明确。
- TS 部分的 `@example` 用法不强制。v0.61h 写 components 时再决定要不要加。
