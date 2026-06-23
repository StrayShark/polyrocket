# polyrocket — Football Pivot PRD (v0.119)

> 产品定位 / 目标用户 / 范围变更 / 命名 / 成功指标
>
> 版本：v1.0 · 2026-06-22 (项目目标 pivot: 从"通用 Polymarket 分析客户端" → "专注足球市场的预测 + 套利桌面端")
>
> 配套: [`polyrocket-football-ui.md`](./polyrocket-football-ui.md) (UI 设计) · [`polyrocket-football-frameworks.md`](./polyrocket-football-frameworks.md) (4-framework 知识库) · [`polyrocket-ui-design.md`](./polyrocket-ui-design.md) (legacy 18-route UI, 部分章节被 football-ui 取代)

---

## 0. Pivot 一句话总结

把 polyrocket 从"通用 Polymarket 桌面客户端(18+ 路由覆盖 9 个市场类别)"重塑为"**足球菠菜智能终端**:综合 Dixon-Coles + Elo + xG + CLV 4 个业界框架,跟 Polymarket 足球盘口实时对比,找出 mispricing edge"。

---

## 1. 为什么 pivot

### 1.1 业务面理由

| 维度 | 数据 |
|---|---|
| **Polymarket 上足球市场的密度** | 当前 `category_NAV` 计数显示 **Football = 62** 个活跃市场(最多), CS2 = 41, Politics = 25 — 足球是 Polymarket 最大的"非政治"垂直 |
| **足球分析的学术深度** | 4 个有 30+ 年研究沉淀的 framework (Dixon-Coles 1997 / Elo 1978 / xG 2012 / Kelly 1956) — 其他类别 (CS2 / 流行文化) **没有**等量级的可调用知识库 |
| **数据可获得性** | FBref / clubelo.com / StatsBomb / Opta 公开 + 半公开, ETL 友好; CS2 / Politics 的训练数据稀薄 |
| **用户的"edge" 心理模型** | 足球菠菜用户天然理解 "expected goals / closing line value / bookmaker vig" 这套语言; CS2 政治用户没这个 baseline |

### 1.2 工程面理由

| 维度 | 数据 |
|---|---|
| **LLM reasoning 质量** | v0.118 football.v1.0 prompt 综合 4 framework 输出 `framework_breakdown` 中间值,实测 LLM 在足球语境下能给出 Dixon-Coles λ/xG/CLV 等量化输出; 非足球语境下 LLM 经常 hallucinate |
| **prompt 模板复用率** | 现有 3 个 prompt (market.v1.0 / thesis.v1.0 / consensus.v1.0) 通用化但失去深度; football.v1.0 把"垂直深度"换来 |
| **Category 分类器利用率** | 现有 `Category` enum 9 个变体实际只用 3 个 (Football/Cs2/Politics); 其余 6 个 (Crypto/Tech/Science/PopCulture/Business/Other) 都是 noise |

### 1.3 不 pivot 的成本

继续做通用 Polymarket 客户端:
- LLM prompt 永远通用化,失去"垂直深度"的核心壁垒
- 用户面对 18 个路由 (Dashboard/Markets/Signals/Copy/PnL/Trade/Lab/History/Wallets/Settings/Analysis/LlmPerf/LlmMgmt/Brief/Welcome/Audit/Notifications/Bankroll/Help) 不知道先点哪个
- 难以跟 BetteReddite / Football Whispers / Pinnacle 等垂直玩家差异化

---

## 2. 目标用户 (Personas)

### 2.1 Primary: "Football Quant Hobbyist"

| 维度 | 描述 |
|---|---|
| **背景** | 受过 STEM 训练,业余时间搞足球预测; 读过《Soccermatics》《The Numbers Game》; 自己写过 Elo 模型但嫌弃手工维护 |
| **痛点** | Pinnacle / Asian books 数据不公开; Polymarket 上有盘口但没有"模型 vs 市场"对比工具 |
| **工作流** | 每周末扫一遍未来 7 天 fixtures → 看模型 P(YES) vs Polymarket YES price → 找 +5% 以上 edge → 下注 |
| **付费意愿** | 中。愿意为"省时" + "可视化" 付月费 $20-50, 但前提是 win-rate 真能 +2-3% |
| **关键 KPI** | 模型 Brier score / CLV / 周 edge picks 数量 / 月 ROI |

