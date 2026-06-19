# Bankroll Allocation — Design (v0.78)

> Status: **draft (v0.78) — design + L2 prototype**. L1 IPC + L3
> UI planned for v0.78b/c. The design below is the contract
> between L2 (Rust domain) and L1/L3 (Tauri command + UI).
>
> Spec: [`polyrocket-modules.md`](./polyrocket-modules.md) M11
> (Bankroll Management — 新增 module)
>
> Trigger: 用户需求 (2026-06-19) — "AI 应当可以根据资金剩余量 +
> 预期收益率进行下注分配"。

---

## 0. 目标

把"按信号逐个手动下注"升级为"AI 驱动的多信号组合分配":
- 输入: 剩余资金 + N 个 signal (edge / confidence / market_prob)
- 输出: 每个 signal 的建议下注金额 + 总分配 + 保留金

核心算法: **Fractional Kelly Criterion + 多重约束** (per-signal cap /
total exposure cap / reserve / market grouping)。

---

## 1. 算法 (deterministic, no IO)

### 1.1 Kelly 基础

每信号独立计算 Kelly fraction:

```
b = (1 / market_prob) - 1             // 净赔率
p = predicted_prob                    // 赢的概率 (LLM 输出)
q = 1 - p                             // 输的概率
f_kelly = (b * p - q) / b             // optimal fraction
f_fractional = f_kelly * kelly_mult   // user setting, 0.25 = quarter-Kelly
```

边界:
- `f_kelly <= 0` (没 edge) → 跳过
- `market_prob == 0` (空盘) → 跳过
- `market_prob == 1` (套利已消失) → 跳过 (避免 div by zero)
- `predicted_prob > 1` (LLM 输出异常) → clamp to 1

### 1.2 多信号分配

```
1. Filter: edge < min_edge_pct → 跳过
2. Group: 同 market_id 合并 → 1 个 signal (保留 max |edge|)
3. Per-signal:
   raw_alloc = f_fractional * bankroll
   capped_alloc = min(raw_alloc, max_per_signal_pct * bankroll)
   if market_liquidity available: capped_alloc = min(capped_alloc, liquidity)
4. Reserve:  total_alloc <= bankroll * (1 - reserve_pct)
5. If total > max_total: scale all proportionally
   (proportional scale preserves Kelly ratios)
6. Round: 每个 alloc round to 2 decimals ($0.01 粒度)
```

### 1.3 关键不变量

- `sum(allocations) + reserve <= bankroll`
- `0 <= allocation[i] <= max_per_signal_pct * bankroll`
- `allocations 全部 >= 0` (filter 已剔除 negative edge)
- `grouped 之后: 1 market = 最多 1 alloc`

---

## 2. 数据结构

### 2.1 L2 domain (Rust)

```rust
// src-tauri/src/domain/bankroll/mod.rs

pub struct BankrollConfig {
    pub kelly_multiplier: f64,        // 0.25 = quarter-Kelly
    pub max_per_signal_pct: f64,      // 0.10 = 10% per signal
    pub reserve_pct: f64,             // 0.20 = 20% reserve
    pub min_edge_pct: f64,            // 0.05 = 5% min edge
    pub max_total_exposure_pct: f64,  // 0.80 = 80% total
    pub min_confidence: f64,          // 0.6 = 60% min
}

pub struct BankrollSnapshot {
    pub available_usdc: String,       // bankroll - already-allocated
    pub reserved_usdc: String,       // reserve_pct * bankroll
    pub allocated_usdc: String,       // sum of open bets
    pub free_usdc: String,            // available - this allocation
    pub wallet_id: String,
}

pub struct AllocationInput {
    pub bankroll_usdc: String,        // total available
    pub config: BankrollConfig,
    pub signals: Vec<Signal>,
    pub market_liquidity: HashMap<String, String>,  // market_id → max $ allocatable
}

pub struct AllocationItem {
    pub market_id: String,
    pub side: BetSide,                 // Yes / No
    pub size_usdc: String,            // 最终分配金额
    pub kelly_pct: f64,               // 原始 Kelly 比例
    pub capped_reason: Option<String>, // "per-signal cap" / "liquidity" / "scale-down"
    pub source_signal_ids: Vec<String>, // 被 group 的原始 signals
    pub model_version: String,
    pub confidence: f64,
    pub expected_roi: f64,            // edge * confidence (per 注释)
}

pub struct AllocationResult {
    pub total_allocated_usdc: String,
    pub reserved_usdc: String,
    pub per_market: Vec<AllocationItem>,
    pub dropped_signals: Vec<String>, // market_ids dropped due to liquidity
}
```

