# polyrocket — 功能模块设计 (FSD)

> 版本：v2.0 · 2026-06-16 (v0.5 — 通知 + mirror + signed-order)
> 配套：[`overview.md`](./overview.md)（5 层架构 + 目录结构） · [`polyrocket-llm-analysis.md`](./polyrocket-llm-analysis.md)（M10/M12 设计） · [`polyrocket-llm-management.md`](./polyrocket-llm-management.md)（M11 设计） · [`polyrocket-flows.md`](./polyrocket-flows.md)（20 个 flow） · [`polyrocket-ui-design.md`](./polyrocket-ui-design.md)（18 页 × 3 主题 UI 规范）
> 范围：Tauri 2 桌面客户端的所有功能模块拆解，含职责、依赖、对外接口
> 想了解**代码在哪一层、目录怎么组织** → 看 `overview.md`

---

## 0. 顶层架构（v0.2 含 M10 / M12）

```
┌────────────────────────────────────────────────────────────────────┐
│                    polyrocket Desktop (Tauri 2)                     │
│                                                                     │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │                   Frontend (React 18 + TS)                    │  │
│  │                                                                │  │
│  │   Dashboard · Markets · Signals · Copy · PnL · ModelLab       │  │
│  │   Analysis (v0.2 NEW) · LLM Performance (v0.2 NEW)            │  │
│  │   AppShell (collapsible Sidebar + TopBar) · Theme (3 themes)  │  │
│  │                                                                │  │
│  │   ┌─────────────────────┐    ┌─────────────────────────┐      │  │
│  │   │ UI Components (24+) │    │ Data Layer              │      │  │
│  │   │ shadcn/ui base      │    │ TanStack Query          │      │  │
│  │   │ Recharts · Tables   │    │ zustand (theme, ui)     │      │  │
│  │   │ lucide · cmdk       │    │ Drizzle (TS schema)     │      │  │
│  │   └─────────────────────┘    └─────────────────────────┘      │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                              │ IPC (invoke / event)                  │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │                    Backend (Rust 1.77+)                       │  │
│  │                                                                │  │
│  │   18 IPC commands · sqlx · keyring · reqwest · tracing        │  │
│  │                                                                │  │
│  │   ┌─────────┐  ┌─────────┐  ┌──────────┐  ┌──────────────┐    │  │
│  │   │ Markets │  │ Signals │  │   Bets   │  │  CopyTrading │    │  │
│  │   │ module  │  │ module  │  │  module  │  │   module     │    │  │
│  │   └─────────┘  └─────────┘  └──────────┘  └──────────────┘    │  │
│  │   ┌─────────┐  ┌─────────┐  ┌──────────┐  ┌──────────────┐    │  │
│  │   │ Wallets │  │   PnL   │  │ ModelLab │  │  LLMAnalysis │    │  │
│  │   │ module  │  │ module  │  │  module  │  │   (M10 NEW)  │    │  │
│  │   └─────────┘  └─────────┘  └──────────┘  └──────────────┘    │  │
│  │                          │                                     │  │
│  │   ┌──────────────────────▼─────────────────────┐              │  │
│  │   │      SQLite (14 tables, WAL)               │              │  │
│  │   └────────────────────────────────────────────┘              │  │
│  └──────────────────────────────────────────────────────────────┘  │
│          │                          │                              │
│          │ HTTPS                    │ HTTPS (parallel, v0.2)       │
│   ┌──────▼────────┐         ┌───────▼─────────────────┐           │
│   │ Polymarket API│         │ 4 LLM Providers         │           │
│   │ Gamma + CLOB  │         │ OpenAI / Anthropic /     │           │
│   └───────────────┘         │ Google / DeepSeek / ...  │           │
│                             └─────────────────────────┘           │
└────────────────────────────────────────────────────────────────────┘
```

---

## 1. 模块清单

polyrocket 拆为 **10 个功能模块 + 2 个横切关注点**：

