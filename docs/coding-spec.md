# polyrocket — Coding Spec

> 代码注释 / 文档化规范。**所有新增代码必须遵循此规范；存量代码按 v0.61 计划分轮翻新。**

**版本**：v2.1 · 2026-06-19 (v0.76 final — codegen Phase 1 跑通, dashboard_kpis 标注 + bin + 生成的 TS)
**配套**：[`overview.md`](./overview.md)（5 层架构） · [`polyrocket-modules.md`](./polyrocket-modules.md)（17 模块业务） · [`polyrocket-flows.md`](./polyrocket-flows.md)（20 交互流程） · [`polyrocket-v0.69-final.md`](./polyrocket-v0.69-final.md)（CI 修复记录）

---

## 0. 目标

| 维度 | 目标 |
|---|---|
| **自解释** | 一个新工程师（或 AI 助手）能基于「类型 + 文档注释 + 关键 inline」理解代码意图，不必追源码 |
| **业务连续性** | 关键业务流程（侧车调用 / IPC 路由 / 调度循环 / L1 状态机）有 inline 解释，离职也能交 |
| **可校验** | 注释密度能用脚本量化（v0.61h 引入 `check-comment-density.mjs`） |
| **务实主义** | 不为 `pub struct UserId(pub u32);` 这种 newtype 写 10 行废话 |

**核心原则**：**注释解释「为什么」和「做什么」，而不是重述类型签名。** 注释是给「三个月后的自己」看的，不是给编译器看的。

---

## 1. 通用规则（三语言共通）

### 1.1 必须有注释

- **公开类型**（Rust `pub struct/enum/trait`，TS `export interface/type/class`，Python top-level `class`）
- **公开函数 / 方法**（Rust `pub fn`，TS `export function`，Python top-level `def`）
- **关键业务流程**：跨 3 步以上的函数 / 含 invariant 的函数 / 跨层调用的 glue 函数

### 1.2 可以不注释

- 简单 getter / setter（≤ 3 行的 `pub fn name(&self) -> &str { &self.name }`）
- newtype（`struct UserId(u32);`）—— 名字已经说清
- 单元测试的函数（测试名 + 断言已经自解释）
- 纯 import 行 / `use` / `from ... import` / `export type` re-export
- 简单常量（`const MAX_RETRIES = 3;` —— 命名足够）

### 1.3 注释风格

- **解释「为什么」**：设计意图、不变量、跨层契约（"该函数被 X 串行调用以避免 race"）
- **解释「做什么」**：当函数名不能完整描述行为时（"返回最近一次成功的 partial fill，**不**包括 timeout"）
- **不要重述类型**：`/// name: 用户名` 配合 `pub name: String` 是废话
- **不要写 `// 这是一个函数`**、`// 构造函数`、 `// TODO: 待优化`（如果真的 TODO 用 `// TODO(2026-Q3): 原因`）
- **不要写无信息量的 `// update X`** 配合 `x = new_value`

---

## 2. Rust 规范

### 2.1 文档注释 `///`

```rust
/// `ExplainModel` —— 调侧车计算单样本的特征贡献。
///
/// **调用方**：`commands/sidecar.rs::explain_model`（被 L1 `ExplainabilityCard` 通过 IPC 调用）。
/// **侧车协议**：method=`explain_model`，params=`{model_version, sample}`，返回 `ExplainResult`。
/// **计算模型**：3-feature 线性模型，contribution_i = w_i * x_i * sigmoid(z) * (1 - sigmoid(z))。
///
/// **为什么不直接调 LLM**：特征贡献需要可解释性（per-feature 数字），LLM 输出是自然语言。
pub async fn explain_model(
    state: State<'_, AppState>,
    sample: ExplainSample,
) -> Result<ExplainResult, String> { ... }
```

每个 `pub struct`、`pub enum`、`pub trait`、`pub fn` 都需要 `///`：

- 第一行：一句话总结（动词开头）
- 空行
- 详细说明：调用方、协议、不变量、为什么这么做

### 2.2 模块顶部 `//!`

```rust
//! `domain/pnl` —— 纸面成交的 PnL 计算。
//!
//! **职责**：从 `bets` + `paper_fills` 推导每次 paper bet 的 mark-to-market PnL。
//! **依赖**：仅 `infra/db` + `domain/price`（读价格快照）。
//! **不依赖**：L1 / L2 / 侧车 / 任何全局状态。
//!
//! 关键不变量：PnL 只反映已 settle 的 paper fill，live 但未成交的 bet 显示 0。
```

每个模块文件顶部 `//!` 1-3 行说明模块职责、依赖、不变量。

### 2.3 行内注释 `//`

- **多步骤流程**：`// 1. read model from db  2. validate  3. dispatch to sidecar`
- **Invariant 违反守卫**：`// INVARIANT: caller must hold `pool.write()` —— 读路径可读并发，写路径必须串行`
- **为什么不用更显然的写法**：`// Why not `Mutex<Client>`? Holding a Mutex across await blocks other clients; ArcSwap is lock-free.`
- **TODO**：`// TODO(2026-Q3): 升级到 TreeSHAP —— 当前 KernelSHAP 在 M>10 时 O(2^M) 爆炸`