### 2.2 L1 IPC (Tauri commands)

```rust
// src-tauri/src/commands/bankroll.rs

#[tauri::command]
pub async fn get_bankroll_config(state: State) -> AppResult<BankrollConfig>;

#[tauri::command]
pub async fn set_bankroll_config(state: State, config: BankrollConfig) -> AppResult<()>;

#[tauri::command]
pub async fn get_bankroll_snapshot(state: State, wallet_id: String) -> AppResult<BankrollSnapshot>;

#[tauri::command]
pub async fn compute_allocation(state: State, input: AllocationInput) -> AppResult<AllocationResult>;

#[tauri::command]
pub async fn apply_allocation(state: State, result: AllocationResult) -> AppResult<Vec<String>>;  // bet_ids
```

### 2.3 L1 wrappers (TypeScript)

```ts
// src/ipc.ts (new wrappers)
export function getBankrollConfig(): Promise<BankrollConfig>;
export function setBankrollConfig(config: BankrollConfig): Promise<void>;
export function getBankrollSnapshot(walletId: string): Promise<BankrollSnapshot>;
export function computeAllocation(input: AllocationInput): Promise<AllocationResult>;
export function applyAllocation(result: AllocationResult): Promise<string[]>;
```

### 2.4 L3 route (UI)

- New route: `/bankroll`
- Sidebar nav: new entry "Bankroll" with `Wallet` icon
- Page sections:
  - **Bankroll Summary Card**: available / reserved / allocated / free
  - **Allocation Table**: each market with size / kelly / capped-reason
  - **Config Card**: kelly_mult / max-per-signal / reserve / min-edge (sliders)
  - **Apply Button**: writes to `bets` table (mode = `'C_allocated'`)

### 2.5 L4 components

```tsx
// src/components/feedback/BankrollCard.tsx
// src/components/feedback/AllocationTable.tsx
// src/components/feedback/AllocationPreview.tsx
```

---

## 3. DB schema (新增)

```sql
-- v0.78 — bankroll_config (per-wallet)
CREATE TABLE bankroll_config (
  wallet_id TEXT PRIMARY KEY,
  kelly_multiplier REAL NOT NULL DEFAULT 0.25,
  max_per_signal_pct REAL NOT NULL DEFAULT 0.10,
  reserve_pct REAL NOT NULL DEFAULT 0.20,
  min_edge_pct REAL NOT NULL DEFAULT 0.05,
  max_total_exposure_pct REAL NOT NULL DEFAULT 0.80,
  min_confidence REAL NOT NULL DEFAULT 0.6,
  updated_at INTEGER NOT NULL
);

-- v0.78 — allocation_batches (one batch = one compute + apply)
CREATE TABLE allocation_batches (
  id TEXT PRIMARY KEY,         -- uuid
  wallet_id TEXT NOT NULL,
  bankroll_usdc TEXT NOT NULL,
  config_json TEXT NOT NULL,   -- snapshot of BankrollConfig
  total_allocated_usdc TEXT NOT NULL,
  applied_at INTEGER NOT NULL,
  FOREIGN KEY (wallet_id) REFERENCES wallets(id)
);

-- v0.78 — extend bets with allocation_id
ALTER TABLE bets ADD COLUMN allocation_id TEXT
  REFERENCES allocation_batches(id);
```

---

## 4. 边界条件 + 测试矩阵

