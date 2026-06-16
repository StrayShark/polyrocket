# polyrocket — UI 设计规范 (UID)

> 版本：v1.0 · 2026-06-16
> 配套：[`polyrocket-modules.md`](./polyrocket-modules.md)（数据来源）
> 设计基线：Cursor IDE（dark / light） + 配色变体（matrix）
> 强约束：[`polyradar-dev-governance.md §11`](../polyradar-dev-governance.md) — 三主题仅配色差异

---

## 0. 设计原则（6 条）

1. **同一布局，三套配色**——三主题共享全部布局 / 字体 / 圆角 / 阴影 / 动效。
2. **信息密度 > 装饰**——单屏可见 4 个 KPI + 1 张图表 + 4 行表 + 4 行列表。
3. **数据是主角**——数字、走势、信号都比 chrome 更突出。
4. **Cursor 风骨架**——220px 紧凑侧栏、12px 行高、8px 圆角、11px 大写分组标签。
5. **避免误操作**——所有写操作（Place / Delete / Import Key）需要二次确认或 undo。
6. **键盘优先**——⌘K 全局搜索、⌘⇧L 切主题、ESC 关闭弹窗。

---

## 1. 主题 Token 矩阵

> 三主题仅 color tokens 差异。layout / typography / radius / shadow / motion 完全相同。

| Token | dark (Cursor / VS Code Dark+) | light | matrix (Codex CLI) | 用途 |
|---|---|---|---|---|
| `--bg` | `#1E1E1E` | `#FFFFFF` | `#0A0A0A` | 页面底色 |
| `--surface` | `#181818` | `#F3F3F3` | `#0F0F0F` | sidebar / 卡片 |
| `--surface-2` | `#252526` | `#ECECEC` | `#161616` | input / segment bg |
| `--surface-hover` | `#2A2D2E` | `#E5E5E5` | `#1C1C1C` | hover |
| `--border` | `#2D2D2D` | `#E4E4E7` | `#1F1F1F` | 1px |
| `--border-strong` | `#3F3F46` | `#D4D4D8` | `#2A2A2A` | focus |
| `--fg` | `#D4D4D4` | `#1F2328` | `#D4D4D4` | 主文字 |
| `--fg-secondary` | `#9D9D9D` | `#4B5263` | `#8A8A8A` | 次文字 |
| `--muted` | `#6B6B6B` | `#6B7280` | `#5C5C5C` | placeholder / hint |
| `--accent` | `#007ACC` | `#2563EB` | `#10A37F` | 主色 |
| `--accent-hover` | `#1F8AD2` | `#1D4ED8` | `#1AB892` | hover |
| `--bull` | `#4EC9B0` | `#1A7F37` | `#10A37F` | 涨 / Yes / won |
| `--bear` | `#F48771` | `#CF222E` | `#E84A4A` | 跌 / No / lost |
| `--warning` | `#DCDCAA` | `#9A6700` | `#D4A72C` | 警告 |
| `--shadow-card` | `0 1px 0 rgba(0,0,0,0.4)` | `0 1px 2px rgba(0,0,0,0.04)` | `none` | 卡片 |
| `--shadow-overlay` | `0 8px 24px rgba(0,0,0,0.6)` | `0 8px 24px rgba(0,0,0,0.12)` | `0 8px 32px rgba(0,0,0,0.7)` | popover / modal |

**全局变量（所有主题共用）**：
```css
--font-sans: 'Inter', system-ui, sans-serif;
--font-mono: 'JetBrains Mono', Menlo, monospace;
--radius-sm: 4px;
--radius-md: 6px;
--radius-lg: 8px;
--duration-fast: 80ms;
--duration-base: 160ms;
--duration-theme: 180ms;
--ease-out: cubic-bezier(0.16, 1, 0.3, 1);
```

**禁止行为**（governance §11 强约束）：
- ❌ matrix 主题强制等宽字体
- ❌ matrix 主题用 ASCII 字符画边框
- ❌ matrix 主题加扫描线 / phosphor glow
- ❌ matrix 主题 `polyradar >` prompt 前缀
- ❌ matrix 主题用块字符 sparkline
- ❌ matrix 主题动效加速
- ❌ matrix 主题圆角缩小
- ❌ matrix 主题阴影改为边线
- ❌ 任何主题调整字间距
- ❌ 提交 `--matrix-*` 系列 CSS 变量