| # | 模块 | 类型 | 主要职责 | 版本 |
|---|---|---|---|---|
| M1 | Markets | 业务 | 市场元数据采集、分类、缓存、查询 | v0.1 |
| M2 | Signals | 业务 | 信号生成、模型输出、置信度评分、过期管理 | v0.1 |
| M3 | Bets | 业务 | 下单（A 跳转 / B 签名）、持仓、结算、P&L 记账 | v0.1 |
| M4 | Wallets | 业务 | 钱包注册、地址簿、OS keyring 集成 | v0.1 |
| M5 | CopyTrading | 业务 | 目标地址监控、订单镜像、事件匹配 | v0.1 |
| M6 | PnL | 业务 | 聚合统计：总权益、胜率、Brier、日 P&L、按分类 | v0.1 |
| M7 | ModelLab | 业务 | 模型版本管理、训练触发、回测、性能追踪 | v0.1 |
| M8 | Dashboard | 业务（前端聚合） | 跨模块数据汇总视图 | v0.1 |
| M9 | Settings | 业务 | 用户偏好、主题、阈值、API key（可选） | v0.1 |
| **M10** | **LLM Analysis** | **业务** | **多 LLM 并行分析、prompt 工程、共识聚合、用户决策追踪、LLM 胜率统计** | **v0.2** |
| **M11** | **LLM Management** | **业务** | **Provider / Key 配置（OS keyring 持久化）+ 连通性测试 + 流量监控 + 健康探针 + 配额 + 异常告警** | **v0.2** |
| **M12** | **Daily Brief** | **业务** | **每日自动分析今日值得关注的市场，评分 + 缓存 + 用户偏好** | **v0.2** |
| **M13** | **Onboarding** | **业务** | **首启 4 步引导（welcome → theme → wallet → LLM keys）+ Market Detail + Notifications + Help + Trade History 入口** | **v0.2** |
| X1 | AuditLog | 横切 | 所有写操作的可追溯记录 | v0.1 |
| X2 | Notifications | 横切 | Toast / 系统通知 / 重要事件推送 | v0.1 |

**详细 M10 / M11 / M12 设计**：见 [`polyrocket-llm-analysis.md`](./polyrocket-llm-analysis.md) + [`polyrocket-llm-management.md`](./polyrocket-llm-management.md)。

## 2. 模块详细设计

### M1 — Markets

**职责**：管理 Polymarket 市场元数据，本地缓存 + 增量同步。

**核心实体**：`markets` 表（id, slug, question, category, end_date, active, resolved, outcome, liquidity, volume_24h, ...）

**关键能力**：
- 增量拉取活跃市场（Gamma API `/markets?active=true`）
- 按 category 过滤（football / cs2 / politics）
- 按 end_date 排序（即将关闭优先）
- 缓存 TTL：5 分钟
- 软删除（`active=false` 而非真删）

**依赖**：外部 — Polymarket Gamma API；内部 — 无

**对外接口**（IPC 命令）：
```ts
list_markets({ category?, active_only?, limit? }): MarketDto[]
sync_markets(): number   // 返回 upsert 条数
```

---

### M2 — Signals

**职责**：管理预测模型的输出。包含生成、过期、重新计算、查询。

**核心实体**：`signals` 表（id, market_id, computed_at, model_version, predicted_prob, market_prob, edge, confidence, horizon_hours, rationale, active）

**关键能力**：
- 周期重算（默认 5 分钟，可在 Settings 调）
- 边缘过滤（默认 |edge| ≥ 5% 才标 `active=1`）
- 解释快照（`rationale` JSON：模型特征贡献 top-K）
- 历史保留（每 24h 降采样到 `signal_snapshots_daily` 表）

**依赖**：M1（markets）；可选 — M7（model-lab 触发器）

**对外接口**：
```ts
list_active_signals({ min_edge?, category?, limit? }): SignalDto[]
recompute_signals(): number
```

**MVP 简化**：v0.1 的 signals 表由前端 mock 数据驱动（已在 Dashboard.tsx 体现），model-lab 集成在 v0.2。

---

### M3 — Bets

**职责**：下单、持仓跟踪、结算、模式 A/B 决策。

**核心实体**：`bets` 表（id, wallet_id, market_id, signal_id, mode, side, size, price, shares, placed_at, settled_at, pnl, status, tx_hash, notes）

