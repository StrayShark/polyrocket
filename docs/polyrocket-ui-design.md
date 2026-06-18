# polyrocket — UI 设计规范 (UID)

> 版本：v2.1 · 2026-06-16
> 配套：[`polyrocket-modules.md`](./polyrocket-modules.md)（数据来源） · [`polyrocket-flows.md`](./polyrocket-flows.md)（交互流程） · [`polyrocket-llm-analysis.md`](./polyrocket-llm-analysis.md)（M10） · [`polyrocket-llm-management.md`](./polyrocket-llm-management.md)（M11）
> 设计基线：Cursor IDE（dark / light） + 配色变体（matrix）
> 强约束：[`polyradar-dev-governance.md §11`](../polyradar-dev-governance.md) — 三主题仅配色差异
>
> 覆盖范围：18 个页面 / 32 个组件 / 3 个主题 / 9 个状态 / 54 张 PNG 截图（`docs/previews/{dark,light,matrix}/*.png`） / 完整设计 tokens / 完整组件状态机 / 动效 / a11y / 图标 / 数据可视化 / 错误边界 / i18n / Empty-Loading-Error 视觉规范
> v2.1 新增：54 张 PNG 截图 + Feature Coverage Matrix 验证 17 个 module 全部覆盖

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

**18 页面 × 3 主题（dark / light / matrix）= 54 PNG 截图**
截图存于 `docs/previews/{theme}/{page}.png`，由 `python3 scripts/snapshot_pages.py` 一键重生成。

| 路由 | 页面 | 用途 | 主要模块 | PNG（dark） | 关键组件 |
|---|---|---|---|---|---|
| `/dashboard` | Dashboard | 总览：KPI + Equity + Today's Brief | M1-M6 + M12 | ![](previews/dark/dashboard.png) | KpiCard × 4, EquityCurve, CalibrationChart, BriefList |
| `/markets` | Markets | 市场列表 | M1 | ![](previews/dark/markets.png) | MarketsTable (sortable) + CategorySegmented |
| `/market-detail` | Market Detail | 单市场深视图（行情+orderbook+LLM+signals） | M1 + M2 + M10 | ![](previews/dark/market-detail.png) | PriceChart, OrderbookTable, BetForm, ConsensusCard, SignalsList |
| `/signals` | Signals | 全部活跃信号 | M2 | ![](previews/dark/signals.png) | SignalsTable (full) + EdgeChart |
| `/copy` | Copy Trading | 目标地址监控 + 事件流 | M5 | ![](previews/dark/copy.png) | CopyTargetsList, CopyEventsTimeline, MirrorStrategyForm |
| `/pnl` | P&L | 聚合统计 + 类别细分 | M3 + M6 | ![](previews/dark/pnl.png) | PnLCurveChart, CategoryBreakdown, BetsHistoryTable |
| `/history` | Trade History | 全部 bet 明细 | M3 (detail) | ![](previews/dark/history.png) | KPI × 3, BetsTable (10-col) |
| `/lab` | Model Lab | 模型版本管理 | M7 | ![](previews/dark/lab.png) | ModelVersionsList, PerformanceComparison, CalibrationOverlay |
| `/analysis` | Analysis | 多 LLM 并行 + 共识 | M10 | ![](previews/dark/analysis.png) | ContextPanel, 4× ProviderCard, ConsensusCard, DecisionBar |
| `/llm-perf` | LLM Performance | 5 tab 胜率 / Brier / scatter / timeseries / decisions | M10 stats | ![](previews/dark/llm-perf.png) | 5 tab (Table / Heatmap / Scatter / Timeseries / Decisions) |
| `/llm-mgmt` | LLM Management | Provider + key + 连通性 + 流量 + 异常 | M11 | ![](previews/dark/llm-mgmt.png) | SecretsBanner, ProviderList (collapsible), TrafficCard, WinRateTable, KeyModal |
| `/brief` | Daily Brief | 每日 top 8 + 评分公式权重 + 缓存 | M12 | ![](previews/dark/brief.png) | WeightsPanel, Top8List (with breakdown), TomorrowPreview |
| `/wallets` | Wallets | 钱包 + PM CLOB 凭证 | M4 + M11 (PM creds) | ![](previews/dark/wallets.png) | PMCredentialsTable, WalletsTable, KeyringStatusBanner |
| `/notifications` | Notifications | 集中 toast + 系统事件 | X2 | ![](previews/dark/notifications.png) | KindFilter (5+ 段), EventTable |
| `/audit` | Audit Log | 所有写操作历史 | X1 | ![](previews/dark/audit.png) | FilterBar, EventTable (6-col) |
| `/help` | Help & Docs | 5 spec docs 链接 + 快捷键 + troubleshooting | M9 (docs) | ![](previews/dark/help.png) | QuickStartList, ConceptsList, KbdTable, SpecDocsList, TroubleshootingDetails |
| `/settings` | Preferences | 主题 + 阈值 + 通知 | M9 | ![](previews/dark/settings.png) | ThemeSwitch, TradingDefaults, CopyTrading, DataExport |
| `/onboarding` | Onboarding (首启) | 4 步引导 | M13 | ![](previews/dark/onboarding.png) | ProgressDots, 4-step wizard, KeyForm |

> 注：`/onboarding` 只在 `localStorage.polyrocket.first-run-done !== '1'` 时自动进入；用户主动从 user menu 也能进（v0.3+）。

---

## 3.1 Feature Coverage Matrix

**目标**：验证 UI spec 覆盖了 `polyrocket-modules.md` 里声明的所有 17 个 module（M1-M13 + X1 + X2）。