### 2.2 Secondary: "Polymarket Sports Trader"

| 维度 | 描述 |
|---|---|
| **背景** | Polymarket 资深用户, 之前 trade 政治 / 加密, 现在发现足球市场流动性最高 (62 个 > 政治 25) 想切入 |
| **痛点** | Polymarket UI 没有"按 league filter"; Sports tab 没有"模型建议"; 决策全靠 Discord / Twitter |
| **工作流** | 开 polyrocket → /football → 按 league filter (Premier League / World Cup) → 看 edge 排行 → 点进 MarketDetail 看 framework breakdown → place order |
| **付费意愿** | 低-中。 Polymarket 本身免费, polyrocket 必须有 clear "alpha" 价值 |

### 2.3 Tertiary: "Content Creator / Tipster"

| 维度 | 描述 |
|---|---|
| **背景** | Twitter/X / Telegram 上发足球 tips, 想用"模型依据"代替纯 gut feel |
| **痛点** | 需要 "我们的模型觉得这场阿根廷 67%, Polymarket 65%, edge +2%, 推荐 YES" 这种 one-liner 可截图 / 复制粘贴 |
| **工作流** | /brief → 复制 today edge picks → 写 Twitter thread |
| **付费意愿** | 低。如果有 "share card" / "export to image" 功能会付 |

### 2.4 Anti-personas (不做)

- **传统 bookmaker 用户** (Bet365 / 1xBet 用户) — 数据不在我们 scope, 不追
- **Casual bettor** (周末看球随手买) — 太碎片, 不值得为这个人群优化
- **专业 sportsbook trader** (有 Quant team / 自建模型) — 不会用桌面端, 用 Python + 直接打 CLOB API

---

## 3. MVP 范围 (v0.119-v0.130)

### 3.1 保留的模块 (P0 必留)

| Module | 原因 | 改动 |
|---|---|---|
| **M1 Markets** | 核心 — 用户来这就是为了看市场 | **Filter 强制 football**, UI 加 league / competition filter |
| **M1.b MarketDetail** | 核心 — 看 framework breakdown | 加 "Football Analytics" 区块 (Elo/xG/Poisson 可视化) |
| **M2 Signals** | 核心 — 主动推送 edge picks | 信号 source 改为 football-specific (xG trend / lineup news / odds movement) |
| **M3 Bets** | 核心 — 下单 | Trade page form 加 football-specific fields (handicap / O/U line / stake 类型) |
| **M4 Wallets** | 基础 — Polymarket CLOB 需要 USDC wallet | **保留**, 但 UI 简化 (football-only context 不需要 wallet manager) |
| **M6 PnL** | 基础 — 看过往 ROI | 保留 |
| **M8 Dashboard** | 入口 — 今日 / 明日 fixtures | **Rebuild** 成 "Football Hub": upcoming fixtures + today's edge picks + standing |
| **M9 Settings** | 基础 — LLM / theme / data path | **保留** |
| **M10 Analysis** | 核心 — LLM analysis 结果展示 | 重点改造: 加 football framework breakdown 卡片 |
| **M11 Bankroll** | 中期 — Kelly / 仓位管理 | 保留, 加 football-specific bankroll rule (per-league exposure cap) |
| **X1 AuditLog** | 基础 — 记录所有 action | 保留 |
| **X2 Notifications** | 中期 — 系统推送 edge picks | 保留, 但 notification 模板 football-specific |
| **M12 Brief** | 核心 — 每日 high-edge summary | **Rebuild** 成 "Today's Edge": 5 个 football picks with CLV ≥ 3% |
| **M13 LLM Mgmt** | 基础 — 多 provider 配置 | 保留 |
| **M14 LLM Perf** | 中期 — 模型 win-rate per prompt version | 保留, 加 football.v1.0 / market.v1.0 对比 tab |
| **M15 Notifications Center** | 横切 | 保留 |

### 3.2 修改的模块 (P0-P1)