| 场景 | 输入 | 预期输出 |
|---|---|---|
| Zero bankroll | bankroll=$0 | total=0, no per_market |
| Zero edge (all) | 5 signals with edge=0 | total=0 |
| Negative edge | signal.edge=-0.10 | skipped (filter) |
| Single signal huge | bankroll=$1000, kelly=0.50, cap=0.10 | alloc=$100 (capped) |
| Same market 2 signals | 2 signals on market X | grouped → 1 alloc |
| Liquidity < alloc | alloc=$50, liquidity=$30 | alloc=$30, capped_reason="liquidity" |
| Total > max_exposure | sum=$800, max=$600 | scale all by 0.75 |
| Reserve violated | alloc=bankroll | reserved=20%, alloc=80% |
| market_prob=1 | div by zero | return 0 (no alloc) |
| market_prob=0 | no liquidity | skipped (filter at step 3) |
| predicted_prob > 1 | clamp | use 1.0 (saturating) |
| Empty signals | [] | empty result, no error |
| Confidence < min | conf=0.5, min=0.6 | skipped |
| Round to 2 decimals | $12.345 | $12.35 (banker's round) |
| Tie in |edge| | 2 signals with edge=0.10 | first-by-id wins (deterministic) |

---

## 5. v0.78 拆分

| sub | 内容 | 工时 |
|---|---|---|
| v0.78a | L2 核心算法 + 13 个 deterministic unit tests | 1h |
| v0.78b | L1 IPC + DB migration (bankroll_config / allocation_batches) + 5 integration tests | 1.5h |
| v0.78c | L3 /bankroll route + L4 BankrollCard / AllocationTable + 8 component tests | 2h |
| v0.78d | Sidebar nav entry + Settings → Bankroll tab (config sliders) | 1h |
| v0.78e | 端到端: 1 active signal, compute → preview → apply → verify bet row | 1.5h |
| v0.78 final | ship log + overview v2.42 + spec v2.3 | 30min |

**Total: ~7.5h**. v0.78 = "feature round", 不是 coverage ratchet.

---

## 6. 暂不做 (v0.79+)

- **Correlation matrix**: signals on correlated markets → reduce aggregate allocation
- **Slippage modeling**: limit orders vs market orders → adjust for expected slippage
- **Auto-rebalance**: when new signal arrives, recompute and adjust existing
- **Cross-wallet**: split across multiple wallets
- **Live Kelly updates**: Brier score degrades → reduce multiplier
- **Backtest mode**: replay historical signals against config to find optimal
- **Tax-lot tracking**: each allocation batch has tax implications

---

## 7. 风险 + 缓解

| 风险 | 缓解 |
|---|---|
| Kelly 过度激进 → 大回撤 | 默认 quarter-Kelly (0.25) + 用户可调 |
| 多信号同方向 → 过度集中 | 强制 max_per_signal_pct (10%) + grouping |
| 流动性不足 → 实际成交价滑点 | 接受 market_liquidity 输入, cap 于此 |
| LLM 输出异常 (predicted_prob > 1) | clamp to 1.0 + log warning |
| 浮点累积误差 | 最后一步 round to 2 decimals |
| 用户改 config → 既有 batch 不一致 | allocation_batches.config_json 快照 |
| 同一 batch 重复 apply | apply_alloc 检查 allocation_id 已存在 |

---

## 8. 配置默认值 (rationale)

| Param | Default | Rationale |
|---|---|---|
| `kelly_multiplier` | 0.25 | "Quarter-Kelly" — 业内标准,接受 25% Kelly 减少方差 |
| `max_per_signal_pct` | 0.10 | 单一信号不超过 10% bankroll — 避免单点失败 |
| `reserve_pct` | 0.20 | 20% 永远不动 — 心理缓冲 + 应对回撤 |
| `min_edge_pct` | 0.05 | 5% minimum edge — 低于此视为噪声 |
| `max_total_exposure_pct` | 0.80 | 总投入不超过 80% — 与 reserve_pct 互补 |
| `min_confidence` | 0.60 | 60% minimum confidence — LLM 自报信心下限 |

---

## 9. 与 v0.75-v0.77 coverage ratchet 关系

v0.78 是 **feature round**, 不动 coverage ratchet 主线。
但:
- L2 algorithm + tests → 自动提升 domain 模块 coverage
- L1 IPC mocks → 走 `createIpcMock` 模式, 无副作用
- L3/L4 components → 8+ component tests, 加 ~5pp stmts/br 估计

预期 v0.78 后 coverage 反而会升 (算法是纯函数,容易 100% cover)。

---

## 10. 验收标准 (v0.78 final)

- [ ] L2 13+ unit tests PASS (deterministic, no IO)
- [ ] L1 IPC 5+ integration tests PASS (Rust unit + cargo test)
- [ ] L3 8+ component tests PASS (vitest)
- [ ] E2E smoke: 1 signal → compute → apply → bet row inserted
- [ ] Cargo test 全 green (--test-threads=1)
- [ ] pnpm typecheck 0 errors
- [ ] pnpm test 全 green (819+ tests, +N new)
- [ ] pnpm test:coverage 不降 (≥ 86.34/83.99/80.06/87.57)
- [ ] Threshold 86/83/80/87 仍 pass (可能 bump)
- [ ] 5 类 config 字段在 Settings page 有 slider UI
- [ ] /bankroll route 渲染无 console error
- [ ] Doc: bankroll-allocation-design.md (this file) + overview v2.42 + spec v2.3 + ship log