**关键能力**：
- **Mode A (Jump)**：构建 `https://polymarket.com/event/{slug}?side=...&price=...` URL，前端 `shell.open()` 调起系统浏览器。零合规风险。
- **Mode B (Signed)**：从 OS keyring 读私钥 → 签名 CLOB 订单 → 提交 → 落库 → audit_log。v0.1 仅占位（audit_log 写入路径已通），v0.2 接入 `rs-clob-client`。
- **手动下单**：不关联 signal 的纯手动记录。
- **结算监听**：每 60s 检查活跃市场是否 `resolved=true`，若是则根据 outcome 更新 `bets.status`（won/lost）+ `pnl`。

**依赖**：M1（markets）；M4（wallets）；X1（audit_log）

**对外接口**：
```ts
place_jump_link(args): string   // 返回 URL
place_signed_order(args): BetDto
list_bets({ status?, wallet_id?, limit? }): BetDto[]
```

**状态机**：

```
[pending] ──place_jump──▶ [pending_external] ──manual confirm──▶ [open]
   │                                                                │
   │ place_signed                                                   ▼
   ▼                                                             [won] | [lost] | [cancelled]
[open] ◀──── signal-driven entry
```

---

### M4 — Wallets

**职责**：钱包地址簿 + OS keyring 集成。

**核心实体**：`wallets` 表（id, address, label, chain_id, wallet_type, created_at, last_synced_at）

**关键能力**：
- 添加 EOA / Smart Wallet 地址
- 标签管理（"primary"、"trade-1"、"cold"）
- 链 ID 校验（默认 Polygon mainnet = 137）
- **私钥管理**：通过 `keyring` crate 写入 macOS Keychain / Windows Credential Manager / Linux Secret Service。**前端永远接触不到私钥明文**。
- 别名映射：`{alias: "primary"}` → `{address: "0x..."}`，签名时按 alias 取。

**依赖**：OS Keyring；M3（bets 引用 wallet_id）

**对外接口**：
```ts
list_wallets(): WalletDto[]
add_wallet(args): WalletDto
```

**Rust 内部 API**（不暴露 IPC）：
```rust
keyring::set_key(alias, secret) -> Result<()>
keyring::get_key(alias) -> Result<String>
keyring::delete_key(alias) -> Result<()>
```

---

### M5 — CopyTrading

**职责**：跟踪一组目标地址，把它们的成交动作解析为可分析事件，部分触发镜像下单。

**核心实体**：`copy_targets` 表（address, label, enabled, allocation_cap, min_edge）+ `copy_events` 表（target_id, market_id, detected_at, side, size, price, tx_hash）

**关键能力**：
- 监控模式：后台轮询（默认 30s）目标地址的最新 fill 事件（通过 Polymarket Data API 或自建 indexer）
- 信号提取：解析 fill → 提取 side / size / price → 与本地 markets/signal join
- 镜像模式：可选。开启后 fill 触发新 signal，按 allocation_cap 计算 size，落 `bets`（mode 自动 B_signed）
- 去重：`tx_hash` UNIQUE
- 镜像策略可在 Settings 配置：max_allocation / min_edge / pause_on_drawdown

**依赖**：M1, M2, M3, M4

**对外接口**：
```ts
list_copy_targets(): CopyTargetDto[]
add_copy_target(args): CopyTargetDto
recent_copy_events({ target_id?, limit? }): CopyEventDto[]
```

---

### M6 — PnL

**职责**：聚合统计所有交易结果，给 Dashboard 和 PnL 页供数。

**核心实体**：`bets`（源）+ 衍生查询

**关键能力**：
- 总权益 = sum(open.size) + sum(closed.pnl)
- 30D 胜率 = won / (won + lost)
- Brier Score（聚合）= 最近 `model_performance` 记录
- 按 category 切：football / cs2 / politics 三维 PnL
- 按 model_version 切：哪个模型版本贡献了最多收益

**依赖**：M3（bets）；M7（model_performance）

**对外接口**：
```ts
dashboard_kpis(): DashboardKpis  // 总权益、open PnL、胜率、Brier、active signals、open positions
// 后续：
pnl_by_category(): CategoryPnL[]
pnl_curve(): PnLCurvePoint[]
```