### 2.4 测试不需要 doc

```rust
#[test]
fn explain_model_efficiency_axiom_holds_at_extremes() {
    // 测试名已经说清测什么；assertion 就是 spec
    ...
}
```

### 2.5 豁免示例（不写 doc）

```rust
// OK —— newtype 命名自解释
pub struct UserId(pub u32);

// OK —— simple getter
pub fn name(&self) -> &str { &self.name }

// OK —— private helper（如果模块顶部 `//!` 已说明）
fn parse_xyz(s: &str) -> Option<xyz::X> { ... }
```

---

## 3. TypeScript / React 规范

### 3.1 JSDoc `/** ... */`

```typescript
/**
 * `explainModel` —— 通过 IPC 调 Rust 侧车，计算单样本的特征贡献。
 *
 * @param sample - 待解释的样本（3 个特征 + 1 个 model_version）
 * @returns `{ result, contribs[], method }`；contribs 数组长度 = 特征数
 * @throws 侧车未启动（"no_sidecar_running"）/ 模型版本不存在（"unknown_model_version"）
 *
 * @example
 *   const r = await explainModel({ features: [0.1, 0.5, 0.9], model_version: 1 });
 *   console.log(r.contribs); // [0.02, 0.15, 0.41]
 */
export async function explainModel(sample: ExplainSample): Promise<ExplainResult> { ... }
```

每个 `export function`、`export class`、`export interface`、`export type` 都需要 JSDoc：

- 第一行：一句话总结
- `@param` / `@returns` / `@throws` —— 标类型不够说的部分
- `@example` —— 仅当用法非显然时
- React component 顶部说明 props 的**业务含义**，不是类型重述

### 3.2 React Component

```typescript
/**
 * `ExplainabilityCard` —— 解释模型对单个样本的预测。
 *
 * 受控：
 *   - `sample` —— 当前要解释的样本（来自 Dashboard 的"explain last bet"）
 *   - `onChangeSample` —— 用户切换样本时回调
 *
 * 状态机：
 *   - `idle` —— 未触发；用户点 "Run" 才请求
 *   - `loading` —— IPC 中
 *   - `result` —— 展示条形图 + 方法标签（derivative | kernel_shap）
 *   - `error` —— 侧车未启动 / 样本非法 / 超时
 */
export function ExplainabilityCard(props: ExplainProps) { ... }
```

### 3.3 关键 inline

- **状态机**：i18n / store / 路由状态机（"view = 'idle' | 'loading' | 'result' | 'error'"）
- **IPC wrapper**：解释返回的错误如何映射（"reject with mapped error: `no_sidecar_running` → 用户可重试"）
- **store action**：跨 store 的 mutation 链（"set trade view to 'submitting'; toast progress; then async IPC; finally clear view"）

### 3.4 测试不需要 JSDoc

```typescript
// OK —— 测试名 + 断言已自解释
it('toggles SHAP / derivative on click', async () => { ... });
```

### 3.5 豁免示例

```typescript
// OK —— pure type alias
export type UserId = number;

// OK —— re-export
export type { Theme } from './themes';

// OK —— tiny helper
const fmt = (n: number) => n.toFixed(2);
```

---

## 4. Python 规范

### 4.1 Docstring `"""..."""`

```python
def explain_model(model_version: int, sample: dict[str, float]) -> dict:
    """计算单个样本的精确分解（contribution_i = w_i * x_i * p * (1-p)）。

    Args:
        model_version: 模型版本号；目前仓库里只有 v1。
        sample: 特征字典 {"feat_0": float, "feat_1": float, "feat_2": float}。

    Returns:
        {"result": float, "contribs": [float, float, float], "method": "exact_decomp"}。
        result 是 sigmoid(z) 后的预测概率；contribs 之和 ≈ result（忽略数值误差）。

    Raises:
        KeyError: sample 缺特征 / model_version 不存在。
    """
    ...
```

每个 top-level `class` 和 `def` 都需要 docstring：

- 一句话总结
- `Args` / `Returns` / `Raises`（Google 风格）
- 算法步骤（如果是多步）放函数体内 inline 注释

### 4.2 模块顶部 docstring

```python
"""侧车 / `explainability` —— 特征贡献 / SHAP 计算。

侧车方法: ``explain_model``、``shap_explain``。
"""

def explain_model(...): ...
```

### 4.3 Inline 注释

- **多步骤算法**：`# 1. 生成所有 2^M 联盟  2. 评估每个联盟  3. 加权 LS 拟合 SHAP 值`
- **关键不变量**：`# INVARIANT: sample 必须包含 3 个特征 (M=3)，少一个 KeyError`
- **为什么不用 lib**：`# Why not shap library? Pulls numpy+scipy+pandas (~30MB) for 100 lines of math.`

### 4.4 测试不需要 docstring