| Module | 描述（缩写） | 主页面 | 次级入口 | 测试可见？ | 状态 |
|---|---|---|---|---|---|
| M1 Markets | 市场元数据 | /markets, /market-detail | Dashboard "Today's Brief" | ✅ | v0.1 |
| M2 Signals | 信号生成 | /signals | /market-detail "Signals" | ✅ | v0.1 |
| M3 Bets | 下单 / 持仓 / 结算 | /history, /pnl | /market-detail "Place a bet" | ✅ | v0.1 + v0.2 增强 |
| M4 Wallets | 钱包 + keyring | /wallets | /llm-mgmt 间接 | ✅ | v0.1 + v0.2 keychain |
| M5 CopyTrading | 目标监控 + 镜像 | /copy | — | ✅ | v0.1 |
| M6 PnL | 聚合统计 | /pnl | Dashboard KPI | ✅ | v0.1 |
| M7 ModelLab | 模型版本管理 | /lab | — | ✅ | v0.1 |
| M8 Dashboard | 跨模块汇总 | /dashboard | — | ✅ | v0.1 |
| M9 Settings | 主题 / 阈值 / 通知 | /settings | user menu | ✅ | v0.1 |
| X1 AuditLog | 写操作追溯 | /audit | — | ✅ | v0.1 |
| X2 Notifications | toast / 系统通知 | /notifications | Topbar 铃铛 | ✅ | v0.1 |
| M10 LLM Analysis | 多 LLM 并行分析 | /analysis, /llm-perf | /market-detail consensus | ✅ | v0.2 |
| M11 LLM Management | Provider/Key + 流量 + 健康 | /llm-mgmt | /wallets (PM CLOB) | ✅ | v0.2 + v1.1 keychain + v1.2 scheduler |
| M12 Daily Brief | 每日 top 8 | /brief | Dashboard "Today's Brief" | ✅ | v0.2 |
| M13 Onboarding | 首启 4 步 | /onboarding | first-run auto-trigger | ✅ | v0.2 |

**100% 覆盖** — 每个 module 都有至少 1 个主页面 + 1 个次级入口（嵌入到 drill-down 视图）。

**还**加** 2 个辅助页面**（不属于 module，是横切 + 系统）：
- `/help` — Help & Docs (M9 docs 子集)
- `/audit` — Audit Log (X1，**也**是独立入口)

**7 大 UI 状态组合**（18 页面 × 3 主题 = 54 PNG 都覆盖）：
- 正常态（每页主态）
- 加载态（skeleton — `uiSkeleton()` helper）
- 空态（empty — `uiEmpty()` helper）
- 错误态（error — `uiError()` helper）
- 弹窗态（modal — Add key / Add wallet / Confirm delete）
- 折叠态（sidebar collapsed / provider row expanded）
- 主题态（dark / light / matrix）

---

## 3.2 截图重生成

```bash
# 一次性生成全部 54 张 PNG
python3 scripts/snapshot_pages.py

# 输出
# docs/previews/
#   dark/   <18 pages>.png
#   light/  <18 pages>.png
#   matrix/ <18 pages>.png
```

机制：
- 1440×900 viewport, device_scale_factor=2 (Retina)
- seed.html 临时文件写入 localStorage (theme + first-run-done + sidebar.collapsed) → location.replace 到 `prototype.html#<page>`
- `wait_for_function(data-theme === '<theme>')` 确保 paint 完成
- `wait_for_timeout(300ms)` 让 lucide icon font + 异步 paint settle
- `page.screenshot(full_page=True)` — 含 sidebar 完整长度
- 54 PNG, 54 distinct MD5（**不**会撞缓存坑）

**MD5 不重复保证**：
- 每次 `goto` 都通过新的 `seed.html` 写 localStorage（值不同 → 渲染不同）
- 不用 mockito / 不用 cache-buster query — file:// 协议天然无缓存

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

## 5.10 Analysis `/analysis`（M10 v0.2 新增）

**目的**：单市场多 LLM 并行分析触发 + 结果聚合 + 用户决策记录入口。

**布局**：3 列布局
- **左 1/3** — 市场快照（YES/NO 当前价、24h 走势 mini chart、关键时间节点）
- **中 1/3** — 4 LLM 并行结果卡片（每张卡顶部 provider 名 + latency + 完整 prompt 折叠器 + 推荐 side + 置信度 + 推理摘要 + 关键依据列表 + 反方观点）
- **右 1/3** — 共识聚合（median prob、edge、置信区间、用户决策 A/B 按钮 + 笔记输入框）

**关键交互**：
- 任何 LLM 卡片可点击展开 → 显示完整 prompt + 完整响应 + token 用量
- 共识值用**加权中位数**算法（provider weight 来自 M11 统计：30D Brier 越低权重越大）
- 用户决策（Follow A / Follow B / Skip）写 `llm_decisions` 表
- 「Save decision & close」按钮 → toast 成功 → 跳 `market-detail`

**Loading 状态**：4 个 LLM 卡片各自独立 loading，已完成的先渲染（用 partial streaming）
**Error 状态**：单 provider fail 不阻塞其余 3 个，fail 的卡片显示 ✗ + 错误码 + retry 按钮
**Empty 状态**：尚未选市场时显示 "Pick a market to analyze" + 跳 `/markets` CTA

---

## 5.11 LLM Management `/llm-mgmt`（M11 v0.2 新增）

**目的**：provider / key 增删改 + 连通性测试 + 流量监控 + 健康 + 配额 + 异常告警。

**布局**：单列可折叠行
- **顶部 banner** — `secrets_status` 红色 chip 列出 missing key（"1 key missing · azure-proxy / key-1"）+ Add key 按钮
- **Provider list**（5 行）— 每行可展开：身份 / 配额条 / 健康摘要 / key 列表 / 探针历史入口
- **Traffic summary**（2 列卡片）— 24h calls / success rate / cost / p50 + 异常检测
- **Win rate by confidence band**（表格）— 5 维 × 4 视图
- **Wallets & PM CLOB credentials**（4 行）— 3 PM + 1 wallet

**关键交互**：
- 每行 provider 的 `Test` 按钮 → 调 `llm_test_connectivity` → 实时显示结果
- 卡片内 `Add key` 按钮 → 弹窗（含 provider 选择 + alias + password + 实时 keyring alias 预览）
- 卡片内 `Remove key` 按钮 → 确认对话框（"This will also remove the secret from your OS keychain"）
- 顶部 banner chip 跳对应 provider 行的 `Add key` 弹窗
- 行右侧 `Update` 按钮 → 同一弹窗，调 `llm_key_set_secret` (rotate)

