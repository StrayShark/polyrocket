# polyrocket — Coding Spec

> 代码注释 / 文档化规范。**所有新增代码必须遵循此规范；存量代码按 v0.61 计划分轮翻新。**

**版本**：v1.0 · 2026-06-18
**配套**：[`overview.md`](./overview.md)（5 层架构） · [`polyrocket-modules.md`](./polyrocket-modules.md)（17 模块业务） · [`polyrocket-flows.md`](./polyrocket-flows.md)（20 交互流程）

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

| 语言 | 模块类别 | 目标注释行占比 | 当前（v0.60 final） |
|---|---|---|---|
| Rust | `commands` / `domain` / `infra` | **≥ 15%** | ~5% |
| Rust | `platform` | ≥ 10% | ~15% |
| TypeScript | `routes` / `components` / `lib` / `stores` | **≥ 10%** | ~5% |
| TypeScript | `types` | ≥ 5% | ~15% |
| Python | `polyrocket_sidecar/` | **≥ 12%** | ~8% |

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
| v1.0 | 2026-06-18 | 初版：Rust + TS + Python 三套规则 + 密度目标 + CI 校验 |