| Module | 改动 |
|---|---|
| **Copy Trading (M5)** | **降级为 P2**。Football copy trading 价值低于 sports trader 直接 trade; 暂保留 stub, 不 active dev |
| **Model Lab (M7)** | **重定位**: 从 "通用 ML training" → "Football model training"。Elo/xG/Poisson 模型训练界面, UI 加 league filter |
| **Trade (M3 增强)** | UI form 加 "market type" radio (1X2 / O/U / Asian Handicap); submission 后用 football.v1.0 prompt 算 recommendation |

### 3.3 移除的模块 (P3)

| Module | 原因 |
|---|---|
| **Polymarket generic tab** | 不再有"按 category filter", 默认只显示 football |
| **Category enum 其他 6 个** (Crypto/Tech/Science/PopCulture/Business/Other) | 在 seed / classify 里直接 hardcode 为 "Skip" 或 filter 掉 |

### 3.4 新增的模块 (P1-P2)

| Module | 优先级 | 描述 |
|---|---|---|
| **`/football` (Football Hub)** | **P1** | 主入口页: future 7 days fixtures + today's edge picks + competition browse |
| **`/football/:competitionId`** | P1 | 单个 competition (Premier League / La Liga / World Cup) 详情: standings + fixtures + 历史 model-vs-market |
| **`/football/fixtures/:matchId`** | P1 | 单场 deep-dive: Elo history / xG trend / Poisson distribution 可视化 / our model prob vs Polymarket prob time-series / framework breakdown 详情 |
| **`/football/edge-board`** | P1 | 全场 edge 排行: 按 `clv_edge` 降序排, filter by competition / market type / 最小 confidence |
| **`/competitions`** | P2 | 比赛管理: 添加 / 移除 tracked competitions |
| **Share Card** | P2 | 每个 edge pick 生成 "可分享图卡" (X / Twitter 优化), 含 team logos + model prob + Polymarket prob + edge% |
| **Football Schedule ETL** | P2 | 后台 cron: 每天 04:00 拉取未来 7 天 fixtures (从 football-data.org / API-Football / polymarket 事件), 写 SQLite |

### 3.5 Naming 决策 — **保持 `polyrocket`,只改 tagline**

**`polyrocket` 不改**, 全部保留 (repo 名 / bundle id / Tauri window title / docs 前缀 / i18n key)。

Football pivot 的产品定位通过以下方式表达:
- **Tagline**: "Football intelligence for Polymarket" (替代之前的通用 tagline)
- **Hero icon**: ⚽ 足球 emoji (替代原 radar icon, 仅 Welcome / onboarding / branding 处用)
- **产品定位文案**: README / docs / Welcome Step 1 / Settings → About 等位置改 football-first 措辞
- **Theme accent**: 改成 pitch green (足球场色) — 详见 UI §1.2

理由:
- 完整 git history / branch / 协作 保持 (用户决策: 不做 rename)
- pivot 的"足球专注"通过定位 + UI 表达, 不通过 rename 表达
- 如果未来产品成熟, 再做 product rebrand (时机: 公开 launch 时)

### 3.6 Tagline + Branding 改动 (v0.119)

低风险, ~10 处文案改动, 不涉及代码 rename:
- `package.json` `description` (从 generic 改 football-focused)
- `README.md` 顶部 hero tagline
- `src/components/layout/AppShell.tsx` logo 旁的 icon (`Radar` → `CircleDot` ⚽, 仅 logo 区)
- `src/main.tsx` `<title>` tag
- i18n key `app.tagline` (新 key)
- `docs/polyrocket-football-prd.md` (本文, v0.119 已写)
- `docs/polyrocket-football-ui.md` (本轮已写)
- `docs/overview.md` 顶部副标题改 football-first
- `docs/coding-spec.md` 同上
- `docs/llm-providers.md` 同上

---

## 4. 核心功能 (MVP)

### 4.1 Football Hub (`/football`)

- **顶部**: 今日 / 明日 / 本周 fixtures 计数, 当前最有 edge 的 3 场比赛
- **中部**: league selector (Premier League / La Liga / Serie A / Bundesliga / Ligue 1 / Champions League / World Cup 2026 / Euro 2024 等)
- **底部**: today edge picks 列表 (sort by `clv_edge` DESC, top 5)