**关键弹窗 — Add / Update API key**：
- provider 下拉（4 LLM + 3 PM + 1 wallet = 9 项）
- alias 文本框（默认 `prod-1`，wallet 隐藏此字段）
- password 框（type=password，提交后立即清空 DOM 值）
- 实时 keyring alias 预览（`llm/openai/prod-1` / `polyrocket/pm/api` / `polyrocket/wallet/primary`）
- shield-check icon + 说明文字 "Written to <alias> in your OS keychain"
- PM 模式下额外显示 host / chain_id 字段
- wallet 模式下显示 64-hex 验证提示

**Empty 状态**：0 provider 时显示 "No providers yet — add one to get started" + Add 按钮
**Error 状态**：keyring 写失败 → toast "OS keychain unavailable, retry or restart"

---

## 5.12 Daily Brief `/brief`（M12 v0.2 新增）

**目的**：每天 00:00 UTC 自动分析所有市场，挑选 top N + 用户兴趣度加权。

**布局**：单列
- **顶部** — page header + 当前 scoring 权重展开器（6 维：edge / confidence / consensus / time decay / user interest / cost penalty）+ Refresh now 按钮
- **Today 区块** — top 8 卡片网格（每张：market q + category pill + 评分 + 推荐 side + 4 LLM 共识 chip + "Open" + "Dismiss" + "Snooze 24h"）
- **Tomorrow preview** — 折叠区，列明日将开赛/到期市场
- **Recent dismissals** — 已 dismiss 的市场 + 撤销链接（48h 内可恢复）

**关键交互**：
- 评分公式权重可在 UI 调整（实时保存到 `user_brief_prefs`）
- "Run on this market" 按钮 → 调 `llm_analyze` 立即跑（不等 cron）
- "Dismiss" → `daily_brief_dismiss` IPC，写 `brief_dismissed_at` 字段
- "Snooze 24h" → 24h 内不再出现，after that 重新评分

**Empty 状态**：0 market above threshold → "No high-edge markets today" + 评分阈值调整链接
**Error 状态**：cache 过期 + refresh 失败 → "Last refresh failed, retry now"

---

## 5.13 Landing `/welcome`（v0.53 重做 — 首启 6 步引导）

> 详细设计：[`polyrocket-landing-design.md`](./polyrocket-landing-design.md)。
> 路径从 `/onboarding` 改 `/welcome`（与 VS Code / Postman / Figma 桌面惯例对齐）。Storage key 从 `polyrocket.onboarding` 改 `polyrocket.welcome`，旧 key 启动时一次性迁移。

**目的**：首次启动引导用户完成**真实**最小配置：选存储路径 + 选主题 + 至少 1 个 LLM key + Polymarket 凭据。**不是**4 步描述性，**是** 6 步真操作（每步 Next 触发真实 IPC 副作用）。

**步骤**：

| # | 步骤 | UI 元素 | 真实副作用（IPC） |
|---|---|---|---|
| 1 | Welcome | 语言选择 + hero + Get started | `setLocale` |
| 2 | Storage path | default 单选 / custom input + Browse 按钮（v0.54+） | `setStoragePath`（重启生效） |
| 3 | Theme | 3 主题大预览卡 + 实时换色 | `setTheme` |
| 4 | LLM providers | provider 网格 + alias + paste key + Test connectivity | `llm_provider_upsert` + `llm_key_set_secret` + `llm_test_connectivity` |
| 5 | Polymarket | CLOB 三件套 + 可选 wallet pk | `llm_pm_set_credentials` + `polyrocket_wallet_set_pk` |
| 6 | Finish | 汇总 + Finish btn | `set welcome.done=true` + navigate /dashboard |

**进度指示**：顶部 6 dot + 连接线（`Step 3 of 6`）

**关键交互**：
- 每步「Back / Next」按钮
- 任一步可「Skip for now」直接进 dashboard（不写 `welcome.done`）
- Step 4 连通性测试失败时，错误 inline 显示 + 提供 3 选项（继续 / 重试 / 跳过）
- Step 2 选 custom path 后提示"This change takes effect on next launch" + [Restart now] 按钮（v0.53+ 暂未接 tauri-plugin-dialog，文本输入）
- Step 6 Finish 后下次启动不再进 /welcome

**触发逻辑**（main.tsx 启动时）：
```ts
const isFirstRun = localStorage.getItem('polyrocket.welcome.done') !== '1';
useEffect(() => {
  if (isFirstRun) {
    navigate('/welcome', { replace: true });
    return;
  }
  // 强制跳转后，检查半配置状态
  invoke('secrets_status').then((s) => {
    const needsLlm = s.llm_keys.length === 0;
    const needsPm = !s.polymarket.find((x) => x.kind === 'pm_api')?.configured;
    if (needsLlm || needsPm) {
      // 不强制跳转 — 在 Dashboard 顶部显示横幅
      queryClient.setQueryData(['welcome-banner'], { needsLlm, needsPm });
    }
  });
}, []);
```

**Dashboard 半配置横幅**：
```
┌─────────────────────────────────────────────────┐
│ ⚠ Setup incomplete — 2 of 4 secrets missing     │
│   • LLM providers: 0 keys configured            │
│   • Polymarket CLOB: not configured              │
│                                       [ Complete ] │
└─────────────────────────────────────────────────┘
```
"Complete" 按钮 → 跳 `/welcome` 的最后未完成步骤（不重置已完成的）

**Storage path 步骤关键文案**：
- "This folder will hold: polyrocket.db (your bets, signals, models, audit log) · logs/ (Tauri + scheduler) · logs/telemetry/ (opt-in lifecycle events). NO secrets — those live in OS keyring only."

**keyring alias 实时显示**（每步）：
- LLM: `polyrocket/llm/<provider_id>/<alias>`
- Polymarket: `polyrocket/pm/{api,secret,passphrase}`
- Wallet: `polyrocket/wallet/<alias>`