```python
def test_efficiency_axiom_holds_at_extremes():
    # 测试名 + 断言自解释
    ...
```

---

## 5. 密度目标

| 语言 | 模块类别 | 目标注释行占比 | 当前（v0.71 final） |
|---|---|---|---|
| Rust | `commands` / `domain` / `infra` | **≥ 15%** | avg 27.5% (57/67 pass, 85.1%) |
| Rust | `platform` | ≥ 10% | 100% pass (5/5) |
| TypeScript | `routes` / `components` / `lib` / `stores` | **≥ 15%** | avg 21.9% (45/78 pass, 57.7%) |
| TypeScript | `types` | ≥ 5% | 100% pass (7/7) |
| Python | `polyrocket_sidecar/` | **≥ 12%** | 100% pass (7/7) |

**v0.68c round 1**:rust-commands-domain-infra 10→15%, ts-routes 5→10%。
**v0.70 round 2**:ts-routes-components-lib 10→15%(45/78 pass, avg 21.9%)。
**v0.71 round 3 — deferred**:rust-commands-domain-infra 15→20% 候选未执行,理由:
- 当前 57/67 在 15% 目标下已 PASS(85.1%),提升到 20% 会让 ~17 个文件 fail,
  其中 `seed/mod.rs`(47/641=7.3%)、`sidecar_health/mod.rs`(9/114=7.9%)为
  机器生成/内部 SQL dump 类文件,加 /// 收益小。
- ts-routes 15→20% 更激进(33 个文件 fail),10-15h 工作量,优先级低于
  coverage ratchet。
- 当前 5/5 类别 PASS 是 good enough,不强推 round 3,等 coverage 进入
  plateau(v0.74+)后再启动前置 /// doc 工作。

**统计口径**：`///` + `//!` + `//` + `/** */` + `""" """` + `#` 之和 / 总行数。空行不计入分子。

**目标节奏**：v0.61 完成 Rust；v0.62 完成 TS；v0.63 完成 Python。

---

## 6. CI 校验

`scripts/check-comment-density.mjs`（v0.61h 引入）：

- 遍历 `src/**/*.{ts,tsx}` / `src-tauri/src/**/*.rs` / `sidecar/polyrocket_sidecar/*.py`（排除 `*.test.*` 与 `test_*.py`）
- 计算每个文件的注释行占比
- 输出全仓 + per-file 报告
- 失败阈值：上述 5 类模块中**任何一类 < 50% 文件达标**即 fail

不卡 v0.61+ 的"v0.61b 翻新"PR（避免一次性 6000 行 PR 难以 review），但作为 weekly cron 报告（v0.62+ candidate）。

---

## 7. 豁免清单（明示）

以下**不需要** doc / JSDoc / docstring：

1. 单元测试函数 / 方法（`#[test]` / `it(...)` / `def test_...`）
2. 简单 getter / setter（≤ 3 行，类型签名自解释）
3. newtype（`pub struct X(pub T);` / `type X = T;` / `X = TypeAlias(...)`）
4. 纯 import / re-export 行
5. 简单常量（命名足够）
6. 私有 helper（在 `//!` / `@module` / 模块 docstring 里覆盖即可）
7. 配置 / fixture 文件（如 `vitest.config.ts`、`tailwind.config.js`、`tauri.conf.json`）

---

## 8. 与现有规范的关系

| 规范 | 关系 |
|---|---|
| [`overview.md §1.2` 依赖规则](./overview.md) | 强约束，本 spec 不重复 |
| [`scripts/check-layers.mjs`](../scripts/check-layers.mjs) | 强约束，CI 强制 |
| [`scripts/check-doc-sync.mjs`](../scripts/check-doc-sync.mjs) | 强约束，改 code 必须改 doc；本 spec 是 doc 的一部分 |
| `scripts/check-theme-contrast.mjs` | 强约束，独立 |
| `scripts/check-comment-density.mjs`（v0.61h 引入） | 本 spec §6 引入 |

---

## 9. 变更日志

| 版本 | 日期 | 变更 |
|---|---|---|
| v1.8 | 2026-06-19 | v0.74: 新增 §12 Visual Acceptance Gate + `scripts/check-class-coverage.mjs` + 修 nav-item 缺失 CSS |
| v1.7 | 2026-06-19 | v0.73 final: coverage 81.9→82.98% stmts + branches 80.25→81.31% ⭐,threshold 81/78/73/82 → 82/81/76/84 |
| v1.6 | 2026-06-19 | v0.73a: CI gate HARDENED — 移除 `POLYROCKET_PRE_PUSH_SKIP` env + `--quick`,新增 `.git/CI_VERIFIED` state file,§11 新增完整 policy |
| v1.5 | 2026-06-19 | v0.72 final: branches 79.2→80.3%(首次 > 80%),threshold 维持 81/78/73/82 |
| v1.4 | 2026-06-19 | §5 density round 3 deferred 决策(不 bump,等 v0.74+ plateau) |
| v1.3 | 2026-06-19 | §5 density round 2: ts-routes-components-lib target 10→15% |
| v1.2 | 2026-06-19 | 新增 §10.7「Pre-push 本地 CI gate」—— `scripts/run-ci-local.sh` + pre-push hook, push 前必跑 |
| v1.1 | 2026-06-19 | 新增 §10「CI 环境契约」—— pnpm 9 / Rust 1.89 / `--test-threads=1` / README sync 4 条硬约束 |
| v1.0 | 2026-06-18 | 初版:Rust + TS + Python 三套规则 + 密度目标 + CI 校验 |

