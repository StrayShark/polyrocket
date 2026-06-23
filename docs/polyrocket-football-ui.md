# polyrocket — Football-First UI Design (v0.119)

> UI 设计规范:从 polyrocket 18-route 通用 Polymarket 客户端重塑为 football-first (产品名保持 polyrocket, 通过 tagline + UI 表达 pivot, 不通过 rename)。
>
> 版本：v1.0 · 2026-06-22
>
> 配套: [`polyrocket-football-prd.md`](./polyrocket-football-prd.md) (PRD) · [`polyrocket-football-frameworks.md`](./polyrocket-football-frameworks.md) (4-framework 知识库) · [`polyrocket-ui-design.md`](./polyrocket-ui-design.md) (legacy 1484 行, 部分章节被本文取代)

---

## 0. 设计决策 (需要确认)

⚠️ 在动手改代码前,以下 5 个决策必须拍板:

| # | 决策 | 选项 | 推荐 |
|---|---|---|---|
| **D1** | **Naming**: 仓库 + 产品名 | (a) 保持 polyrocket (推荐) / (b) 完全重命名 | ✅ **(a) 选定** (用户决策 2026-06-22)。Football pivot 通过 tagline + UI 表达, 不 rename。 |
| **D2** | **Markets filter default** | (a) 默认只显示 football (b) 默认 all + 顶部 toggle | **(a)** — pivot 决定, 让用户看不到 noise |
| **D3** | **Landing 路由** (`/` 默认跳哪) | (a) `/football` (新 hub) (b) `/dashboard` (现有) (c) `/brief` (今日 edge) | **(a)** — 与 PRD 一致, Football Hub 是入口 |
| **D4** | **非 football categories** | (a) DB 全删 (b) DB 保留 + UI hide (c) Settings toggle 显示 | **(b)** — 兼容旧数据, 不破坏升级 |
| **D5** | **Share card / 分享图卡** | (a) v0.122 内置 (b) P3 后做 | **(a)** — Twitter persona 关键功能 |

---

## 1. 新产品身份 (Identity)

### 1.1 产品名 + Logo + Slogan

```
polyrocket

[logo] ⚽ polyrocket                          (窗口顶部 logo + ⚽ 足球 icon)
        ↑ 用足球 emoji + lowercase wordmark
        "Football intelligence for Polymarket"
```

### 1.2 三主题色 (新 football accent)

| 主题 | 主背景 | 文字 | Accent (新增 football) | 适用 |
|---|---|---|---|---|
| **dark** (default) | `#0a0e14` | `#e6edf3` | **Pitch Green `#2e7d32`** | 默认 |
| **light** | `#fafafa` | `#1a1a1a` | **Pitch Green `#1b5e20`** | 白天 |
| **hc** (high contrast) | `#000000` | `#ffffff` | **Lime `#a4ff00`** | 无障碍 |