---

## 2. 全局布局

```
┌──────────────────────────────────────────────────────────────────────┐
│ TopBar (h=48) — breadcrumb · ⌘K search · sync · bell · settings · D │
├──────┬───────────────────────────────────────────────────────────────┤
│      │                                                                │
│ Side │   Main Content (路由出口)                                       │
│ bar  │                                                                │
│ w=220│   ┌──────────────────────────────────────────────────┐         │
│      │   │  Page Header (title + tabs/actions)              │         │
│ Logo │   ├──────────────────────────────────────────────────┤         │
│ ─── │   │                                                  │         │
│ Wrk │   │  Page Body (KPIs / Charts / Tables / Lists)       │         │
│ Mkt │   │                                                  │         │
│ Sgn │   │                                                  │         │
│ Cpy │   │                                                  │         │
│ P&L │   │                                                  │         │
│ Lab │   │                                                  │         │
│ ─── │   └──────────────────────────────────────────────────┘         │
│ Cat │                                                                │
│ Ftb │                                                                │
│ CS2 │                                                                │
│ Plt │                                                                │
│ ─── │                                                                │
│ Set │                                                                │
│ ─── │                                                                │
│ Thm │                                                                │
│ Wlt │                                                                │
└──────┴───────────────────────────────────────────────────────────────┘
```

---

## 3. 路由 → 页面矩阵

| 路由 | 页面 | 主要组件 | 关键数据源 |
|---|---|---|---|
| `/dashboard` | Dashboard | KpiCard × 4, EquityCurve, CalibrationChart, SignalsTable, OpenPositionsList, ActivityTimeline | M1-M6 |
| `/markets` | Markets | MarketsTable (TanStack Table) | M1 |
| `/signals` | Signals | SignalsTable (full) + SignalDetailPanel | M2 |
| `/copy` | Copy Trading | CopyTargetsList, CopyEventsTimeline, MirrorStrategyForm | M5 |
| `/pnl` | P&L | PnLCurveChart, CategoryBreakdown, BetsHistoryTable | M3, M6 |
| `/lab` | Model Lab | ModelVersionsList, PerformanceComparison, CalibrationOverlay | M7 |
| `/wallets` | Wallets (v0.2) | WalletsTable, AddWalletForm, KeyringManager | M4 |
| `/settings` | Settings | ThemePicker, ThresholdsForm, NotificationsConfig | M9 |

---

## 4. 组件库（24 个核心组件）

### 基础（shadcn 衍生）

| 组件 | 用途 | 关键 props |
|---|---|---|
| `Button` | 主按钮 | `variant: 'primary' \| 'default' \| 'ghost' \| 'danger'` |
| `Input` | 输入框 | `error?: string` |
| `Select` | 下拉 | 基于 Radix |
| `Checkbox` | 勾选 | – |
| `Toggle` | 开关 | `checked`, `onChange` |
| `Slider` | 滑块 | `min`, `max`, `step`, `value` |
| `Tabs` | 标签页 | `value`, `onChange`, list of `{label, content}` |

### 反馈

| 组件 | 用途 | 关键 props |
|---|---|---|
| `Toast` | 操作反馈 | `kind: 'success' \| 'error' \| 'info'`, `message`, `duration?` |
| `Dialog` | 模态对话框 | `title`, `description`, `open`, `onOpenChange` |
| `Tooltip` | 悬停提示 | `content`, `side`, `delay?` |
| `Popover` | 弹出面板 | 同 Tooltip 但可交互 |
| `Command` | ⌘K 命令面板 | `items: CommandItem[]` |

### 数据展示