---

## 10. CI 环境契约（v0.69 强制）

> 本节列出 polyrocket CI 必须满足的硬约束。所有 PR / push 改动若违反任一条，CI 会直接红。**新增依赖、新增测试、新增 build script 前先读本节。**

### 10.1 pnpm 9 + `onlyBuiltDependencies`

| 项 | 约束 |
|---|---|
| **CI 版本** | `pnpm 9.x`（`pnpm/action-setup@v4` with `version: 9`） |
| **本地推荐** | pnpm 9.x（与 CI 1:1）或 pnpm 11.x（更宽松，会容忍一些 pnpm 9 报错） |
| **原生模块** | 必须放在 `package.json#pnpm.onlyBuiltDependencies` 数组里：`better-sqlite3`、`esbuild` 等有 postinstall 的包 |
| **禁止** | 单独的 `pnpm-workspace.yaml` 仅含 `allowBuilds:` —— pnpm 9 会报 `packages field missing or empty` 并直接 fail install |

**为什么**:pnpm 9 要求 workspace yaml 含 `packages:` 字段；只放 `allowBuilds` 等价于"声明了工作区但没工作区",会被拒绝。`onlyBuiltDependencies` 在 `package.json` 里是 pnpm 9.4+ 原生支持的位置,既不需要 workspace yaml,也不需要 11.x 的 `allowBuilds`。

**校验命令**:

```bash
# 应该能跑通(pnpm 9 与 11 都行)
pnpm install --frozen-lockfile
```

如果报 `packages field missing or empty`,检查 `pnpm-workspace.yaml` 是否存在但只有 `allowBuilds:` —— 删除它,把列表迁到 `package.json#pnpm.onlyBuiltDependencies`。

### 10.2 Rust 1.88 toolchain

| 项 | 约束 |
|---|---|
| **CI 版本** | `rustc 1.88`（`dtolnay/rust-toolchain@stable` with `toolchain: 1.88`） |
| **本地推荐** | 1.88+ (1.96 是 user 当前默认) |
| **Cargo.lock 必须提交** | ✅ 已经在 .gitignore 排除项之外 |
| **降级 CI toolchain** | ❌ 不允许 —— Tauri 2.11 的传递依赖(dlopen2_derive 0.4.3, time-0.3.49, darling 0.23)需要 1.85+;darling 需要 1.88+ |
| **本地可选** | 添加 `src-tauri/rust-toolchain.toml` 写 `channel = "1.88"` 让本地与 CI 1:1(v0.70 candidate) |

**为什么**:Cargo 1.77 无法解析 edition2024 的 manifest(Cargo.lock 已 pin 在 0.4.3,没法 downgrade)。升级 toolchain 比 `[patch.crates-io]` 强制老版本更可持续 —— 后者会跟 Tauri 上游打架。

**校验命令**:

```bash
cargo +1.88 check --manifest-path src-tauri/Cargo.toml
cargo +1.88 test --lib --manifest-path src-tauri/Cargo.toml -- --test-threads=1
```

### 10.3 `cargo test --lib -- --test-threads=1`

**所有 Cargo 测试必须以 `--test-threads=1` 运行。** 包括本地(强制)和 CI(`ci.yml` 已配置)。

**为什么**:`commands::sidecar::tests::archive_filters_by_job_ids` 用 `std::env::set_var("POLYROCKET_SIDECAR_MODEL_DIR", ...)` 改了进程级环境变量。并行线程下另一个测试可能 race 同一个 var,导致 flaky failure(约 1/50 次失败)。`--test-threads=1` 强制串行,代价是测试慢约 1.5x(总时长 < 1 分钟,可接受)。

**根治方案**(v0.70+ candidate):重构该测试,model dir 通过函数参数注入,而不是读环境变量。需要改 4-5 个测试 fn。

**禁止**:`--test-threads=2` 或更高 —— race 风险仍存在。

### 10.4 README badges sync 步骤(故意设计为 fail-fast)

`.github/workflows/ci.yml` 的 `README badges in sync (v0.67d)` 步骤会跑:

```bash
node scripts/update-readme-coverage.mjs 2>&1 | grep -q "no changes needed"
```

如果脚本判定 README 与 `coverage/coverage-summary.json` 有 drift,**脚本会原地修改 README.md 并 exit 0**(不会让 CI 步骤本身挂);但 grep 没匹配上 "no changes needed",CI 步骤 fail。这是**故意的 fail-fast drift 检测**。

**正确流程**(维护者本地):