**v0.53 不做**（v0.54+ candidate）：
- tauri-plugin-dialog → 用 `<input type="text">` 替代 "Browse..." 按钮
- 多 wallet alias 引导（留 /wallets 页面）
- Storage path 迁移工具（"copy existing db to new path"）
- LLM provider `base_url` 自定义配置（CUSTOM provider 默认 OpenAI-compatible）

---

## 5.14 Market Detail `/markets/:id`（v2.0 新增 — 钻取视图）

**目的**：单个市场的深视图，从 list 页点击行进入。

**布局**：2 列
- **左 1/2** — 大尺寸 price chart（200px 高）+ YES/NO 当前价（32px 字号）+ orderbook 表（3 档 bid / 3 档 ask + spread / midpoint）+ market 元信息（resolution source、category、volume 24h / total）
- **右 1/2** — 3 卡片堆叠：
  1. **Place a bet**（mode A/B 选择 + size 输入 + 实时 cost 计算 + Open on Polymarket 主按钮）
  2. **Latest LLM consensus**（YES 68% + edge +3.5% + 4 provider 明细 + "view full analysis →" 跳 `/analysis`）
  3. **Signals on this market**（BTC momentum / order book imbalance / funding rate / F&G 指数 4 条）

**关键交互**：
- Mode A 切换到 Mode B 时显示 wallet 选择下拉（用哪个地址签）
- price chart hover 显示十字光标 + tooltip（时间 + 价 + 量）
- orderbook 行 hover 高亮（同一价位的 bid+ask 配对）
- 「Run LLM analysis」按钮 → 跳 `/analysis` 并预填 market

**Loading 状态**：orderbook 实时刷新，每 10s 一帧（loading 期间左侧 chart 灰显 + 右下角 spinner）
**Error 状态**：clob.polymarket.com 不可达 → 全卡片显示 error_state + retry 按钮
**Empty 状态**：edge 太小（<2%）→ 中卡片显示 "No LLM consensus yet — edge too small" + Run 按钮

---

## 5.15 Notifications Center `/notifications`（v2.0 新增 — X2 横切）

**目的**：所有 toast 历史 + 关键事件集中地（last 7 days 内存缓存）。

**布局**：单列
- **顶部** — page header + Mark all read + Preferences
- **分类 segmented** — All / Unread / Trades / LLM / System / Signals
- **表格** — 5 列：icon + event + detail + when + status (new/read) + chevron

**事件 kind 映射**（icon + 颜色）：
- `success` → check-circle-2 + var(--bull)
- `warn` → alert-triangle + var(--warning)
- `error` → x-circle + var(--bear)
- `info` → info + var(--accent)

**关键交互**：
- 行点击跳对应 route（`llm.key.upsert` → /llm-mgmt，`brief.refresh` → /brief）
- Preferences 弹窗：订阅哪些 kind、每 kind 频率上限、是否系统通知、声音
- 「Mark all read」→ 改所有 unread 为 read

**Empty 状态**：0 unread + 0 recent → "All caught up ✓" + illustration
**Loading 状态**：初次加载 5 行 skeleton

---

## 5.16 Trade History `/history`（v2.0 新增 — M3 增强）

**目的**：所有 bet 的明细 + 累计统计（区别于 `/pnl` 的 KPI 概览）。

**布局**：单列
- **顶部 3 KPI 卡** — Realized P&L 90D / Win rate / Avg hold time
- **筛选器** — date range / side / mode / status / llm_assisted
- **明细表** — 10 列：placed / market / side / size / @price / filled / status / P&L / mode / llm assist

**关键交互**：
- 排序：所有列可点 header 切换 asc/desc
- 行 hover 显示 mini sparkline（最近 7d price）
- 「Open market」 → /market-detail
- 「Re-analyze」 → 跳 /analysis 重新跑
- 「Export CSV」 → 调 `llm_stats_export` 同款 CSV writer

---

## 5.17 Audit Log `/audit`（v2.0 新增 — X1 横切）

**目的**：所有写操作的不可篡改记录（X1 模块的 viewer）。

**布局**：单列
- **顶部** — page header + Export JSONL + Filter
- **筛选器** — action pattern (e.g. `llm.*`)、target/actor、result、date range
- **明细表** — 6 列：timestamp / actor / action / target / payload (truncated) / result
- **分页** — Showing 10 of 1,287 + Prev/Next

**关键交互**：
- payload hover 显示完整 JSON（不展开，monospace tooltip）
- actor 分类颜色：user = blue、system = gray、background = purple
- Export JSONL 导出 last 90D（合规/审计用途）
- 无 filter 时显示最近 100 条

**Empty 状态**：filter 命中 0 条 → "No events match" + clear filter 按钮

---

## 5.18 Help & Docs `/help`（v2.0 新增 — M9 增强）

**目的**：在 app 内嵌入完整文档（不离开 app）。

**布局**：2 列
- **左** — Quick start（5 步列表）+ Concepts（6 链接）+ Keyboard shortcuts 表
- **右** — Specification docs 列表（5 个 .md 文件 + 版本号）+ Troubleshooting（4 折叠面板）+ Version & build 表格

**关键交互**：
- 所有 docs 链接打开 modal 渲染对应 .md（marked.js 客户端渲染）
- Troubleshooting 折叠面板（`<details>` 原生）
- Build info 一键复制按钮

---

## 9. Design Tokens（深度规范）

### 9.1 Spacing 间距尺度

| token | px | 用途 |
|---|---|---|
| `--space-0` | 0 | 清除默认 |
| `--space-1` | 2 | 文字行内微调 |
| `--space-2` | 4 | pill 内边距、icon-text gap |
| `--space-3` | 6 | 小型 input 垂直内边距 |
| `--space-4` | 8 | 通用 gap（grid cell、flex item） |
| `--space-5` | 12 | 中型 gap（卡片内 row 间） |
| `--space-6` | 16 | 卡片 padding、grid column gap |
| `--space-7` | 20 | 卡片大 padding |
| `--space-8` | 24 | page section 间 gap |
| `--space-9` | 32 | 大区块间 gap |
| `--space-10` | 48 | page top margin（hero 类） |
| `--space-11` | 64 | 极端留白（empty state 上下） |