| 组件 | 用途 | 关键 props |
|---|---|---|
| `KpiCard` | 单指标卡 | `label`, `value`, `delta`, `hint?` |
| `DataTable` | TanStack Table 包装 | `columns`, `data`, `onRowClick?`, `empty?` |
| `EquityCurve` | Recharts area chart | `data: PnLCurvePoint[]`, `baseline?: boolean` |
| `CalibrationChart` | 散点 + 对角线 | `buckets: CalibrationBucket[]` |
| `PriceFlash` | 数字 + 涨跌闪烁 | `value`, `prev?`, `flash?: 'up' \| 'down' \| 'none'` |
| `Sparkline` | 微型折线 | `data: number[]`, `width?`, `height?` |
| `OrderBook` | Yes/No 深度条 | `bids`, `asks`, `mid` |
| `ProgressBar` | 进度条 | `value`, `max`, `tone?: 'bull' \| 'bear' \| 'accent'` |

### 业务专用

| 组件 | 用途 | 关键 props |
|---|---|---|
| `SignalRow` | Active Signals 表行 | `signal: SignalDto`, `onJump`, `onCopy` |
| `BetRow` | Bets 表行 | `bet: BetDto`, `onSettle` |
| `WalletCard` | 钱包卡 | `wallet: WalletDto`, `onEdit`, `onDelete` |
| `CopyTargetRow` | 复制目标行 | `target: CopyTargetDto`, `events: CopyEventDto[]` |
| `ActivityTimeline` | 操作时间线 | `items: ActivityItem[]` |
| `CalibrationOverlay` | 多模型对比 | `versions: {label, buckets}[]` |

---

## 5. 页面详细规范

### 5.1 Dashboard `/dashboard`

**布局**（自顶向下，v0.2 含 Daily Brief）：

```
┌─────────────────────────────────────────────────────────────┐
│ PageHeader                                                   │
│   h1 "Good evening, dutong"                                  │
│   subtitle "{active_signals} active · {open} positions ·     │
│             @trader filled 4 orders"                         │
│                                                              │
│   [24H|7D|30D|All]  [Export ↧]                              │
├─────────────────────────────────────────────────────────────┤
│ Today's Brief (M12) — top 5 markets worth attention today    │
│   ┌────────────────────────────────────────────────────┐    │
│   │ ⭐ LGD vs Spirit — Map 2     match 87 · 3h 12m   │    │
│   │ signal +13% · 3/4 LLM agree YES                   │    │
│   │ [Open analysis ↗] [Watchlist] [Skip] [Why?]      │    │
│   ├────────────────────────────────────────────────────┤    │
│   │ ⭐ Man City vs Arsenal        match 79 · 8h 04m   │    │
│   │ ...                                                │    │
│   ├────────────────────────────────────────────────────┤    │
│   │ (3 more)                                           │    │
│   └────────────────────────────────────────────────────┘    │
│   [Refresh ↻]  [Settings ⚙]   last refreshed 00:00 UTC       │
├─────────────────────────────────────────────────────────────┤
│ KPI Row (grid-cols-4, gap=12)                                │
│   ┌──────┐ ┌──────┐ ┌──────┐ ┌──────┐                       │
│   │Equity│ │PnL   │ │Win%  │ │Brier │                       │
│   │$12.8k│ │+$187 │ │68.4% │ │0.142 │                       │
│   └──────┘ └──────┘ └──────┘ └──────┘                       │
├─────────────────────────────────────────────────────────────┤
│ Charts Row (grid-cols-3, gap=12)                             │
│   ┌─────────────────────────┐ ┌─────────────────┐            │
│   │ Equity Curve (2/3)      │ │ Calibration     │            │
│   │ [SVG line + area]       │ │ [scatter]       │            │
│   └─────────────────────────┘ └─────────────────┘            │
├─────────────────────────────────────────────────────────────┤
│ Active Signals Card                                          │
│   Header: "Active Signals" · edge ≥ 5% · live                │
│   Actions: [Edge>5% ▾] [Refresh] [+ New Signal]              │
│   Table: Market · Our · Mkt · Edge · Conf · Action           │
├─────────────────────────────────────────────────────────────┤
│ Bottom Row (grid-cols-2)                                     │
│   ┌─────────────────────┐ ┌─────────────────────┐           │
│   │ Open Positions      │ │ Recent Activity     │           │
│   │ [list, 4-6 rows]    │ │ [timeline]          │           │
│   └─────────────────────┘ └─────────────────────┘           │
└─────────────────────────────────────────────────────────────┘
```

**Daily Brief 卡片内部结构**（每条 market）：