```bash
pnpm test:coverage                                          # 生成 coverage-summary.json
node scripts/update-readme-coverage.mjs                     # 自动同步 README + overview
git add README.md docs/overview.md
git commit -m "v0.XX: ratchet NN% → MM% (auto update)"       # 跟主 commit 一起或单独 commit
```

**禁止**:
- ❌ 跳过 `update-readme-coverage.mjs` 直接 push → CI 必挂 governance guards
- ❌ 在 PR description 里说"README drift 是 known issue" → 不会被接受
- ❌ 把 `grep -q "no changes needed"` 改成 `grep -q "README"` —— 会让 drift 检测失效

**Status 字段额外步骤**(可选):

```bash
node scripts/update-readme-coverage.mjs --version v0.XX    # 同时更新 README Status 行 + overview 版本号
```

### 10.5 其他 CI 硬约束(沿用 v0.57c)

| 项 | 约束 | 失败影响 |
|---|---|---|
| TypeScript typecheck | `pnpm typecheck` 必须 0 error | 阻塞 PR merge |
| vitest 覆盖率门槛 | stmts 73% / branches 69% / funcs 63% / lines 75%(v0.68) | 低于门槛 CI fail |
| L1↔Tauri 守门 | `scripts/check-l1-tauri.mjs` 检查 111 个 IPC 命令一一对应 | 不匹配 fail |
| Layer rules | `scripts/check-layers.mjs` 阻止 L3→L1/L2 等反向依赖 | 提交前 pre-commit 钩子拦截 |
| Theme contrast | `scripts/check-theme-contrast.mjs` 检查 3 主题 WCAG AA(≥4.5:1) | 任意一对 < 4.5:1 fail |
| Comment density | `scripts/check-comment-density.mjs` 检查 5 类模块 ≥50% 文件达标 | 全部 < 50% fail |
| Doc sync | `scripts/check-doc-sync.mjs` 阻止改 code 不改 doc | pre-commit 钩子拦截 |
| Python sidecar tests | `pytest`(单进程,CI 上无 race) | 任意 fail CI fail |

### 10.6 紧急降级(勿轻用)

如果某个 CI 约束临时挡住了紧急 PR,可以:

1. 在 PR description 明确标注 "CI override: <reason>"
2. 在 commit 里 `git revert` 临时关闭守门脚本(不是删除)
3. merge 后立即在新 commit 里 revert revert + 修复

**禁止**:
- ❌ 在 `.github/workflows/ci.yml` 里给守门脚本加 `continue-on-error: true` —— 静默退化
- ❌ 把 `cargo +1.89` 降回 `1.77` —— edition2024 / dlopen2_derive 会回来
- ❌ 把 `version: 9` 改成 `version: 10/11` —— workspace.yaml 逻辑要重写
- ❌ 删 `scripts/check-*.mjs` —— 守门就废了

### 10.7 Pre-push 本地 CI gate(v0.69h,v0.73a HARDENED)

**所有 `git push` 必须在本地通过 `scripts/run-ci-local.sh` 才能 push。** v0.73a
起,所有 bypass 都被移除,只剩 git 原生 `--no-verify`。

```bash
# 一键安装 hook(每个开发者只需运行一次):
./scripts/install-ci-hook.sh

# 之后 git push 会自动:
#   1. 读 .git/CI_VERIFIED state 文件
#   2. 若 state 是 1h 内的新鲜记录 + sha 匹配 HEAD → 直接通过(快速路径)
#   3. 否则跑 scripts/run-ci-local.sh(4 jobs: governance + L1 + Rust + Python)
#   4. 失败则 BLOCK push + 提示具体哪个 job 出错
#   5. 通过则写新 state 文件 + "safe to push"
```

**v0.73a hardening — 移除的 bypass**:

| bypass | 状态 | 原因 |
|---|---|---|
| `POLYROCKET_PRE_PUSH_SKIP=1` env | **REMOVED** | 多次 v0.69-v0.72 push 在 CI 上 fail,根因都是本地 gate 被 skip |
| `run-ci-local.sh --quick`(跳 cargo) | **REMOVED** | "remote pipeline keeps erroring" 的最大单点根因 |
| `git push --no-verify` | **保留**(git 原生,无法从 hook 拦截) | 仅 emergency 用:re-push 已知 CI 绿色 commit 修复远端状态 |

**State file(`.git/CI_VERIFIED`)**:

`run-ci-local.sh` 通过后写:

```
sha=<current_head_sha>
timestamp=<unix_ts>
jobs=all
runner=<hostname>:<pid>
```

Hook 读取判定:state 存在 + `sha == HEAD` + `timestamp` < 1h ago + `jobs == all` → 跳过
重新跑 pipeline。否则删除 state 或标记 stale,下次 push 必须重跑完整 pipeline。

**为什么必须有这个 gate(v0.69-v0.72 的教训)**:

v0.69 一连 7 个 fix 都在 GHA 上才暴露问题:

| fix | 表面成功(本地) | 真实失败(CI) |
|---|---|---|
| v0.69c 切 Rust toolchain 1.77→1.88 | `cargo +1.88 check` pass | CI 报 `notify-rust 4.18 需要 1.89+` |
| v0.69e README sync step 移到 frontend job | 本地手测 grep 通过 | CI 上 `coverage-summary.json` 不存在 → script exit 1 |
| v0.69f 加 Linux 系统依赖 | macOS 已有 glib,build pass | CI ubuntu-latest 没装 → glib-sys 找不到 |
| v0.69g stub `dist/index.html` | 本地有 stale `dist/`,pass | CI 干净 checkout,`tauri::generate_context!()` panic |

`run-ci-local.sh` 通过以下手段把 CI 的"真实环境"逼近到本地:

| 手段 | 解决哪个问题 |
|---|---|
| 强制 `pnpm@9`(检测 + 提示) | pnpm 11 容忍空 workspace.yaml |
| 强制 `rustc 1.89`(rustup run) | 不用本地 default toolchain |
| 强制 `pnpm test:coverage` 后跑 sync check | coverage-summary.json 必须先存在 |
| `rm -rf dist && mkdir stub` | 模拟 CI 干净 checkout |
| Linux 上跑 apt-get 系统依赖 | macOS 跳过(brew 已有) |
| `rm -f ~/.polyrocket/sidecar/models/active.json` | 清除 pytest stale 状态 |
| `--test-threads=1` | 已知 race 修复(§10.3) |
| Cargo build 错误检测(teelog + grep) | 捕获编译错误,warnings 不阻塞 |
| Pytest pass 检测(teelog + grep) | 区分 pass / fail / collection error |

**Bypass(真·紧急情况)**:

```bash
git push --no-verify                      # git 原生 bypass,保留
```

**禁止**:
- ❌ `POLYROCKET_PRE_PUSH_SKIP=1`(v0.73a 起会直接失败 — env 被 hook 完全忽略)
- ❌ `run-ci-local.sh --quick`(v0.73a 起会直接 exit 2 + 提示原因)
- ❌ `--no-verify` 用作常规绕道 —— 每次用都要在 PR 描述里说明原因,否则 merge 会被拒
- ❌ 手动删除 `.git/CI_VERIFIED`(下次 push 会强制重跑全 pipeline,不是"作弊")

**完整流程**(标准 push):

```bash
# 1. 写代码
git add ...
git commit -m "..."

# 2. 推(hook 自动跑)
git push origin main
#   → hook 读 .git/CI_VERIFIED
#   → 若 stale 或不存在 → 跑 scripts/run-ci-local.sh(全 4 jobs,~3-5 分钟)
#   → cargo build/test + pytest 全 pass → 写 state file → push 成功
#   → 任一 fail → push BLOCKED,看 stderr 修

# 3. push 后
# GitHub Actions 立即收到 push,通常 3-5 分钟内 4 jobs 全部绿
# 如果 push 成功但 CI 红了 → hook 没拦住(罕见),需要查 runner 差异
```

---

## 11. CI Gate Policy(v0.73a 强制 — 完整契约)

> **本节是 §10.7 的上层契约**。目的是把"本地 pipeline 必须 pass 后才能 push"这
> 一规则**升级到 spec 级别**,而不是散落在 hook 注释里。所有维护者**首次入库前**必须
> 读完本节。违反 §11 的 commit / push 会被自动拒绝(hook 拦截)或人工 review 拒收。

### 11.1 三条铁律

1. **`scripts/run-ci-local.sh` 必须 exit 0 才能 push**。没有"差不多绿"的概念。
2. **`run-ci-local.sh` 必须跑全 4 个 job**(governance + L1 + Rust + Python)。
   - ❌ `--quick` / `POLYROCKET_PRE_PUSH_QUICK=1` / cargo skip —— **v0.73a 起全部禁止**
   - ✅ 唯一允许的跳过场景:CI runner 自己(GitHub Actions),hook 通过 `GITHUB_ACTIONS` / `CI` env 检测自动跳过
3. **State file `.git/CI_VERIFIED` 必须存在 + sha 匹配 + < 1h ago**,否则 push 会强制重跑。
   - 不要手动删除/编辑这个文件。它是 hook 自动维护的,不是给手用的。
   - 新 commit 会让 state file stale,这是**正确的行为**(新代码需重新验证)。

### 11.2 不可绕过的设计(为什么)

| 旧版(v0.69h-v0.72)允许的 bypass | v0.73a 移除原因 |
|---|---|
| `POLYROCKET_PRE_PUSH_SKIP=1 git push` | 整个 hook 不跑。多次 v0.69-v0.72 push 在 CI 上 fail,根因都是"本地 hook skip 了" |
| `run-ci-local.sh --quick` | Cargo 是 silent gap。push 时 cargo 没跑 → CI cargo 报错 → 浪费时间 |
| `git push --no-verify`(部分场景) | 仍保留,但 §11.3 强制每次使用要在 PR 描述里说明原因 |