**用法规则**：
- 卡片 padding 用 `--space-6` (16) 或 `--space-7` (20)
- 表格行高由内容决定，**不**额外加 padding
- 同一行内 icon 与文字 gap = `--space-2` (4)
- section 间 gap = `--space-8` (24)

### 9.2 Radius 圆角

| token | px | 用途 |
|---|---|---|
| `--radius-0` | 0 | 表格 cell、code block |
| `--radius-1` | 3 | kbd、mini badge |
| `--radius-2` | 4 | 按钮、input、pill、progress bar |
| `--radius-3` | 6 | card（浅视觉） |
| `--radius-4` | 8 | card（默认）、modal |
| `--radius-5` | 12 | 大型 modal、empty state 容器 |
| `--radius-full` | 9999 | 头像、toggle thumb、dot |

**强约束**：matrix 主题下所有 radius 减 2（kbd 不变仍 3）；dark/light 不变。

### 9.3 Shadow 阴影

| token | value | 用途 |
|---|---|---|
| `--shadow-0` | none | 默认状态（**所有**卡片默认无阴影，matrix 强制） |
| `--shadow-1` | `0 1px 2px rgba(0,0,0,0.06)` | row hover 微提升 |
| `--shadow-2` | `0 2px 6px rgba(0,0,0,0.08)` | popover、tooltip |
| `--shadow-3` | `0 4px 12px rgba(0,0,0,0.12)` | modal、command palette |
| `--shadow-overlay` | `0 8px 32px rgba(0,0,0,0.4)` | modal-overlay（仅 dark/light） |

**强约束**：matrix 主题下 `--shadow-0` → 所有 shadow token 重写为 `none`（Codex 风格无阴影）。
**强约束**：dark 主题阴影透明度上限 0.4；light 主题 0.12。

### 9.4 Motion 动效

| token | value | 用途 |
|---|---|---|
| `--duration-instant` | 60ms | hover 状态切换 |
| `--duration-fast` | 120ms | button active、color shift |
| `--duration-normal` | 200ms | 弹窗、抽屉、tab 切换 |
| `--duration-slow` | 320ms | 路由切换、页面进入 |
| `--duration-theme` | 240ms | 主题切换（color/border/background） |
| `--ease-out` | `cubic-bezier(0.16, 1, 0.3, 1)` | 默认出场（gentle landing） |
| `--ease-in-out` | `cubic-bezier(0.4, 0, 0.2, 1)` | 状态机切换 |
| `--ease-spring` | `cubic-bezier(0.34, 1.56, 0.64, 1)` | 弹性（toast 入场、toggle thumb） |

**强约束**：
- 所有 transition 必须 ≤ 320ms（超过视为「慢」，需 UX 评审）
- 用户开启 `prefers-reduced-motion: reduce` 时全部 `duration` → `0ms`，仅保留 opacity
- 主题切换是唯一可用 240ms 的全局 transition；其余瞬时切换

### 9.5 Z-index 分层

| token | value | 用途 |
|---|---|---|
| `--z-base` | 0 | 默认 |
| `--z-dropdown` | 50 | 下拉菜单、popover |
| `--z-sidebar` | 60 | sidebar（始终在内容之上） |
| `--z-modal-overlay` | 200 | modal 遮罩 |
| `--z-modal` | 210 | modal 内容 |
| `--z-toast` | 300 | toast |
| `--z-tooltip` | 400 | tooltip（最高，盖 toast） |
| `--z-drag-overlay` | 500 | drag preview |

**强约束**：sidebar 永远在内容之上，modal 永远在 sidebar 之上。

---

## 10. 组件状态机

每个组件必须显式定义：**6 个基础状态** + **业务专属状态**。

### 10.1 Button 状态机

| 状态 | 视觉 | 触发 | 行为 |
|---|---|---|---|
| `default` | bg=var(--surface-2), fg=var(--fg) | 初始 | 可点击 |
| `hover` | bg=var(--surface-hover), 阴影 --shadow-1 | 鼠标进入 | — |
| `active` | bg=var(--surface-2), 1px inset shadow | mousedown | 立即触发 click |
| `focus-visible` | outline 2px var(--accent), offset 2px | 键盘 Tab | 同 default |
| `disabled` | opacity 0.4, cursor not-allowed | prop disabled=true / 权限不足 | 不响应 click |
| `loading` | 内嵌 spinner (16px) + 文案 "Loading…" | async in flight | 不可重复点击；防抖 200ms |

**变体**：`primary` (accent 色) / `danger` (bear 色) / `ghost` (无 bg) / `icon` (32×32 圆角方形)

### 10.2 Input 状态机

| 状态 | 视觉 |
|---|---|
| `default` | bg=var(--bg), border 1px var(--border) |
| `hover` | border var(--border-strong) |
| `focus` | border var(--accent), 无 outline（border 自身变化） |
| `error` | border var(--bear) + 下方 11px error 文字 |
| `disabled` | bg=var(--surface-2), opacity 0.5 |
| `readonly` | bg=transparent, border dashed var(--border) |

### 10.3 Card 状态机

| 状态 | 视觉 |
|---|---|
| `default` | bg=var(--surface), border 1px var(--border), radius-4, **无阴影** |
| `hover` (可点击) | bg=var(--surface-hover), shadow-1 |
| `selected` | border 2px var(--accent), 内部 -1px 补偿避免 layout shift |
| `loading` | 内部替换为 skeleton |
| `error` | border var(--bear), bg=rgba(248,81,73,0.04) |

### 10.4 Toggle 状态机

| 状态 | 视觉 |
|---|---|
| `off` | bg=var(--surface-2), thumb left 2px |
| `on` | bg=var(--accent), thumb right 2px, ease-spring 240ms |
| `disabled-on` | opacity 0.4 |
| `disabled-off` | opacity 0.4 |

### 10.5 Modal 状态机

| 状态 | 视觉 |
|---|---|
| `opening` | overlay opacity 0→1 (200ms), modal scale 0.96→1 (240ms ease-out) |
| `open` | overlay opacity 1, modal scale 1 |
| `closing` | overlay opacity 1→0 (160ms), modal scale 1→0.96 (160ms) |
| `closed` | display: none, focus trap 释放 |