---

### M7 — ModelLab

**职责**：模型版本管理、性能追踪、再训练触发。

**核心实体**：`model_performance` 表（model_version, category, window_start, window_end, n_predictions, brier_score, log_loss, win_rate, avg_edge, calibration JSON）

**关键能力**：
- 模型注册：每次 `recompute_signals` 后写一条 performance 快照
- 按窗口（7d / 30d / all）聚合
- Calibration 数据点：每 10% 概率桶的实际频率
- v0.2：触发 Python sidecar 跑训练（feature extraction + gradient boosting）

**依赖**：M2（signals）；M3（bets — 用于实际结果回填）

**对外接口**：
```ts
// v0.1 占位
// v0.2+：
list_model_versions(): ModelVersionDto[]
trigger_training({ category?, from?, to? }): TrainingJobDto
model_performance({ model_version, window }): ModelPerformanceDto
```

---

### M8 — Dashboard（前端聚合视图）

**职责**：跨模块数据汇总的第一屏。

**页面元素**（详见 UID 文档）：
- 4 张 KPI 卡（Equity / Open P&L / Win Rate / Brier）
- Equity Curve 图（PnL 模块供数）
- Calibration 图（ModelLab 供数）
- Active Signals 表（Signals 模块供数）
- Open Positions 列表（Bets 模块供数）
- Recent Activity 时间线（AuditLog + Bets 联合）

**依赖**：M1-M6 的查询接口

**对外接口**（前端路由）：
```
/dashboard
```

---

### M9 — Settings

**职责**：用户偏好管理。

**关键能力**：
- 主题：dark / light / matrix
- 默认 edge 阈值：默认 5%
- 默认 allocation cap（每个仓位最大 USDC）
- copy trading 启用 / 暂停
- 通知：toast / 系统通知 / 关
- 数据：清空本地 cache、导出全部数据为 JSON

**依赖**：跨模块（M1-M5 写操作都可能受阈值影响）

**对外接口**（前端路由）：
```
/settings
```

**当前实现**（v0.1）：仅主题切换 + 部分阈值显示在 Sidebar 底部，其他留到 v0.2。

---

## 3. 横切关注点

### X1 — AuditLog

**职责**：所有写操作的可追溯记录。

**核心实体**：`audit_log` 表（at, actor, action, target, payload JSON, result）

**写入点**：
- `add_wallet` / `add_copy_target`（资源创建）
- `place_signed_order`（关键操作）
- `set_key` / `get_key`（密钥访问）
- `recompute_signals`（计算触发）
- `update_settings`（配置变更）

**字段规范**：
- `actor`: `'user' | 'system' | 'tauri:<cmd_name>'`
- `action`: `'<module>.<verb>'`，例如 `'bet.place'`、`'wallet.add'`
- `result`: `'ok'` 或 `'error:<error_code>'`

**不记录**：纯查询（`list_*`）和内部状态变更。

### X2 — Notifications

**职责**：toast + 系统通知。

**关键能力**：
- 重要事件：新 signal 触发、订单成交、模型重算完成
- 错误事件：sync 失败、订单失败、keyring 访问拒绝
- 等级：info / warning / error
- 通道：app toast + 系统通知（macOS Notification Center / Windows toast）

**v0.1 实现**：仅 toast（前端组件 + zustand store），系统通知留到 v0.2。

---

## 4. 模块依赖图