Accent 选择理由:
- **Pitch Green** 是经典足球场色 (#2e7d32), 暗示"grass / 球门 / 战术板"
- 替代 polyrocket 当前的紫色 accent, 立刻视觉差异化
- 三个主题的 accent 都偏绿/黄绿, 保持 brand identity 一致

### 1.3 字体 / Icon

- 字体保持现有 `Inter` (sans) + `JetBrains Mono` (numeric)
- **新增 icon**: ⚽ soccer ball (lucide `CircleDot` 或新加 `soccer-ball` 组件)
- 比赛组件用 **team color 标记条** (1px 左侧 border) 暗示球队归属 (不强制, 仅在 fixture 卡片用)

---

## 2. 路由总览 (Route Map)

### 2.1 v0.119 改造后路由 (18 → 16)

```
/                   → Navigate to /football (D3)
/welcome            → Onboarding 6 步 (重做 hero: football focus)
/football           ← 新主入口 (Football Hub)
/football/:comp     ← 新: 单 competition 详情 (Premier League / La Liga / etc.)
/football/fixtures/:id  ← 新: 单场比赛 deep-dive
/football/edge-board ← 新: 全场 edge 排行
/dashboard          → 改为 /football, redirect /dashboard → /football (legacy 兼容)
/markets            → 改造: 默认 filter=football, 顶部加 competition filter
/markets/:id        → 改造: 加 Football Analytics 区块 (Elo/xG/Poisson)
/signals            → 改造: signal source 改为 football-specific
/copy               → 降级为 P2, 保留但 inactive nav
/pnl                → 保留
/lab                → 改造: 加 league filter, 重命名为 "Football Model Lab"
/trade              → 改造: form 加 market_type radio (1X2/O/U/Handicap)
/history            → 保留
/wallets            → 简化: football context only
/settings           → 保留
/analysis           → 改造: framework_breakdown 卡片
/llm-perf           → 加 football.v1.0 vs market.v1.0 对比 tab
/llm-mgmt           → 保留
/brief              → 重建: "Today's Edge" 8 football picks
/audit              → 保留
/notifications      → 保留
/bankroll           → 加 per-league exposure cap
/help               → 保留
```

### 2.2 Sidebar 新结构

**v0.118 当前**:
```
polyrocket logo
├── Workspace
│   ├── Dashboard    128
│   ├── Markets      128
│   ├── Signals      7
│   ├── Copy
│   ├── PnL
│   ├── Model Lab
│   ├── Trade        (v0.52)
│   └── Bankroll     (v0.78)
├── Categories
│   ├── Football     62
│   ├── CS2          41
│   └── Politics     25
└── Settings section
```

**v0.119 新**:
```
⚽ polyrocket       (logo)
├── Football        (主导航 section, NEW)
│   ├── Hub         (NEW = /football)
│   ├── Fixtures    (NEW, 折叠下拉: Today / Tomorrow / This Week)
│   ├── Edge Board  (NEW = /football/edge-board, badge "3+")
│   └── Competitions(NEW = /football/:comp, 折叠下拉: PL / La Liga / etc.)
├── Insights        (新增 section)
│   ├── Signals     (football-specific, badge)
│   ├── Analysis    (单市场 + framework breakdown)
│   └── Brief       (Today's Edge)
├── Trading         (NEW section, 重命名 + 重排)
│   ├── Markets     (filter=football 默认)
│   ├── Trade       (form 升级)
│   └── Bankroll    (per-league cap)
├── Tools           (新增 section)
│   ├── Model Lab   (football training)
│   ├── LLM Mgmt
│   ├── LLM Perf
│   └── PnL
└── Settings section
    ├── Wallets
    ├── Settings
    ├── Help
    └── Audit (折叠到 settings 底部, dev-only)
```

**Nav 改动**:
- 删除 `CATEGORY_NAV` section (football 是默认, 不再列)
- 新增 `Football` / `Insights` / `Trading` / `Tools` 4 个 section
- 每个 section 4-6 项, 总数从 13 → ~17, 但分组更清晰

---

## 3. Landing Page (`/welcome`)

**当前** Welcome 是 6 步 wizard (Welcome / Storage / Theme / LLM / Polymarket / Finish)。

**v0.119 新版**: 保留 6 步结构, 但 **Step 1 (Welcome) 重做**:

### Step 1: Welcome (hero)

```
┌─────────────────────────────────────────────────────────────┐
│ ⚽  polyrocket                                  [Skip →]   │
│                                                              │
│                                                              │
│      Football intelligence for Polymarket                   │
│      ────────────────────────────────────                   │
│      Use 4 industry-standard frameworks to find             │
│      mispricings in football prediction markets.            │
│                                                              │
│      ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐│
│      │ Dixon-   │  │ Elo      │  │ xG       │  │ CLV      ││
│      │ Coles    │  │ 1850→0.78│  │ 1.85→0.62│  │ edge=+3% ││
│      │ Poisson  │  │          │  │ Sam Green│  │ Pinnacle ││
│      │ + ρ adj  │  │ HFA +100 │  │ 2012     │  │ framework││
│      └──────────┘  └──────────┘  └──────────┘  └──────────┘│
│                                                              │
│      [ Get started → ]   [ Restore last session ]           │
│                                                              │
│      v0.119 · 14d edge picks avg +3.2% · 200+ resolved      │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

**Step 2-5**: 保留现有 wizard (Storage / Theme / LLM / Polymarket)。
**Step 6 (Finish)**: 改为 football-themed 完成页:
- "Setup complete. Your football intelligence is ready."
- 跳转按钮: `[ Open Football Hub → ]` (跳 `/football`)

---

## 4. 主页面详细规范

### 4.1 `/football` (Football Hub) — **NEW 主入口**

```
┌─────────────────────────────────────────────────────────────┐
│  Football Hub                                    [Sync ↻]  │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  ┌─ KPI strip ─────────────────────────────────────────┐    │
│  │  📅 7 upcoming     🎯 +3.2% avg edge   📊 62 mkts   │    │
│  │     fixtures       (14d, conf ≥0.65)    tracked     │    │
│  └──────────────────────────────────────────────────────┘    │
│                                                              │
│  ┌─ Today / Tomorrow ──────────────────────────────────┐   │
│  │                                                      │   │
│  │  ⚽ Argentina vs Austria        17:00 UTC            │   │
│  │     [TeamWin] Polymarket 66.5%  Our 67.2%  edge+0.7% │   │
│  │     [O/U 2.5]   Polymarket 50.5%  Our 58.1%  edge+7.6%│   │
│  │     [Handicap]  ...                                  │   │
│  │                                                      │   │
│  │  ⚽ Senegal vs [TBD]           Tomorrow 00:00 UTC    │   │
│  │     ...                                              │   │
│  └──────────────────────────────────────────────────────┘    │
│                                                              │
│  ┌─ Competitions ──────────────────────────────────────┐   │
│  │   🏆 Premier League (12 mkts)                       │   │
│  │   ⚽ La Liga (8 mkts)                                │   │
│  │   🇫🇷 Ligue 1 (5 mkts)                                │   │
│  │   🌎 World Cup 2026 (24 mkts)                        │   │
│  │   ... + Show all (8)                                 │   │
│  └──────────────────────────────────────────────────────┘    │
│                                                              │
│  ┌─ Top Edge Picks Today ──────────────────────────────┐   │
│  │ #1 Argentina O/U 2.5    model 58.1%  mkt 50.5%  +7.6%│   │
│  │ #2 France -2.5          model 71%    mkt 56.5%  +14% │   │
│  │ #3 Senegal TeamWin      model 35%    mkt 29.5%  +5.5%│   │
│  │ #4 ...                                                  │   │
│  └──────────────────────────────────────────────────────┘    │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

**Layout 规范**:
- 顶部: 标题 + Sync button
- KPI strip: 4 个 metric 卡 (count of fixtures / avg edge / market count / LLM call latency)
- 今日/明日 fixtures: 时间倒序, 每个 fixture 一个 card
- Competitions: collapsible list, click → /football/:comp
- Top Edge Picks: 5 条, sort by edge DESC

### 4.2 `/football/fixtures/:matchId` — 单场 deep-dive (NEW)

5 个 tab:

```
┌─────────────────────────────────────────────────────────────┐
│  ← Back   Argentina vs Austria                              │
│            FIFA World Cup 2026 · Group C · 2026-06-22 17:00 │
│            [Polymarket →]  [Refresh ↻]                      │
├─────────────────────────────────────────────────────────────┤
│ [Match Info] [xG] [Poisson] [Polymarket] [Our Model]        │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│   MATCH INFO TAB (default)                                  │
│                                                              │
│   ┌─ Home: Argentina ──────────┐  ┌─ Away: Austria ─────┐  │
│   │ Elo:        1875           │  │ Elo:        1640      │  │
│   │ FIFA Rank:  1              │  │ FIFA Rank:  25        │  │
│   │ Form W-D-L: W-W-D-W-W      │  │ Form: W-L-D-W-L       │  │
│   │ Season xG:  1.85 / 90      │  │ Season xG: 0.92 / 90  │  │
│   │ Season xGA: 0.71 / 90      │  │ Season xGA: 1.42 / 90 │  │
│   └────────────────────────────┘  └─────────────────────────┘ │
│                                                              │
│   H2H last 5: ARG 3W-1D-1L                                   │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

**xG Tab**:
- 双 line chart: home / away xG per 90 (最近 5 / 10 / 赛季)
- 数据来源: FBref / Opta (v0.123+ ETL)
- v0.119-v0.122 fallback: LLM 估 (with confidence 标注)

**Poisson Tab**:
- Dixon-Coles λ_home / λ_away 大字
- 1X2 probability 3 柱 (Home 58% / Draw 22% / Away 20%)
- Score matrix (5x5 grid) showing P(home=i, away=j) %
- O/U 2.5 probability bar
- Asian Handicap -1.5 probability bar

**Polymarket Tab**:
- List of all related markets:
  - "Will Argentina win?"  66.5% YES, 33.5% NO, $2.85M 24h vol, $2.15M liq
  - "Argentina vs Austria draw?" 22.5% YES, 77.5% NO
  - "Argentina vs Austria O/U 2.5" 50.5% Over
  - "Argentina -1.5 Handicap" 71% Argentina covers
- 每个 market 旁边 [Trade →] 按钮

**Our Model Tab** (核心差异化):
- LLM Football Analysis card (复用 Analysis.tsx)
- **Football Framework Breakdown** 卡片 (NEW):
  ```
  ┌────────────────────────────────────────────────────┐
  │ Football Framework Breakdown (v0.118 football.v1.0)│
  ├────────────────────────────────────────────────────┤
  │ Elo         diff +235 with HFA   → P(win) 78%     │
  │ Dixon-Coles λ_h=1.85, λ_a=0.95    → P(win) 58%    │
  │             draw 22% / away 20%                    │
  │ xG trend    ARG +0.4 over last 5                   │
  │ CLV edge    model 67.2% vs mkt 66.5%  → +0.7%     │
  │ Confidence  0.72                                    │
  │ Final       YES (edge positive, conf ≥ 0.6)        │
  └────────────────────────────────────────────────────┘
  ```
- "Reasoning" 折叠区: LLM 输出完整 reasoning (≤ 800 chars)
- 时间序列 (optional v0.123+): model_prob vs mkt_implied_prob 历史曲线

### 4.3 `/football/edge-board` — Edge 排行 (NEW)

```
┌─────────────────────────────────────────────────────────────┐
│  Edge Board                                                  │
│  All football markets with |clv_edge| ≥ 3%                  │
├─────────────────────────────────────────────────────────────┤
│  Filters: [Competition ▾] [Market Type ▾] [Min Edge 3% ▾]  │
│           [Min Conf 60% ▾] [Min Vol $50k ▾]                  │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  Sort: [Edge DESC ▾]   Showing 12 of 62 football markets     │
│                                                              │
│  ┌──────────────────────────────────────────────────────┐   │
│  │ #1  France -2.5 Handicap   World Cup                │   │
│  │     Polymarket 56.5%  →  Our 71.0%   edge +14.5%    │   │
│  │     Conf 0.81  Vol $475k  Liq $294k  [Analyze] [↗] │   │
│  ├──────────────────────────────────────────────────────┤   │
│  │ #2  Argentina O/U 2.5      World Cup                │   │
│  │     Polymarket 50.5%  →  Our 58.1%   edge +7.6%     │   │
│  │     Conf 0.74  Vol $522k  Liq $720k  [Analyze] [↗] │   │
│  ├──────────────────────────────────────────────────────┤   │
│  │ ...                                                  │   │
│  └──────────────────────────────────────────────────────┘   │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

**Table 列**:
- Rank
- Match (Home vs Away)
- Competition (badge)
- Market Type (TeamWin / Draw / O/U / Handicap)
- Polymarket YES (%)
- Our Model (%)
- Edge (% with color: green > +3%, yellow 1-3%, red < 0%)
- Confidence (0-1 with 1 decimal)
- Volume / Liquidity (compact format $475k/$294k)
- Actions: [Analyze] (调 football.v1.0) [Trade] (跳 /trade)

### 4.4 `/markets` (改造)

**当前**: 显示所有 markets, filter=category

**v0.119 新版**:
- **默认 filter**: `category == "football"` (D2 决策)
- **顶部新增**: Competition filter (`[All competitions ▾]` / `[Premier League] [La Liga] ...`)
- **顶部新增**: Market Type filter (`[All types ▾]` / `[TeamWin] [Draw] [O/U] [Handicap]`)
- **顶部新增**: Edge filter (`[All] [+1%] [+3%] [+5%]`)
- **卡片角标**: 每个非 football market 卡片右上角灰色 "CS2" / "Politics" badge, 点开才能看到 (符合 D4 决策)
- **排序选项**: 加 `Edge DESC` 排序 (默认 = Volume DESC)

### 4.5 `/markets/:id` (MarketDetail 改造)

复用现有 MarketDetail 组件, **新增 "Football Analytics" 区块** (仅当 category == "football"):

```
┌─ Football Analytics ─────────────────────────────┐
│ [View full breakdown on /football/fixtures/:id ↗]│
├──────────────────────────────────────────────────┤
│   Elo diff:  +235  →  P(win) 78%                 │
│   λ_h / λ_a: 1.85 / 0.95                         │
│   1X2 probs: 58% / 22% / 20%                    │
│   xG last5:  ARG +0.4 / AUT -0.2                 │
│   Our prob:  67.2%   Polymarket: 66.5%           │
│   CLV edge:  +0.7%   Confidence: 0.72            │
│   Action:    YES                                   │
└──────────────────────────────────────────────────┘
```

(完整 breakdown 跳 `/football/fixtures/:matchId`)

### 4.6 `/dashboard` → redirect `/football`

**D3 决策**: landing = `/football`

`main.tsx`:
```tsx
{ index: true, element: <Navigate to="/football" replace /> }
{ path: 'dashboard', element: <Navigate to="/football" replace /> }, // legacy 兼容
```

`/football` 复用现有 Dashboard 组件 (KPI strip + Today's picks + Upcoming fixtures), 不重写。

### 4.7 `/brief` (Daily Brief 重建)

**当前**: Top 8 daily brief cards

**v0.119 新版**: "Today's Edge"

```
┌─────────────────────────────────────────────────────────────┐
│  📈 Today's Edge                                            │
│  2026-06-22 · 8 football picks · avg edge +3.2% (conf≥0.65)│
│  [Refresh ↻]  [Export as image 📷]                          │
├─────────────────────────────────────────────────────────────┤
│  #1  Argentina O/U 2.5                                     │
│      World Cup 2026  ·  17:00 UTC                           │
│      Polymarket 50.5%  →  Our 58.1%  →  edge +7.6%         │
│      Conf 0.74  ·  [YES]                                    │
│  ─────────────────────────────────────────                  │
│  #2  France -2.5 Handicap                                  │
│      ...                                                    │
└─────────────────────────────────────────────────────────────┘
```

- 顶部: 日期 + 总数 + avg edge (KPI strip)
- 每条: ranking + match + competition + Polymarket → Our → Edge 三栏 + Conf + Action
- `[Export as image]` 按钮 → 生成 PNG (Twitter 优化 1200x630)
- 自动生成时间: 每天 08:00 (cron)

### 4.8 `/lab` (Model Lab 改造)

**当前**: 通用 ML training 界面

**v0.119 新版**: "Football Model Lab"
- 顶部加 League filter (`[Premier League]` etc.)
- 加 Model Type selector: `[Elo]` `[Dixon-Coles]` `[xG]` `[Poisson]` (默认 Elo)
- 加训练数据: 公开 FBref datasets, Opta sample
- 训练结果可视化: Brier score / CLV / win-rate

### 4.9 `/trade` (改造)

**当前**: Polymarket 通用 place order form

**v0.119 新版**:
- 顶部加 Market Type radio: `( ) 1X2 ( ) O/U ( ) Asian Handicap ( ) Outright`
- 选择 market type 后, 显示对应 fields:
  - 1X2: bet on which side (Home / Draw / Away)
  - O/U: line (e.g. 2.5) + side (Over / Under)
  - Asian Handicap: line + side (Home covers / Away covers)
  - Outright: pick team
- 右侧加 "Football Model Recommendation" 卡片 (用 football.v1.0 输出)

### 4.10 `/bankroll` (改造)

**当前**: 通用 Kelly / 仓位管理

**v0.119 新版**:
- 新增 "Per-League Exposure Cap" 配置
- 每个 league 可设置 max % of bankroll
- 例如: Premier League ≤ 30%, La Liga ≤ 20%, 其他 ≤ 50%
- 下注时如果超过 cap, 弹 warning

---

## 5. 主题 / 视觉

### 5.1 新增 Pitch Green accent

**v0.118 CSS variables**:
```css
--accent: #6366f1;  /* indigo/purple — generic */
```

**v0.119 新增 football accent**:
```css
--accent-football: #2e7d32;          /* dark theme */
--accent-football-light: #1b5e20;    /* light theme */
--accent-football-hc: #a4ff00;       /* high contrast */
```

**Default `--accent` 切换**: 全部改为 football accent (因为不再有 generic mode)

### 5.2 League color 标记条 (optional v0.123+)

每个 league 一个 accent color (球队球衣色), fixture 卡片左侧 1px border:
- Premier League: `#3d195b` (深紫)
- La Liga: `#ee8707` (橙)
- Serie A: `#008fd7` (浅蓝)
- Bundesliga: `#d20515` (红)
- Ligue 1: `#091c3e` (深蓝)
- World Cup 2026: `#ffffff` (白, 加 border)
- Champions League: `#001d63` (深蓝)

(暂定, v0.119 不实现, 留 P3)

---

## 6. 关键交互模式

### 6.1 Football Market Detection 自动分流

**v0.118 已实现**:
- `commands/llm.rs` 在 `market.category == "football"` 时自动用 football.v1.0 prompt
- 这个行为对用户透明, 不需要 UI 改动

### 6.2 Edge 颜色编码 (3 色)

| Edge | 颜色 | 语义 |
|---|---|---|
| `edge > +5%` | `--bull` (绿) | 强 edge, 立即考虑 trade |
| `+1% < edge < +5%` | `--accent` (accent) | 中等 edge, 需看 confidence |
| `edge < -5%` | `--bear` (红) | 反向 edge, 考虑 NO 边或 skip |
| 其他 | `--muted` (灰) | 中性, 不建议 trade |

### 6.3 推送通知模板

**Football edge alert** (新模板):
```
⚽ polyrocket · +7.6% edge found

Argentina vs Austria · O/U 2.5
Our model: 58.1%  Polymarket: 50.5%
Conf: 0.74  Action: YES
17:00 UTC · ~3 hours to kickoff

[Open in app]
```

### 6.4 Share Card (v0.122)

每条 edge pick 可生成 PNG (1200x630 Twitter optimal):
```
┌─────────────────────────────────────┐
│ ⚽ polyrocket                        │
│ ────────────────────────────────    │
│                                      │
│ Argentina  vs  Austria               │
│ World Cup 2026 · 2026-06-22 17:00   │
│                                      │
│   Market:  O/U 2.5                  │
│   Polymarket: 50.5%                 │
│   Our model:  58.1%                 │
│   Edge:       +7.6%  ✅             │
│   Confidence: 0.74                   │
│   Action:     YES                   │
│                                      │
│ ────────────────────────────────    │
│ football.v1.0 · Dixon-Coles+...    │
└─────────────────────────────────────┘
```

---

## 7. i18n 新增 keys

```
nav.football_hub         "Football Hub"
nav.football_fixtures    "Fixtures"
nav.football_edge_board  "Edge Board"
nav.football_competitions "Competitions"
nav.insights             "Insights"
nav.trading              "Trading"
nav.tools                "Tools"

football.kpi.upcoming    "7 upcoming fixtures"
football.kpi.avg_edge    "+3.2% avg edge (14d)"
football.kpi.markets     "62 markets tracked"
football.kpi.latency     "12s avg LLM call"

football.market_type.team_win       "Team Win"
football.market_type.draw          "Draw"
football.market_type.over_under     "Over/Under"
football.market_type.asian_handicap "Asian Handicap"
football.market_type.outright       "Outright"

football.framework.dixon_coles     "Dixon-Coles"
football.framework.elo             "Elo Rating"
football.framework.xg              "xG (Expected Goals)"
football.framework.clv             "CLV (Closing Line Value)"

football.action.yes   "YES"
football.action.no    "NO"
football.action.skip  "Skip"

football.brief.title             "Today's Edge"
football.brief.subtitle          "{n} football picks · avg edge {edge}%"
football.brief.export_image      "Export as image"
```

约 25 个新 key, 中文版同步翻译。

---

## 8. 改动量估算

| 维度 | v0.118 | v0.119 增量 | 备注 |
|---|---|---|---|
| **Routes** | 18 | 16 (+3 new - 5 replaced) | new: /football, /football/:comp, /football/fixtures/:id, /football/edge-board |
| **Components** | 24 | +6 (FootballHub, FixtureDetail, EdgeBoard, FootballFrameworkCard, CompetitionBadge, LeagueColorBar) | 6 new |
| **CSS variables** | 6 | +3 (`--accent-football` × 3 themes) | 主题适配 |
| **i18n keys** | ~570 | +25 | football nav + market types |
| **Prompts** | 4 | unchanged (football.v1.0 已就位) | 0 新 |
| **Database tables** | 16 | 0 (复用 `llm_analyses` + `signals`) | 0 新 |
| **IPC commands** | 78 | 0 (复用现有) | 0 新 |
| **Cron jobs** | 1 | +1 (`football_scan_every_15m`) | scan + signal push |

**Total 增量**: ~1500 行 (UI components + i18n + landing rebrand + nav restructure + share card 框架)。

---

## 9. Migration Plan

### 9.1 数据迁移

- 现有 markets 表保留 (D4 决策 b): 不删, 加 `category IS NULL OR category = 'football'` 为默认 filter
- 现有 `llm_analyses` 全部保留, 历史 prompt_version = "market.v1.0" / "consensus.v1.0" 维持
- 现有 settings 保留 (theme / locale / LLM keys)

### 9.2 代码迁移

- **新增**: 6 个 football 组件 (FootballHub, FixtureDetail, EdgeBoard, FrameworkBreakdown, CompetitionBadge, LeagueColorBar)
- **修改**: 5 个现有组件 (Sidebar, Dashboard, Markets, MarketDetail, Brief, Trade, Lab, Bankroll)
- **路由修改**: main.tsx (3 个新 path + 2 个 redirect)
- **i18n**: 25 个新 key
- **theme**: 3 个新 CSS variable

### 9.3 不破坏的保证

- 所有现有 18 个 route path 都还能访问 (要么 redirect 到新 path, 要么保留并 hide in nav)
- 现有 settings / wallet / LLM 配置全部保留
- 现有 tests 不修改 (只有 5-10 个 test file 可能因 UI props 改动需要微调)

---

## 10. Acceptance Criteria (v0.119)

✅ **必需通过**:
- [ ] 默认 landing = `/football`, 不是 `/dashboard`
- [ ] Sidebar 含 4 section (Football / Insights / Trading / Tools), 17 个 nav item
- [ ] Markets 默认 filter = football, 顶部有 Competition / Market Type / Edge 三个 filter
- [ ] `/football/edge-board` 至少显示 1 条 football market (用 seed 数据)
- [ ] `/football/fixtures/:id` 5 个 tab 都能渲染
- [ ] Football Market Detail 加 "Football Analytics" 区块 (复用 `FootballRecommendationPayload.framework_breakdown`)
- [ ] `/trade` form 加 Market Type radio, 切换显示对应 fields
- [ ] `/bankroll` 加 Per-League Exposure Cap 配置
- [ ] Brief 改名为 "Today's Edge", 加 Export as image 按钮 (v0.122 才实现也可)
- [ ] **命名保持 polyrocket**(用户决策):`package.json` `description` 改 football-focused、`README.md` tagline 改 football-focused、`AppShell` logo icon 加 ⚽(保留 polyrocket 文字)、`src/main.tsx` `<title>` 加 football 副标题、新 i18n key `app.tagline = "Football intelligence for Polymarket"`。**bundle id / productName / Cargo bin name 全部不变**
- [ ] 主题 accent 改为 pitch green (#2e7d32)
- [ ] 现有 415 个 cargo test 仍 pass
- [ ] vitest 覆盖率不掉

✅ **强烈推荐**:
- [ ] Onboarding Step 1 hero 重做 (4-framework pitch)
- [ ] Onboarding Step 6 finish 改 football-themed
- [ ] 至少 20 个新 vitest tests (FootballHub, FixtureDetail, EdgeBoard 组件)
- [ ] Sidebar nav restructured 文档化 (更新 polyrocket-ui-design.md §3.3)
- [ ] E2E 1 个 Playwright test: 打开 app → /football → 点 1 个 edge → 跳到 MarketDetail

⚠️ **可选**:
- [ ] Share card PNG 生成 (v0.122 也行)
- [ ] League color border (v0.123+)
- [ ] Fixture deep-dive 时间序列图 (v0.123+)

---

## 11. 与现有 docs 的关系

| Doc | 关系 |
|---|---|
| `polyrocket-football-prd.md` | **AUTHORITATIVE for product scope**. 本文引用 §3.1, §6 success metrics. |
| `polyrocket-football-ui.md` (本文) | **AUTHORITATIVE for UI**. 取代 `polyrocket-ui-design.md` §3 (route map), §5 (page specs), §6 (interactions). |
| `polyrocket-ui-design.md` | **LEGACY** for non-football parts. §0 (设计原则), §1 (theme tokens), §2 (全局布局), §4 (组件库), §7 (load/error/empty), §8 (响应式), §9 (design tokens) — 全部仍适用. §3, §5.1-§5.18 标记 DEPRECATED. |
| `polyrocket-football-frameworks.md` | NEW. 4 个 framework (Dixon-Coles / Elo / xG / CLV) 的学术 + 业界参考. 由 PRD §1 + v0.118 football.v1.0 prompt 引用. |
| `polyrocket-landing-design.md` | **UPDATED** by v0.119 — Step 1 hero + Step 6 finish 重做, 其余 4 步不变. |

---

## 12. 一句话

> v0.119 把 polyrocket 从"18-route 通用 Polymarket 桌面客户端"pivot 为"**16-route 足球专用菠菜 intelligence**(命名保持 polyrocket, tagline + UI 表达 football-first),默认 landing 是 Football Hub,导航重构成 Football / Insights / Trading / Tools 4 个 section,football.v1.0 prompt + 4-framework breakdown + Edge Board 是核心差异化"。