**强约束**：
- 打开时焦点 trap 在 modal 内
- ESC 关闭
- 关闭后焦点恢复到打开前元素
- 不可被 toast 覆盖（z-index: modal=210 < toast=300，但 toast 仍可盖 modal 内容；语义上「modal 阻塞其他 UI」）

### 10.6 Toast 状态机

| 状态 | 视觉 |
|---|---|
| `entering` | translateY -4px → 0, opacity 0 → 1 (240ms ease-spring) |
| `visible` | 停留 3.5s |
| `leaving` | opacity 1 → 0, translateY 0 → -4px (400ms ease-out) |
| `gone` | DOM removed |

**kind**：`success` / `warn` / `error` / `info` — 各自 border-left 颜色（var(--bull/warning/bear/accent)）
**位置**：右上角堆叠，最多同时 3 条（超出排队）
**强约束**：hover 暂停 leave timer

---

## 11. 动效（Easing & Duration 规范）

### 11.1 入场动效

| 元素 | duration | easing | 视觉 |
|---|---|---|---|
| Page enter | 240ms | ease-out | opacity 0→1, translateY 4px→0 |
| Modal | 240ms | ease-out | scale 0.96→1, opacity 0→1 |
| Toast | 240ms | ease-spring | translateY -4px→0, opacity 0→1 |
| Dropdown | 120ms | ease-out | opacity 0→1, translateY -2px→0 |
| Tab 切换 | 160ms | ease-in-out | content fade-cross |

### 11.2 出场动效

| 元素 | duration | easing |
|---|---|---|
| Page leave | 160ms | ease-in |
| Modal | 160ms | ease-in (scale 1→0.96) |
| Toast | 400ms | ease-out (fade-up) |
| Dropdown | 100ms | ease-in |

### 11.3 交互反馈

| 触发 | 视觉 | duration |
|---|---|---|
| Button click | inset shadow 1px | 60ms |
| Card hover | bg lighten 4%, shadow-1 | 120ms |
| Toggle flip | thumb position | 240ms spring |
| Input focus | border color | 120ms |
| Sidebar collapse | width 220→52 | 200ms ease-out |

### 11.4 主题切换

- 所有 `--color-*` `--border-*` `--surface-*` token 在 240ms 内 transition
- **不在此 transition 范围内的**：radius、shadow、font、icon、layout
- 用户选择主题后立即 apply，**不**给首次切换加 240ms（避免「双闪」）

### 11.5 prefers-reduced-motion 兼容

```css
@media (prefers-reduced-motion: reduce) {
  *, *::before, *::after {
    animation-duration: 0.01ms !important;
    animation-iteration-count: 1 !important;
    transition-duration: 0.01ms !important;
  }
}
```

---

## 12. 可访问性（A11Y）

### 12.1 键盘导航

| 键 | 行为 |
|---|---|
| `Tab` | 下一个可聚焦元素（顺序 = DOM 顺序，**不**改） |
| `Shift+Tab` | 上一个 |
| `Enter` / `Space` | 激活当前 focus 的 button / toggle / link |
| `Esc` | 关闭顶层 modal / dropdown / popover |
| `↑` / `↓` | 在 listbox / menu / segmented control 内移动 |
| `Home` / `End` | 跳到 listbox 首 / 末 |
| `⌘1..9` | 跳到 sidebar 第 n 个 item |
| `⌘B` | toggle sidebar |
| `⌘K` | command palette |
| `⌘R` | 刷新当前 page 数据 |
| `?` | show shortcuts modal |

**focus ring**：所有可聚焦元素必须有 `:focus-visible` 视觉：2px var(--accent) outline, offset 2px, **不**用 box-shadow（避免被 border-radius 切）。

### 12.2 颜色对比度

| 元素 | 最低对比度 | 备注 |
|---|---|---|
| 正文 fg on bg | 4.5:1 | WCAG AA |
| 大字号（≥18px / 14px bold） | 3:1 | WCAG AA |
| muted/secondary fg on bg | 4.5:1 | 不要省 |
| border（非装饰） | 3:1 | 用于区分卡片边界 |
| bull / bear / warning / accent | 4.5:1 对应 bg | 用作状态色时 |

**matrix 主题特例**：`#10A37F` (accent) on `#0A0A0A` (bg) = 5.4:1 ✓；正文 `#D4D4D4` on `#0A0A0A` = 14.8:1 ✓。

### 12.3 屏幕阅读器

- 所有 icon-only button 必须有 `aria-label`
- 所有图片（chart、avatar）有 `alt`
- modal 打开时焦点 trap，关闭后恢复
- toast 用 `role="status"` `aria-live="polite"`，error 用 `role="alert"` `aria-live="assertive"`
- 表单 label 必须 `for` 关联 input
- 表格 `<th scope="col">`、row header `<th scope="row">`

### 12.4 触摸目标

- 最小 32×32 px
- 间距 ≥ 4px（防止误触）
- 移动端（v0.3+）放大到 44×44

### 12.5 焦点管理

- 路由切换后焦点移到 page header 的 `<h1>`
- modal 打开时焦点移到第一个 input 或 close 按钮
- dropdown 关闭后焦点返回 trigger

---

## 13. 图标系统

### 13.1 图标库

- 唯一图标库：**lucide**（`https://unpkg.com/lucide@latest`）
- 风格：stroke，统一 stroke-width: 1.5（默认） / 2（filled 强调时）
- 尺寸梯度：12 / 14 / 16 / 18 / 20 / 24 / 32 / 48

### 13.2 命名映射（常用）