```
┌────────────────────────────────────────────────────┐
│ ⭐  LGD vs Spirit — Map 2 winner       match 87   │
│   closes in 3h 12m  ·  CS2  ·  vol $124k          │
│                                                    │
│   signal edge +13%  ·  3/4 LLM agree YES          │
│   GPT 71% · Claude 64% · Gemini 58% · DSeek 67%  │
│                                                    │
│   [Open analysis ↗]  [Watchlist]  [Skip]  [Why?]  │
└────────────────────────────────────────────────────┘
```

**「Why?」展开**：

```
┌────────────────────────────────────────────────────┐
│ Why this match?                                     │
│ ─────────────                                       │
│ Edge (+13%):              +0.35  ████████████       │
│ Confidence (model):      +0.20  ███████           │
│ LLM consensus (3/4):     +0.18  ██████            │
│ Time decay (3h):         +0.14  █████             │
│ User interest:           +0.00  ·                  │
│ Cost penalty:            −0.05  ██                │
│ ─────────────                                       │
│ Total: 87  ████████████████                       │
│ [Adjust weights ⚙]                                 │
└────────────────────────────────────────────────────┘
```

**KpiCard 内部结构**：
```
┌─────────────────────────┐
│ Label (11px muted)   [↗]│
│ $12,847.22              │  ← 22px semibold tabular-nums
│ +$341.20 (+2.73%) · 24H │  ← 11px, success 色
└─────────────────────────┘
```

---

### 5.2 Markets `/markets`

**布局**：

```
┌─────────────────────────────────────────────────────────────┐
│ PageHeader                                                   │
│   h1 "Markets"                                               │
│   subtitle "128 active · last sync 14:32:08"                │
│                                                              │
│   Filter: [Category ▾] [Status ▾] [Sort: end_date ▾]        │
│   [Sync now ↻]                                               │
├─────────────────────────────────────────────────────────────┤
│ DataTable                                                    │
│   Columns:                                                   │
│     - Market (question + slug)                              │
│     - Category (pill)                                        │
│     - End (relative: "2h 14m")                              │
│     - Volume 24h (USDC)                                      │
│     - Liquidity                                              │
│     - Our edge (if signal exists)                            │
│     - Action (Jump →)                                        │
│                                                              │
│   Row hover: bg-surface-hover                                │
│   Click row: opens DetailDrawer (right)                     │
└─────────────────────────────────────────────────────────────┘
```

**DetailDrawer**（右侧滑出，宽 400px）：
- Market 完整描述
- OrderBook 组件（YES / NO 深度）
- 当前信号（如果有）
- 历史价格曲线（30D）
- "Open in Polymarket" 跳转

---

### 5.3 Signals `/signals`

**布局**：

```
┌─────────────────────────────────────────────────────────────┐
│ PageHeader                                                   │
│   h1 "Signals"                                               │
│   subtitle "{N} active · {M} suppressed · next recompute 4m" │
│                                                              │
│   Filter: [Edge ▾] [Category ▾] [Confidence ▾]              │
│   [Recompute now ⚡]                                         │
├─────────────────────────────────────────────────────────────┤
│ DataTable (full width)                                       │
│   Columns:                                                   │
│     - Market (question + category pill)                     │
│     - Computed (relative time)                               │
│     - Our (predicted_prob)                                   │
│     - Mkt (market_prob)                                      │
│     - Edge (signed, colored)                                 │
│     - Conf (bar + %)                                         │
│     - Horizon (hours)                                        │
│     - Model (version string)                                 │
│     - Action (Jump · Copy · Detail)                         │
│                                                              │
│   Click row: opens SignalDetailDialog (modal)               │
└─────────────────────────────────────────────────────────────┘
```

**SignalDetailDialog**：
- 大数字：predicted vs market
- Edge 解释（rationale JSON 可读化）
- Top 5 特征贡献（横向条）
- 历史该模型的胜率
- "Jump to Polymarket" / "Copy" 按钮

---

### 5.4 Copy Trading `/copy`

**布局**（左右两栏）：