### 4.2 Football Fixtures 详情 (`/football/fixtures/:matchId`)

每场比赛详情页, 5 个 tab:

| Tab | 内容 |
|---|---|
| **Match Info** | 两队 Elo / FIFA ranking / 近期 form (W-D-L) / H2H last 5 / 比赛场地 + 时间 |
| **xG Analysis** | 双方近 5 / 10 / 赛季 xG / xGA / xG per 90 trend, 包含 chi-chart (Opta/StatsBomb style) |
| **Poisson Model** | Dixon-Coles λ_home / λ_away 可视化, 1X2 probability 三柱, O/U probability 分布 |
| **Polymarket Odds** | 该场比赛所有相关 markets (1X2 + O/U + Spread) list, 每个显示 YES price / NO price / liquidity / 24h volume / mid implied prob |
| **Our Model** | LLM 估计 P(YES) vs Polymarket implied prob → CLV edge bar chart (横轴 competition, 纵轴 edge%) |

### 4.3 Edge Board (`/football/edge-board`)

| 列 | 含义 | 来源 |
|---|---|---|
| Match | Home vs Away + 时间 | Polymarket question |
| Competition | League name | 从 question 解析 + lookup table |
| Market Type | TeamWin / Draw / O/U / Handicap | football.v1.0 detection |
| Polymarket YES | YES mid-price cents | Polymarket orderbook |
| Our Model | P(YES) | football.v1.0 framework breakdown |
| Edge | model_prob - polymarket_implied | CLV framework |
| Confidence | model.confidence 0-1 | LLM output |
| Recommended | YES / NO / SKIP | 综合 edge + confidence |
| Last update | 时间戳 | signal.computed_at |

Sortable by Edge (default) / Polymarket YES / Confidence / Volume.
Filter by Competition / Market Type / min confidence / min edge.

### 4.4 自动扫描 + Signal (后台)

- **Cron job**: 每 15 分钟跑一次
- **范围**: Polymarket 上 `category == "football"` 且 `closes_at - now < 7 days` 的市场
- **每个市场**: 调 football.v1.0 prompt (4-framework) → 存 `football_analyses` 表 (复用现有 `llm_analyses` schema, 加 `framework_breakdown` JSON column)
- **Edge 计算**: `edge = model.probability - (yes_price / 100)`
- **Signal 触发条件**: `|edge| >= 0.03 AND confidence >= 0.6 AND volume_24h >= $50k`
- **Signal 推送**: 触发后通过 `X2 Notifications` 推送 desktop notification + 写 `notifications` 表

### 4.5 Daily Brief (`/brief` rebuild)

每天早上 8:00 自动生成今日 edge picks (top 8):
- 标题: "📈 Today's Edge — 8 football picks with model vs market gap"
- 每条: match / competition / market_type / YES price / our prob / edge / recommended action
- 末尾: 一句话总结 "Today's avg edge: +3.2% across 8 picks (model confidence ≥ 0.65)"
- 可 export 为图片 (share card)

---

## 5. 范围外 (Out of Scope)

明确不做:
- **多 bookmaker 比价** (Pinnacle / Bet365 / 1xBet / Asian books) — 数据来源限制, 留 P3
- **传统足球预测榜单 (Football Whispers 风格)** — 内容创作不是产品核心
- **Live in-play betting** — Polymarket 上 in-play 流动性差, 没必要支持
- **Crypto / 政治 / CS2 等其他类别** — 完全 drop
- **Multi-platform sync (mobile / web)** — desktop only, 后续看反馈

---

## 6. 成功指标 (Success Metrics)

### 6.1 用户参与 (Engagement)

| Metric | Target (3 months) | 测量 |
|---|---|---|
| **DAU / WAU** | DAU ≥ 20, WAU ≥ 50 | local telemetry (existing `telemetry` module) |
| **Session duration** | median ≥ 5 min | existing telemetry |
| **/football 页面访问 / 总会话** | ≥ 60% | route-level event counter |

### 6.2 产品价值 (Value)