```
                            ┌──────────────┐
                            │  Dashboard   │
                            │   (前端)     │
                            └──────┬───────┘
                                   │ query
        ┌──────────┬───────────────┼───────────────┬──────────┐
        ▼          ▼               ▼               ▼          ▼
    ┌──────┐  ┌────────┐  ┌──────────────┐  ┌──────────┐  ┌──────┐
    │Markets│  │Signals │  │     PnL      │  │  Copy    │  │Bets  │
    └──┬───┘  └───┬────┘  └──────┬───────┘  └────┬─────┘  └──┬───┘
       │          │              │               │           │
       │          │              │               │           │
       └──────────┴──────┬───────┴───────────────┘           │
                         │                                   │
                         ▼                                   ▼
                    ┌─────────┐                       ┌──────────┐
                    │ ModelLab│                       │ Wallets  │
                    └────┬────┘                       └────┬─────┘
                         │                                │
                         │     ┌──────────────┐          │
                         └────►│ LLM Analysis │◄─────────┘
                               │   (M10)      │
                               └──────┬───────┘
                                      │
                                      ▼
                              ┌──────────────┐
                              │ OS Keyring   │
                              │ (LLM keys)   │
                              └──────────────┘

  ┌─────────────────────────────────────────────────────────────────┐
  │                  X1: AuditLog (all writes)                       │
  │                  X2: Notifications (events)                      │
  └─────────────────────────────────────────────────────────────────┘
```

---

## 5. 数据所有权

| 表 | 写权限 | 读权限 |
|---|---|---|
| `wallets` | M4 | M3, M5, M9 |
| `markets` | M1（sync） | M2, M3, M5, M6, M8 |
| `orderbook_snapshots` | M1（sync 旁路） | M2, M3 |
| `ticks` | M1（sync） | M2 |
| `signals` | M2（recompute） | M3, M5, M6, M8, M9 |
| `model_performance` | M7 | M6, M8, M9 |
| `bets` | M3, M5（镜像） | M3, M6, M8, M9, X1 |
| `copy_targets` | M5 | M5, M9 |
| `copy_events` | M5（detector） | M5, M8, M9 |
| `audit_log` | X1（任意写操作） | M9, X1（管理员视图） |

---

## 6. 版本路线

| 版本 | 模块覆盖 |
|---|---|
| v0.1 | M1 / M2（mock）/ M3（Mode A + Mode B 占位）/ M4 / M5（基础）/ M6 / M8 / M9（主题） |
| v0.2 | M2 真信号接入 / M3 Mode B 真签名 / M5 镜像策略 / M7 模型性能追踪 / X2 系统通知 |
| v0.3 | 5-layer 严格分目录 + CI 守门 (check-layers.mjs) |
| v0.4 | **L3 域全实现** + **L1 真 React 化 18 路由** + audit read-side + onb/notify/help |
| **v0.5 (当前)** | **vitest 88 + mirror 状态机 + 系统通知 + signed-order stub** |
| v0.6 (next) | M3 Mode B 真签名 (rs-clob-client) / M5 镜像自动执行 / M7 Python sidecar |

### 6.1 v0.4 — 全 13 模块

| 模块 | 状态 | 关键文件 |
|---|---|---|
| M-Foundation | ✅ done | src/ipc.ts (41) + 13 组件 + types/ + 2 stores |
| M1 Markets | ✅ 真实现 | domain/polymarket + /markets + /markets/:id |
| M2 Signals | ✅ 真实现 | domain/signal + /signals |
| M3 Bets | ✅ 真实现 | domain/bet + /history |
| M4 Wallets | ✅ 真实现 | domain/wallet + /wallets |
| M5 Copy | ✅ 真实现 | domain/copy + /copy |
| M6 PnL | ✅ 真实现 | domain/pnl + /pnl |
| M7 ModelLab | ✅ 真实现 | domain/lab + /lab |
| M8 Dashboard | ✅ 重写 | /dashboard (consumes M1-M7) |
| M9 Settings | ✅ 真实现 | /settings + prefs-store |
| M10 LLM Analysis | ✅ 真实现 | /analysis (M10 Rust 已实) |
| M11 LLM Mgmt | ✅ 真实现 | /llm-mgmt + /llm-perf |
| M12 Daily Brief | ✅ 真实现 | /brief (M12 Rust 已实) |
| M13 Onboarding | ✅ 真实现 | /onboarding + onb-store |
| X1 Audit | ✅ 增 read-side | commands/audit.rs + /audit |
| X2 Notifications | ✅ toast UI | /notifications + toast-store |
| Help | ✅ static | /help |

### 6.2 v0.5 — 测试 + 通知 + mirror + signed-order stub