```
┌─────────────────────────────────────────────────────────────┐
│ PageHeader                                                   │
│   h1 "Copy Trading"                                          │
│   subtitle "2 active targets · 17 events last 24h"          │
│                                                              │
│   [+ Add target]                                             │
├──────────────────────────┬──────────────────────────────────┤
│ Targets (left, w=320)   │ Events (right, fluid)              │
│ ┌──────────────────────┐ │ ┌──────────────────────────────┐  │
│ │ @quantumorca    [✓]  │ │ │ Filter: [Last 24h ▾]         │  │
│ │ 17 events · +$342    │ │ │                              │  │
│ │ Cap $500 · edge≥5%  │ │ │ Timeline (chronological):    │  │
│ ├──────────────────────┤ │ │ ─ 14:32  q.orca YES @0.58   │  │
│ │ @coldstorage     [✓]  │ │ │     LGD vs Spirit · 120 sh  │  │
│ │ 3 events · +$12      │ │ │ ─ 14:01  q.orca NO  @0.35   │  │
│ │ Cap $200 · edge≥8%  │ │ │     Fed rate cut · 50 sh    │  │
│ ├──────────────────────┤ │ │ ─ 13:42  cs2.eth YES @0.49  │  │
│ │ [+ Add target]      │ │ │     NaVi Map1 · 200 sh      │  │
│ └──────────────────────┘ │ │ ...                          │  │
│                          │ └──────────────────────────────┘  │
└──────────────────────────┴──────────────────────────────────┘
```

**Target 点击 → 展开详情**（inline expand）：
- 完整 events list
- 性能统计：copy 后平均收益
- "Pause mirroring" 开关

---

### 5.5 P&L `/pnl`

**布局**：

```
┌─────────────────────────────────────────────────────────────┐
│ PageHeader                                                   │
│   h1 "P&L"                                                   │
│   subtitle "Equity $12,847 · 30D +$1,247 · Win rate 68.4%"  │
│                                                              │
│   [Range: 7D|30D|90D|All]  [Category ▾]                      │
├─────────────────────────────────────────────────────────────┤
│ PnL Curve (full width, h=300)                                │
│   Stacked area: cumulative P&L + daily bar overlay          │
├─────────────────────────────────────────────────────────────┤
│ Grid (grid-cols-3)                                           │
│   ┌──────────┐ ┌──────────┐ ┌──────────┐                    │
│   │ Football │ │ CS2      │ │ Politics │                    │
│   │ +$624    │ │ +$412    │ │ +$211    │                    │
│   │ 12W 4L   │ │ 8W 5L    │ │ 5W 6L    │                    │
│   └──────────┘ └──────────┘ └──────────┘                    │
├─────────────────────────────────────────────────────────────┤
│ Settled Bets Table                                           │
│   Columns: Market · Side · Size · Entry · Exit · P&L · Date │
│   Sortable by any column                                     │
└─────────────────────────────────────────────────────────────┘
```

---

### 5.6 Model Lab `/lab`

**布局**：

```
┌─────────────────────────────────────────────────────────────┐
│ PageHeader                                                   │
│   h1 "Model Lab"                                             │
│   subtitle "Active: v0.3.2 · Best (90D): v0.3.1 (Brier 0.118)"│
│                                                              │
│   [+ Train new]  [Compare versions]                          │
├─────────────────────────────────────────────────────────────┤
│ Versions Tabs (horizontal)                                   │
│   [v0.3.2 active] [v0.3.1] [v0.3.0] [v0.2.x]                │
├─────────────────────────────────────────────────────────────┤
│ Grid (grid-cols-2)                                           │
│   ┌─────────────────────────┐ ┌─────────────────┐            │
│   │ Performance table        │ │ Calibration     │            │
│   │ Window | N | Brier | W% │ │ overlay (multi) │            │
│   └─────────────────────────┘ └─────────────────┘            │
├─────────────────────────────────────────────────────────────┤
│ Recent Training Jobs                                         │
│   List: job_id, started, duration, status, brier_improvement │
└─────────────────────────────────────────────────────────────┘
```

### 5.9 LLM Performance `/llm-perf`（v0.2 深化）

**布局**（4 个视图 tab）：