| Metric | Target (6 months) | 测量 |
|---|---|---|
| **Model Brier score** | ≤ 0.22 (vs Polymarket baseline ~0.25) | `llm_analyses` + market resolution 时算 |
| **CLV positive rate** | ≥ 55% picks positive CLV | `llm_analyses.edge` vs market closing line |
| **平均 edge per pick** | ≥ +3% | aggregated daily |
| **Football.v1.0 win-rate vs market.v1.0** | football.v1.0 胜率 ≥ +5pp | `llm_stats_by_prompt_version` 加 category filter |

### 6.3 技术健康 (Tech Health)

| Metric | Target |
|---|---|
| **Test count** | ≥ 500 cargo + 1200 vitest |
| **Coverage** | stmts 91% / branches 88% / fns 87% / lines 92% |
| **football.v1.0 prompt win-rate sample size** | ≥ 200 resolved analyses (统计显著性) |
| **football.v1.0 prompt cost per call** | ≤ 2x market.v1.0 (justify deeper) |

---

## 7. 路线图 (Roadmap)

| Version | Scope | 周期 |
|---|---|---|
| **v0.119** | 本次 pivot 的命名 + nav 改造 + Markets filter + 1.landing rebrand | 1 round (~1 day) |
| **v0.120** | `/football` hub + `/football/fixtures/:id` 详情页 + Football Schedule ETL (polymarket events + football-data.org) | 2-3 rounds |
| **v0.121** | Edge Board + 自动扫描 cron + signal push | 2 rounds |
| **v0.122** | Daily Brief rebuild + share card export | 1 round |
| **v0.123** | Football-specific Elo/xG 数据源接入 (FBref / clubelo.com) | 2 rounds |
| **v0.124** | Bankroll football-specific 规则 (per-league cap) | 1 round |
| **v0.125** | Polish: 主题色 (pitch green) / team color accents / 空态文案 | 1 round |
| **v0.130** | Beta launch (X / Reddit / r/sportsbook) | — |

---

## 8. 风险 + 缓解

| Risk | 缓解 |
|---|---|
| **Football data 供应商封号 / 收费** | multi-source fallback (FBref → football-data.org → API-Football → 自建) |
| **LLM football reasoning 不准** (LLM hallucinate Elo / xG) | football.v1.0 prompt 明确说 "如果没数据填 null + 降 confidence 0.2", 后端 run real Elo/xG 模型给 LLM, LLM 只做 qualitative reasoning |
| **Polymarket 足球市场流动性下降** | edge 计算不依赖流动性 (用 mid-price, 不用 depth) |
| **法律风险** (某些地区禁止 sports betting tools) | app 内不集成 bookmaker API, 只分析 Polymarket; 添加 disclaimer: "This is a research tool, not a betting platform. Polymarket is a prediction market, not a bookmaker." |
| **Pivot 后用户流失** (如果现有用户用 CS2 / Politics) | 保留 fallback: Settings 加 "Show non-football markets" toggle (default off), `markets_list` 加 filter 切换 |

---

## 9. 决策点 (Decision Points)

⚠️ 用户需要确认的设计决策:

1. **Naming**: ✅ **保持 `polyrocket` 不改** (用户决策 2026-06-22)。Football pivot 通过 tagline + UI 表达, 不通过 rename 表达。
2. **Markets filter default**: 默认 football-only? / 默认 all + 用户手动 filter? (前 30 天数据, 看哪边更受欢迎)
3. **Landing page 默认**: `/football` (新 hub)? / `/dashboard` (现有)? / `/brief` (today's edge)?
4. **Codex / new-competitions**: 6 个非 football category (Crypto/Tech/Science/PopCulture/Business/Other) 完全 drop 还是保留 in DB 但 hide?
5. **Share card**: v0.122 内置? / P3 后做?

详见 [`polyrocket-football-ui.md`](./polyrocket-football-ui.md) §0 决策清单。

---

## 10. 一句话 elevator pitch

> polyrocket: 用 4 个学术界验证过的足球模型(Dixon-Coles 双 Poisson + Elo + xG + Pinnacle CLV),跟 Polymarket 上的足球盘口实时对比,5 秒告诉你"模型觉得这场阿根廷 67%,市场觉得 65%,edge +2%,推荐 YES"。