| 概念 | icon |
|---|---|
| 涨 / bull | `trending-up` `arrow-up-right` |
| 跌 / bear | `trending-down` `arrow-down-right` |
| Dashboard | `layout-dashboard` |
| Markets | `line-chart` |
| Wallet | `key-round` |
| LLM analysis | `sparkles` |
| LLM mgmt | `shield-check` |
| Brief | `calendar-check` |
| Settings | `settings` |
| Notifications | `bell` |
| Audit | `file-clock` |
| Help | `help-circle` |
| Success | `check` `check-circle-2` |
| Warning | `alert-triangle` |
| Error | `x-circle` `alert-octagon` |
| Empty | `inbox` |
| Refresh | `refresh-cw` |
| Sync | `refresh-cw` |
| Key | `key-round` |
| Search | `search` |
| User | `user` |
| Logout | `log-out` |
| Eye open | `eye` |
| Eye closed | `eye-off` |
| External | `external-link` |
| Download | `download` |
| Trash | `trash-2` |
| Pencil | `pencil` |
| Plus | `plus` |
| Filter | `filter` |

**强约束**：同一概念只用 1 个 icon（不允许 "Settings 页面用 settings 齿轮，Preferences 也用 settings 齿轮" — 应该都用 `settings`）。

### 13.3 主题适配

- 默认 stroke 用 `currentColor`（继承文字色）
- hover 状态不变色
- 状态色（bull/bear/warning）只在 icon 表达状态语义时使用
- matrix 主题下 accent icon 用 `var(--bull-green)` 即 `#10A37F`

---

## 14. 数据可视化规范

### 14.1 配色

| 序列 | 暗色 | 亮色 | matrix |
|---|---|---|---|
| 1 (主) | `var(--accent)` `#007ACC` | `#2563EB` | `#10A37F` |
| 2 (次) | `#4EC9B0` | `#0891B2` | `#A8D8B9` |
| 3 | `#F48771` | `#DC2626` | `#FF6B6B` |
| 4 | `#DCDCAA` | `#CA8A04` | `#FFD93D` |
| 5 | `#9CDCFE` | `#7C3AED` | `#6BCB77` |
| bull | `#4EC9B0` | `#16A34A` | `#10A37F` |
| bear | `#F48771` | `#DC2626` | `#FF6B6B` |
| warning | `#DCDCAA` | `#CA8A04` | `#FFD93D` |
| muted | `#858585` | `#6B7280` | `#5C5C5C` |

### 14.2 图表组件

| 类型 | 用途 | 规范 |
|---|---|---|
| Line (折线) | 价格走势、latency 趋势 | stroke-width 1.5，无 marker（密集），端点 marker 4px |
| Area (面积) | 累计量（cost、calls） | 渐变 alpha 0.2→0 |
| Bar (柱) | 类别对比、win rate | width = 0.6 × category width，gap 0.4 |
| Sparkline (迷你) | 表格行内趋势 | 高度 16px，无坐标轴 |
| Donut (环) | 占比（category distribution） | stroke-width 12，center label 24px |
| Heatmap (热力) | 矩阵（provider × 维度） | 11 阶 sequential colormap，0 = bg color |
| Scatter (散点) | 校准图（confidence vs actual） | dot 4px，回归线 dashed |

### 14.3 坐标轴

- x 轴 / y 轴线 color = var(--border)
- 刻度文字 10.5px var(--muted)，**不**显示单位
- 网格线 dashed 1px var(--border) alpha 0.4
- zero line 实线 1px var(--border-strong)
- 鼠标 hover 显示十字光标 + tooltip

### 14.4 数字格式

| 类型 | 格式 | 示例 |
|---|---|---|
| 价格（¢） | 整数 + ¢ 前缀 | `¢ 64` |
| 价格（小数） | 2 位小数 | `¢ 64.50` |
| USD | `$` + 千分位 + 2 位 | `$1,234.56` |
| 百分比 | 1 位小数 + % | `68.5%` |
| 涨跌幅 | 带符号 + 颜色 | `+1.2%` green / `−3.4%` red |
| 大数字 | K/M/B 缩写 | `1.2K`, `4.5M`, `2.1B` |
| Latency | ms / s 自适应 | `850ms` / `1.2s` |
| 相对时间 | 「2h ago」格式 | `just now` / `5m ago` / `2d ago` |

**字体**：所有数字用 `var(--font-mono)` 等宽（`JetBrains Mono` / `Menlo` / `monospace`），保持数字对齐。

---

## 15. 错误边界

### 15.1 错误分级

| Level | 例子 | 用户感知 | 恢复方式 |
|---|---|---|---|
| L1 信息性 | toast 提示「Saved to keychain」 | 不打断 | 自动消失 |
| L2 可恢复 | API key 无效 | toast + 行内 banner + retry 按钮 | 立即可操作 |
| L3 阻塞 | SQLite 损坏、keyring 不可用 | 全屏 error state | 重启 / 联系支持 |
| L4 致命 | Tauri runtime 崩溃 | 系统对话框 | 重启 app |

### 15.2 Error 状态视觉

```
┌─────────────────────────────────────┐
│        [icon: alert-octagon]        │
│                                     │
│        Title (14px, semibold)       │
│                                     │
│   Reason (12px, mono, muted)        │
│   max-width 480px, centered         │
│                                     │
│        [↻ Retry] [Help →]           │
└─────────────────────────────────────┘
```

- 背景 `rgba(248, 81, 73, 0.06)`，border `rgba(248, 81, 73, 0.2)` 1px，radius-3
- icon 40×40，var(--bear)
- padding 48×32

### 15.3 全局错误捕获

- Tauri command reject → toast `error_code + reason` + audit_log
- JS 异常 → 显示在 dev console，**不**弹窗（避免遮挡），生产 build 静默上报 Sentry
- 异步 IPC 超时（30s）→ 自动 retry 1 次，仍失败 → toast

### 15.4 离线处理

- IPC 调用失败（network error）→ toast "Offline — cached data shown"
- 关键路径（place bet、sync markets）阻塞并显示 retry
- 缓存数据加 `stale` 标签（黄色 chip）

---

## 16. 国际化（i18n）基础

### 16.1 范围

v0.2 仅支持 en-US + zh-CN 2 语言，**v0.3+** 扩展。

### 16.2 字符串提取规则

- 所有用户可见文字必须从 `i18n/<lang>.json` 读
- key 命名：`namespace.subname`，例 `nav.dashboard` `pages.analysis.title`
- 复数用 ICU 格式：`{count, plural, one {# market} other {# markets}}`
- 数字、日期、时间用 `Intl.NumberFormat` / `Intl.DateTimeFormat`