**根因分析**:v0.69-v0.72 一共 5 次"remote pipeline erroring" 事件,3 次是 hook 被 skip,2 次是 `--quick` 跳过了 cargo。这些事件每次都浪费 5-10 分钟的 remote runner 时间 + 一次 fix-then-repush 循环。**净效应:用户为节省本地 1-3 分钟,花费了 10-20 分钟的 remote 重试**。

### 11.3 git push --no-verify 的使用条件

仅以下场景允许 `--no-verify`,且每次都要在 PR description 写明:

1. **Re-push 一个已知 CI 绿色的 SHA**(例如:CI runner 临时挂了,需要重跑)。state file
   还在有效期,但 SHA 没变,理论上不需要 `--no-verify`;只在 state file 损坏或 `.git/` 损坏时才用。
2. **Push 到非主分支**(开发分支 / 实验分支),不在 CI 上跑 main 流水线。
3. **紧急 hotfix**(security CVE / 线上故障)—— 走完正常 review 后再 push,但跳过本地 gate
   以避免环境差异浪费时间。**事后必须立即补一个 "post-hotfix hardening" commit 重跑全 pipeline**。

**禁止场景**:
- ❌ "我手动跑过 cargo build 了" —— 不能取代 `run-ci-local.sh` 的全套环境模拟
- ❌ "CI 上次是绿的所以这次也一定是绿的" —— 新 commit 可能引入 regression
- ❌ "我赶时间" —— §11 的存在就是为了防止"赶时间"的累积风险

### 11.4 State file 协议(`.git/CI_VERIFIED`)

| 字段 | 来源 | 用途 |
|---|---|---|
| `sha=<40-char-hex>` | `git rev-parse HEAD` | hook 检查 sha 一致性,新 commit 自动 stale |
| `timestamp=<unix>` | `date +%s` | hook 检查新鲜度,1h 后强制重跑 |
| `jobs=all` | 写死 | 标记是完整 4 jobs run(预留 `--quick` 已删除,但保留字段以便将来 audit) |
| `runner=<host>:<pid>` | `hostname` + `$$` | 记录哪台机器跑的,debug 用 |

**生命周期**:
- 写入:`run-ci-local.sh` exit 0 后立刻写
- 读取:`pre-push-hook.sh` 每次 push 前读
- 失效:任何新 commit、> 1h 后、文件被删,都会触发重跑
- **不会被 commit**(在 `.git/` 目录下,git ignore)

**为什么不放 GitHub Actions secrets / 环境变量?**  state file 是本地的、本地读的、本地写的,
不需要 server 端验证。简单可靠。

### 11.5 Hook 自身的 fail-safe

```bash
set -euo pipefail    # pre-push-hook.sh 顶部:任何错误立即 fail
```

如果 hook 本身因为文件系统损坏 / `git rev-parse` 异常 fail,**会按设计 block push**。
这是故意的:宁可误杀 1 个 push,也不要让 silent gate fail 通过。

**恢复方法**:
```bash
# 1. 手动跑一次 runner(不依赖 hook),确认环境健康
./scripts/run-ci-local.sh

# 2. 如果 runner 也跑不通,先修复环境(rustup / pnpm / python 装好)

# 3. runner 通过后 state file 会自动写入,hook 恢复

# 4. 如果只是 hook 脚本本身坏了(罕见的 shell 兼容性问题)
cp scripts/pre-push-hook.sh .git/hooks/pre-push
chmod +x .git/hooks/pre-push
```

### 11.6 升级历史

| 版本 | 变更 |
|---|---|
| v0.69h | 引入 pre-push hook + `run-ci-local.sh`(可选 bypass) |
| v0.73a | **HARDENED**:`POLYROCKET_PRE_PUSH_SKIP` env 移除;`--quick` 移除;state file `.git/CI_VERIFIED` 引入;本文档 §11 新增 |

### 11.7 后续演进路线(roadmap)

- **v0.74 candidate**:加 **pre-commit hook**(轻量版,只跑 governance + typecheck,~30s),
  让 commit 时也能发现明显错误,不必等到 push。
- **v0.75 candidate**:把 state file 移到 git notes(`git notes add -m '...' HEAD`),跨 clone 共享。
- **v0.76+ candidate**:研究 **post-commit hook**,commit 后自动跑 cargo check(后台,不阻塞 commit)。

这些都不影响 §11.1 三条铁律,只是补强。

---

## 12. Visual Acceptance Gate(v0.74d 强制 — 防止 nav-item-class-of-bugs 复发)

### 12.1 背景:v0.74 缺失 CSS 事件

`AppShell.tsx` 从 initial commit (c6ec76b) 起就在 4 处用 `.nav-item` class,
**但 `src/styles/globals.css` 从未定义过这个 class**。症状:

- sidebar icon 用 SVG 默认 24×24 渲染(没应用 `w-3.5 h-3.5`)
- 布局 `block` 而非 `flex`,无 padding,无 hover state,无 active 高亮
- 整个 v0.13 → v0.74(~3 个月) 都在 broken 状态下 push
- v0.74a 的 18-route console-error audit **没发现**,因为脚本只检查
  console 输出,不检查视觉渲染