```
┌─────────────────────────────────────────────────────────────┐
│ PageHeader                                                   │
│   h1 "LLM Performance"                                        │
│   subtitle "30D · per-provider win rate, Brier, cost"        │
│                                                              │
│   [7D|30D|90D|All]  [All cats|Football|CS2|Politics]         │
├─────────────────────────────────────────────────────────────┤
│ Tabs: [Table] [Heatmap] [Scatter] [Timeseries] [Decisions] │
├─────────────────────────────────────────────────────────────┤
│ Tab: Table (默认)                                            │
│   per-provider table (Claude / GPT / Gemini / DeepSeek /     │
│   Consensus), columns: Win% / Brier / Avg conf / N / Cost    │
├─────────────────────────────────────────────────────────────┤
│ Tab: Heatmap (LLM × Category)                                │
│   行 = LLM, 列 = Category                                    │
│   单元格 = win%，颜色 = 绿/红（深浅表示高低）                │
│                                                              │
│              Football   CS2     Politics                     │
│   GPT-4o    75% ▓▓▓   62% ▓▓   55% ▓                        │
│   Claude 4  68% ▓▓    75% ▓▓▓  62% ▓▓                       │
│   Gemini    58% ▓     48% ·   68% ▓▓                        │
│   DeepSeek  62% ▓▓    58% ▓   55% ▓                         │
│   Consensus 78% ▓▓▓▓  68% ▓▓  62% ▓▓                       │
├─────────────────────────────────────────────────────────────┤
│ Tab: Scatter (per-LLM P&L)                                  │
│   x = 累计 P&L, y = 跟单胜率, 气泡 = 平均 P&L, 颜色 = LLM   │
│   关键: Claude 在右上角(高胜率高 P&L), GPT 中等              │
├─────────────────────────────────────────────────────────────┤
│ Tab: Timeseries                                              │
│   x = 日期(30D), y = 胜率, 多条折线(每 LLM 一条)           │
├─────────────────────────────────────────────────────────────┤
│ Tab: Decisions (用户决策类型胜率)                            │
│   ┌─────────────────────────────┬──────────┐                 │
│   │ 决策类型                     │ 胜率      │                 │
│   ├─────────────────────────────┼──────────┤                 │
│   │ Follow consensus (3/4)     │ 73.5%   │ ← 最高           │
│   │ Follow any LLM             │ 70.1%   │                  │
│   │ Manual (no LLM)             │ 55.0%   │ ← 基准           │
│   │ Override LLM                │ 52.4%   │ ← 反向            │
│   └─────────────────────────────┴──────────┘                 │
└─────────────────────────────────────────────────────────────┘
```

---

### 5.7 Wallets `/wallets`（v0.2）

**布局**：

```
┌─────────────────────────────────────────────────────────────┐
│ PageHeader                                                   │
│   h1 "Wallets"                                               │
│   subtitle "2 wallets · 1 key in OS keychain"                │
│                                                              │
│   [+ Add wallet]                                             │
├─────────────────────────────────────────────────────────────┤
│ Wallets Table                                                │
│   Columns: Address (truncated) · Label · Chain · Type · Keys │
│   Row actions: Edit label · Delete · Import key              │
└─────────────────────────────────────────────────────────────┘
```

**ImportKey Dialog**（安全敏感）：
- Alias 输入框
- 私钥输入框（**通过 Tauri dialog 而非 webview 输入**，避免 devtools 抓到明文）
- 显示警告："Store key only on trusted device"
- Confirm 按钮红色，文字 "I understand, store in OS keychain"

---

### 5.8 Settings `/settings`

**布局**（单列表单）：

```
┌─────────────────────────────────────────────────────────────┐
│ PageHeader                                                   │
│   h1 "Settings"                                              │
├─────────────────────────────────────────────────────────────┤
│ Section: Appearance                                          │
│   Theme: ( ) Dark ( ) Light ( ) Matrix                       │
│   Accent density: [Compact | Default]                        │
├─────────────────────────────────────────────────────────────┤
│ Section: Trading defaults                                    │
│   Edge threshold: [5%]  [slider]                            │
│   Max position size: [$500 USDC]                             │
│   Confirm before placing: [✓]                                │
├─────────────────────────────────────────────────────────────┤
│ Section: Copy trading                                        │
│   Enable mirroring: [off]                                    │
│   Max allocation per target: [$200 USDC]                     │
│   Pause on drawdown >20%: [✓]                                │
├─────────────────────────────────────────────────────────────┤
│ Section: Notifications                                       │
│   Toast: [✓]  System notifications: [ ]                     │
├─────────────────────────────────────────────────────────────┤
│ Section: Data                                                │
│   [Clear local cache]                                        │
│   [Export all data as JSON]                                  │
│   Local DB path: ~/Library/Application Support/.../         │
└─────────────────────────────────────────────────────────────┘
```