### 16.3 货币

- 数字旁边明确标注单位（USD / USDC / ¢）
- v0.2 仅支持 USD（**不**做汇率换算）
- crypto 价格保留 4 位有效数字（BTC 65423.12）

### 16.4 RTL

v0.2 不支持。预留 token 命名空间（`--space-start` `--space-end` 而非 `--margin-left`），未来切换不破坏布局。

### 16.5 字体回退

- 拉丁：`Inter` → `system-ui` → `sans-serif`
- CJK：`Inter` 后 fallback `PingFang SC` (mac) / `Microsoft YaHei` (win) / `Noto Sans CJK SC` (linux)
- mono：`JetBrains Mono` → `Menlo` → `Consolas` → `monospace`

---

## 17. Empty / Loading / Error 视觉规范

### 17.1 Empty State

**统一组件** `uiEmpty({ icon, title, subtitle, cta? })`：

```
        [48px icon, opacity 0.4, centered]

       Title (15px, semibold, fg)
       
       Subtitle (12.5px, muted, max-width 320px)
       
       [Optional CTA button]
```

- padding 64×32
- 文字居中
- icon 用 lucide 通用类（`inbox` `search` `database` `lock`）
- CTA 按钮仅在「有明确下一步操作」时显示

### 17.2 Loading State — Skeleton

**统一组件** `uiSkeleton(rows)`：

- 用 `linear-gradient` + `@keyframes skeleton-shimmer` 做流光
- 1.4s 周期，linear，无限循环
- 颜色 `var(--surface-2) → var(--surface) → var(--surface-2)`，alpha 0.3
- 表格 skeleton 行 = 3 段 bar（80px + flex + 60px），高 12px
- 列表 skeleton 行 = 整条 48px 高

**禁止**：用 `Loading...` 文字 + spinner 单独占满整个区域（除非 0 数据时）。

### 17.3 Error State

**统一组件** `uiError({ title, reason, onRetry? })`：见 §15.2

### 17.4 何时用哪个

| 状态 | 触发 | 用法 |
|---|---|---|
| Empty | 0 行 / 0 结果 / 首次未配置 | 居中 + 引导 CTA |
| Loading | 数据 fetch 中 | skeleton 流光（**不**用 spinner） |
| Error | fetch 失败 / 解析失败 | 红色卡片 + retry |
| Partial | 部分数据成功（LLM 4 个里 3 个成功） | 渲染成功的 + 失败项 inline error |

### 17.5 三态切换规范

```
mount → skeleton (immediate)
       ↓ data ok → real content (instant)
       ↓ data fail → error state (immediate)
       ↓ data empty → empty state (after skeleton)
```

- skeleton 至少显示 200ms（避免闪屏）
- skeleton → content **不**做 fade（避免「loading 完了还闪一下」）
- content → error 才做 240ms fade

---

## 变更日志

- **v2.1** (2026-06-16) — **PNG 截图 + Feature Coverage Matrix**：
  - 加 §3.1 Feature Coverage Matrix：17 module（M1-M13 + X1 + X2）100% 覆盖验证，每个 module 都有主页面 + 次级入口
  - 加 §3.2 截图重生成流程 + 54 PNG 文件位置（`docs/previews/{dark,light,matrix}/*.png`）
  - §3 路由表扩展到 18 页 × 3 主题，每行附 PNG 内嵌预览图
  - **修复 bug**：`prototype.html` 中 `const SIGNALS` 等 mock data 移到 `const ROUTES` 之前（修复 route() 首次调用时 TDZ ReferenceError，dashboard 之前截图空白的根因）
  - 工具：`scripts/snapshot_pages.py` — Python + Playwright，1440×900 viewport, device_scale_factor=2
- **v2.0** (2026-06-16) — 重大升级：
  - **6 个新页面**（§5.10-5.18）：Analysis / LLM Management / Daily Brief / Onboarding / Market Detail / Notifications / Help / Trade History / Audit Log
  - **§9 Design Tokens 深度规范**：spacing 11 阶 / radius 7 阶 / shadow 5 阶 / motion 6 duration / z-index 8 阶
  - **§10 组件状态机**：Button / Input / Card / Toggle / Modal / Toast 各 6 状态
  - **§11 动效**：入场 / 出场 / 交互反馈 / 主题切换 / prefers-reduced-motion 兼容
  - **§12 可访问性**：键盘导航 / 颜色对比度 / 屏幕阅读器 / 触摸目标 / 焦点管理
  - **§13 图标系统**：lucide 库 + 30+ 命名映射
  - **§14 数据可视化**：8 套配色 + 7 种图表规范 + 数字格式
  - **§15 错误边界**：4 级错误分级 + Error 视觉规范 + 离线处理
  - **§16 i18n**：范围 / 字符串提取 / 货币 / RTL 预留 / 字体回退
  - **§17 Empty/Loading/Error 视觉**：统一组件 + 切换规范
  - **Wallets 页增强**：PM CLOB 凭证表格上移
  - **Topbar 增强**：铃铛 + 头像 + 红点
  - **prototype.html**: 3091 行 / 23 render 函数（12 旧 + 6 新 + 5 helper）
- **v1.2** (2026-06-16) — Dashboard 加 **Today's Brief** 区块（M12 每日看板）；LLM Performance 页面深化为 5 个 tab（Table / Heatmap / Scatter / Timeseries / Decisions）。
- **v1.1** (2026-06-16) — Token 调整：dark 改为 Cursor / VS Code Dark+ 风格（`#1E1E1E` 编辑区、`#181818` sidebar、`#007ACC` VS Code 蓝、`#4EC9B0`/`#F48771` 柔和涨/跌色）；matrix 改为 OpenAI Codex CLI 风格（真黑 `#0A0A0A`、OpenAI 绿 `#10A37F`、border 极弱 `#1F1F1F`、去掉卡片阴影）。
- **v1.0** (2026-06-16) — 初版。基于已实现 AppShell + Dashboard，扩展到 8 个页面、24 个组件、6 条设计原则、完整主题 token 矩阵。