**为什么 typecheck / vitest / CI 都没发现**:

| 检查 | 假阴性原因 |
|---|---|
| `tsc --noEmit` | `className="nav-item"` 是 string,语法 OK |
| `vitest run` | happy-dom 不加载真实 CSS,layout 不真渲染 |
| `gh actions` 4 jobs | governance / typecheck / cargo / python 都不验证 CSS |
| 18-route console audit | 只看 `console.error` / `console.warn`,不看 layout |
| **视觉检查** | **从未做**(没有任何 screenshot diff / Playwright visual) |

### 12.2 三层防御(v0.74d 落地)

| 层 | 工具 | 何时 | 失败影响 |
|---|---|---|---|
| **1. 静态 class coverage lint** | `scripts/check-class-coverage.mjs` | pre-push(加进 `run-ci-local.sh`)| 任何 TSX 引用但 CSS 未定义的 class → exit 1 |
| **2. 真实 browser 渲染** | `pnpm dev` + Playwright 截图 | v0.74d+ 每次提 PR 前手动 | 视觉问题在 commit 前发现 |
| **3. 自动视觉回归(roadmap)** | Playwright `expect(page).toHaveScreenshot()` | v0.75 candidate | pixel-level diff,任何 layout 漂移 fail |

### 12.3 静态 class coverage lint 用法

```bash
node scripts/check-class-coverage.mjs            # 列出所有 missing class
node scripts/check-class-coverage.mjs --strict   # 严格模式(预留)
```

**lint 行为**:
- 扫描 `src/**/*.{ts,tsx}` 里所有 `className="..."` 和 `className={cn('...')}` 用到的 token
- 排除 Tailwind utility(检测:bare `flex/border/center/...`, prefix `text-/bg-/...`, variant `hover:/md:/focus-visible:`, arbitrary `[13px]`)
- 排除 lucide-react icon class (`lucide-*`)
- 排除 `active`(`.nav-item.active` 的子状态)
- 对所有剩余 class,检查 `src/styles/globals.css` + `src/styles/themes.css` + `src/index.css` 里是否有 `.classname {` 选择器
- 缺失 → exit 1 + 列出全部 missing class

**v0.74c 验证**:`nav-item` 修复后 → 363 文件 / 309 unique class,全部有 CSS。
如果有人删掉 `.nav-item { ... }` rule,下次跑 lint 会立即报。

### 12.4 视觉验证(手动,每次大 UI 改动后必跑)

```bash
# 1. 启动 Vite dev
pnpm dev   # http://localhost:1420

# 2. 截所有 18 个 route 的图(推荐用 Playwright)
# 至少必截:sidebar(Dashboard) / Settings / Lab / Analysis / Wallets
# 因为这 5 个覆盖了 AppShell + Modal + List + Form + ErrorState

# 3. 视觉 checklist(看每张图问自己):
#   - sidebar icon 是否正常显示(14×14,不是 24×24)?
#   - 文字颜色 / 间距 / 对齐符合 spec v2.2?
#   - 3 主题切换(Dark/Light/Matrix)都正常?
#   - 没有 overflow / overlap / 文字截断异常?
```

**commit 门禁**:任何动 `src/styles/`,`src/components/layout/`,`src/components/feedback/` 的
commit,**必须**附至少 1 张 Playwright 截图(sidebar 或被改的 component)证明没破。

### 12.5 失败案例总结(从 v0.74c 学到的教训)

1. **TypeScript happy ≠ 视觉 OK**:`className="X"` 是 string,不验证 X 存在
2. **vitest happy-dom 不渲染真实 CSS**:layout bug 永远抓不到
3. **Console error 监控 ≠ 视觉监控**:silent visual regression 不会触发 console
4. **Coverage ratchet 不覆盖 CSS**:8 commits 全部 4/4 CI 绿,但样式一直坏
5. **用户视觉反馈仍是最后一道防线**:18-route 脚本 + 8 commits 都没发现,人工 review 一眼看出

**结论**:lint(自动化) + 视觉(人工/screenshot) 两层都不能省。Lint 抓 95% 的 case,视觉抓剩下 5% 的 silent regression。

### 12.6 与 §11 CI Gate Policy 的关系

| 触发时机 | 工具 | 拦哪类问题 |
|---|---|---|
| pre-commit(v0.74 candidate) | check-class-coverage + tsc | class coverage + TS error |
| pre-push(已落地) | run-ci-local.sh 4 jobs | governance / L1 / Rust / Python + **class coverage**(v0.74d 加) |
| post-push GitHub Actions | 4 jobs | 同上,但在 CI runner |
| 视觉(每 PR 必跑) | Playwright screenshot | silent visual regression |

**lint 必须在 pre-push 跑**:v0.74d 起 `run-ci-local.sh` job 1 governance 增加
`node scripts/check-class-coverage.mjs` 调用(详见 §6 CI 校验)。