---

## 6. 交互模式

### 6.1 关键弹窗

| Dialog | 触发 | 内容 | 二次确认 |
|---|---|---|---|
| Place Jump | 点 SignalRow "Jump" | "Open in browser?" + URL preview | 无（用户主动） |
| Place Signed | 点 SignalRow "Copy" | Size / Side / Price 确认 + "Sign & place" | 有（Confirm 按钮） |
| Import Key | Wallets 页 "Import" | Alias + 私钥 + 警告 | 有（红色 Confirm） |
| Delete Wallet | 行操作 "Delete" | "Permanent? Also delete key?" | 有 |
| Settle Bet | 行操作 "Settle now" | "Use current market state" | 有 |
| Clear Cache | Settings | "All local data will be removed" | 有（双重） |

### 6.2 键盘快捷键

| 快捷键 | 动作 |
|---|---|
| `⌘K` / `Ctrl+K` | 打开 Command Palette |
| `⌘⇧L` / `Ctrl+Shift+L` | 循环切换主题 |
| `⌘B` / `Ctrl+B` | 折叠/展开 Sidebar |
| `⌘1-8` | 跳到对应路由 |
| `ESC` | 关闭最上层弹窗 |

### 6.3 Command Palette（⌘K）

**Items**（按类型分组）：
- **导航**：Dashboard, Markets, Signals, Copy, P&L, Lab, Wallets, Settings
- **动作**：Sync markets, Recompute signals, Add wallet, Export data
- **设置**：Switch to Dark/Light/Matrix
- **跳转**：Open current market in Polymarket

匹配规则：fuzzy search on label + keywords。

---

## 7. 加载 / 错误 / 空态

### 加载（Skeleton）

所有列表/卡片用 skeleton：
- 高度与真实内容一致
- shimmer 动画（背景从 surface-2 渐变到 surface）
- 80ms 出现，最长 200ms 后必须给数据或切到 error

### 错误

- **Toast**（短暂错误，3s 自动消失）：sync 失败、网络断连
- **Inline banner**（持续错误）：keyring 访问被拒、DB 连接丢失
- **Empty state**（无错误但无数据）：插画 + 解释 + 主操作按钮

### 空态文案

| 场景 | 文案 | 主操作 |
|---|---|---|
| 无 signals | "No signals yet — first recompute in 4 minutes" | "Recompute now" |
| 无 bets | "No bets placed — try Jump on a high-edge signal" | "View signals" |
| 无 wallets | "Add a wallet to start trading" | "Add wallet" |
| 无 copy targets | "Track a smart trader to get started" | "Add target" |

---

## 8. 响应式（v0.3+）

v0.1 仅支持桌面窗口（≥1200×720）。v0.3 规划：
- ≥1440：3 列布局（标准）
- 1200-1439：2 列布局
- <1200：单列堆叠 + Sidebar 折叠为抽屉

不在 v0.1 范围。

---

## 变更日志

- **v1.2** (2026-06-16) — Dashboard 加 **Today's Brief** 区块（M12 每日看板）；LLM Performance 页面深化为 5 个 tab（Table / Heatmap / Scatter / Timeseries / Decisions）。
- **v1.1** (2026-06-16) — Token 调整：dark 改为 Cursor / VS Code Dark+ 风格（`#1E1E1E` 编辑区、`#181818` sidebar、`#007ACC` VS Code 蓝、`#4EC9B0`/`#F48771` 柔和涨/跌色）；matrix 改为 OpenAI Codex CLI 风格（真黑 `#0A0A0A`、OpenAI 绿 `#10A37F`、border 极弱 `#1F1F1F`、去掉卡片阴影）。
- **v1.0** (2026-06-16) — 初版。基于已实现 AppShell + Dashboard，扩展到 8 个页面、24 个组件、6 条设计原则、完整主题 token 矩阵。