| 阶段 | 内容 | commit | tests |
|---|---|---|---|
| v0.5a | TS domain mirror (8 modules) + vitest 4.1.9 | `2c28dfd` | +88 vitest |
| v0.5b | M5 mirror auto-trigger (MirrorStatus + MirrorPanel) | `c188430` | +4 rust |
| v0.5c | X2 系统通知 (tauri-plugin-notification) | `37b2b2f` | +10 rust |
| v0.5d | M3 signed-order stub (validate + djb2 tx_hash) | `46f107f` | +9 rust |
| **Total v0.5** | **+111 tests, 4 commits, 1 new Cargo dep, 1 new L1 component, 1 new L3 module** |

新增模块：
- L3 `domain/notify` (NotificationKind + 5 payload builders + should_send)
- L2 `commands/notify` (send_notification / request_permission / permission_state)
- L1 `src/lib/domain/*` (8 modules mirroring L3 pure funcs)
- L1 `src/components/business/MirrorPanel.tsx` (5-stat panel + per-row state)
- vitest 4.1.9 + 8 .test.ts files

---

## 变更日志

- **v1.6** (2026-06-16) — M10 真实化：4-provider HTTP fan-out 落地。新增 5 个 provider client（OpenAI / Anthropic / Google / DeepSeek / Custom）+ key rotation + retry/backoff + 3 prompt 模板 + 2 个新 IPC（llm_get_recommendation / llm_list_analyses）。代码量 ~1200 行 + 8 个测试（3 unit + 5 integration）。
- **v1.8** (2026-06-16) — UI v2.1：18 页面 × 3 主题 = 54 PNG 截图（`docs/previews/{dark,light,matrix}/*.png`）通过 `scripts/snapshot_pages.py` 一键生成。修复 `prototype.html` 中 `const SIGNALS` mock data 移位 bug（导致 dashboard 渲染空白）。UI spec 加 Feature Coverage Matrix 验证 17 module 100% 覆盖。
- **v1.7** (2026-06-16) — M11 后台调度：3 个 tokio loop（health probe 5min / daily brief 00:00 UTC cron / anomaly detect 60min）+ 3-fail auto-disable + 3 个新 IPC（scheduler_status / scheduler_run_health_probe_now / scheduler_run_daily_brief_now）。
- **v1.5** (2026-06-16) — 新增 **M13 Onboarding**：首启 4 步引导。补强 prototype.html 6 个新页面（Onboarding / Market Detail / Notifications / Help / Trade History / Audit Log）；UI spec v2.0 升级到 18 页面 + 完整 tokens/状态机/动效/a11y/图标/数据可视化/错误边界/i18n/Empty-Loading-Error 规范。
- **v1.4** (2026-06-16) — M11 升级：**Client-side key persistence 为主路径**。新增 5 个 IPC（llm_key_set_secret / llm_pm_set_credentials / llm_pm_clear_credentials / polyrocket_wallet_set_pk / polyrocket_wallet_clear_pk / secrets_status）；.env 降级为 dev-only（仅 `POLYROCKET_ENV=dev` 读）；keyring alias 命名空间化（`llm/<pid>/<alias>` 等）。
- **v1.3** (2026-06-16) — 新增 **M11 LLM Management**：provider/key CRUD + 连通性测试 + 流量监控 + 健康探针 + 配额。新增 3 张表（llm_provider_keys / llm_call_logs / llm_health_checks）+ 扩 llm_providers（11→27 列）。IPC 命令 +12（27→39）。
- **v1.2** (2026-06-16) — 新增 **M12 Daily Brief**：每日自动评分 + 缓存 + 用户偏好；markets 加 user_interested + brief_dismissed_at 字段；新增 daily_briefs / user_brief_prefs 2 张表。IPC 命令 +4。
- **v1.1** (2026-06-16) — 新增 **M10 LLM Analysis**：多 LLM 并行 + prompt 工程 + 共识 + 用户决策追踪 + LLM 胜率统计。依赖图 + IPC 命令数 13→18。
- **v1.0** (2026-06-16) — 初版。9 模块 + 2 横切，基于已实现代码（Drizzle schema + 13 IPC + AppShell）反向整理。