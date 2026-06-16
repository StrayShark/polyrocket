# polyrocket — Architecture Overview

> 项目架构分层设计 / 模块清单 / 目录结构 / 数据流 / 迁移路线
>
> 版本：v1.0 · 2026-06-16
> 配套：[`polyrocket-modules.md`](./polyrocket-modules.md)（17 个 module 业务说明） · [`polyrocket-flows.md`](./polyrocket-flows.md)（20 个交互流程） · [`polyrocket-ui-design.md`](./polyrocket-ui-design.md)（18 页面 × 3 主题 UI 规范）
> 强约束：[`polyradar-dev-governance.md §11`](../polyradar-dev-governance.md) — 三主题仅配色差异；`.env` 仅 dev 用途；OS keyring 是秘密唯一存储

---

## 0. 设计目标

polyrocket 是一个 Tauri 2 desktop 客户端（Rust 后端 + React 前端 + 嵌入式 SQLite）。本文件定义 **5 层架构**，使代码、文档、测试、CI 校验能按统一规则组织。

| 维度 | 目标 |
|---|---|
| **可理解性** | 一个新工程师 30 分钟内能找到任何代码的位置（按层定位） |
| **可测试性** | L3 (domain) 完全独立于 UI / IPC / DB，可纯函数单测 |
| **可演进性** | 加新 LLM provider、加新数据源、加新页面**不**需要改其他层 |
| **可治理性** | doc-sync / check 脚本能按层枚举（CI 防 regression） |
| **可移植性** | 未来从 Tauri 迁到其他壳（如 native webview / mobile）只需换 L1+L2 |

---

## 1. 5 层架构总览

```mermaid
graph TB
    subgraph P["Presentation Layer (L1)"]
        direction TB
        P1[React 18 + Vite + Tailwind]
        P2[18 routes · 32 components]
        P3[Zustand stores · hooks · IPC client]
    end

    subgraph A["Application Layer (L2)"]
        direction TB
        A1[commands/ — 39 IPC handlers]
        A2[state — AppState]
        A3[scheduler — manual triggers]
    end

    subgraph D["Domain Layer (L3)"]
        direction TB
        D1[llm — clients + dispatch + prompts + cost]
        D2[consensus — weighted median]
        D3[polymarket — CLOB client]
        D4[business logic (signal/bet/copy/pnl)]
    end

    subgraph I["Infrastructure Layer (L4)"]
        direction TB
        I1[db — SQLite pool + settings]
        I2[http — reqwest shared client]
        I3[error — AppError + AppResult]
        I4[scheduler — 3 tokio loops]
    end

    subgraph PL["Platform Layer (L5)"]
        direction TB
        PL1[keyring — macOS Keychain / Win Cred Mgr / Linux SS]
        PL2[env — POLYROCKET_* loader]
        PL3[paths — ~/Library/Application Support/...]
    end

    P -->|invoke| A
    A --> D
    A --> I
    D --> I
    D --> PL
    I --> PL
    I --> P
    style P fill:#007ACC,color:#fff
    style A fill:#4EC9B0,color:#fff
    style D fill:#DCDCAA,color:#000
    style I fill:#858585,color:#fff
    style PL fill:#F48771,color:#fff
```

### 1.1 各层一句话职责

| 层 | 职责 | 答的问题 |
|---|---|---|
| **L5 Platform** | OS 集成：keychain、env、文件系统路径 | "密钥/配置/数据**存在哪**？" |
| **L4 Infrastructure** | 持久化、网络、错误、调度 | "怎么**存** + 怎么**调**外部？" |
| **L3 Domain** | 业务核心：LLM 客户端、共识算法、业务规则 | "业务**怎么算**？" |
| **L2 Application** | 用例编排：IPC handlers、命令处理 | "UI 说一句话，后端**做什么**？" |
| **L1 Presentation** | UI 渲染、路由、状态、IPC 客户端 | "用户**看到**什么？**按**什么？" |

### 1.2 依赖规则（**严格单向**）

```
L1 → L2 → L3 → L4 → L5
```

| 规则 | 含义 | 例外 |
|---|---|---|
| **上层调下层** | 任何上层可调下层接口 | — |
| **下层不调上层** | L3 不知道 L1/L2 存在 | 唯一例外：`db` 返 Result 向上抛 |
| **同层不互相调** | L2 各 module 不直接调 L2 其他 module | 走 L3 (domain) 共享 |
| **L3 不调 L2** | domain 不知道 IPC / UI 存在 | 否则 domain 不可单测 |
| **L5 不可调 L1-L4** | platform 是叶子 | 调 = 循环依赖 |

**CI 校验**：用 `cargo metadata --format-version 1` + 自定义脚本扫 `mod xxx { ... }` 引用方向，违反即 fail。

---

## 2. 各层核心模块

### 2.1 Platform Layer (L5) — `src-tauri/src/platform/`

| Module | 路径 | 职责 | 关键 API |
|---|---|---|---|
| `keyring` | `platform/keyring/` | OS keychain 读写（mac/win/linux 三套 native impl） | `get_key(alias)`, `set_key(alias, secret)`, `delete_key(alias)`, `has_key(alias)`, `llm_alias()`, `wallet_alias()`, `pm_api_alias()` |
| `env` | `platform/env.rs` | 读 `POLYROCKET_*` 环境变量 + dev-mode `.env` 同步 | `maybe_load_dev_env()`, `sync_env_to_keyring(pairs)`, `parse_env_file(path)` |
| `paths` | `platform/paths.rs` | 解析 app data dir / temp / log 路径 | `app_data_dir()`, `db_path()`, `log_dir()` |

**关键不变量**：
- **唯一**允许调 OS API 的层；任何 L3 / L4 逻辑需要"存秘密"必须走 `keyring` builder
- `keyring` 是**唯一**接受 secret 字符串的入口；不在签名里暴露 secret
- env vars 命名空间：`POLYROCKET_<DOMAIN>_<KEY>`，**不**用 `POLYROCKET_*` 通用

### 2.2 Infrastructure Layer (L4) — `src-tauri/src/infra/`

| Module | 路径 | 职责 | 关键 API |
|---|---|---|---|
| `db::pool` | `infra/db/pool.rs` | SQLite 连接池 + WAL + FK + 初始化 | `init_pool(app_handle) -> SqlitePool` |
| `db::settings` | `infra/db/settings.rs` | `_polyrocket_settings` k/v 读写 | `get_setting(pool, k)`, `set_setting(pool, k, v)` |
| `http` | `infra/http/mod.rs` | 共享 `reqwest::Client`（连接池） | `new_http_client()` (process-singleton via `OnceCell`) |
| `error` | `infra/error.rs` | 统一 `AppError` + `AppResult<T>` | `AppError::Invalid`, `AppError::Auth`, `AppError::Internal`, `AppError::NotFound` |
| `state` | `infra/state.rs` | Tauri managed state（db pool + scheduler handle） | `AppState { db: SqlitePool }` |
| `scheduler` | `infra/scheduler.rs` | 3 个 tokio loop（health probe / daily brief / anomaly detect） | `start(pool, http) -> SchedulerHandle` |

**关键不变量**：
- 所有 SQL 都走 `sqlx::query` / `sqlx::query_as`，**不**直接用 `rusqlite`
- L4 不知道任何 module 业务名（`llm_*` / `bet_*` 都不在 L4 出现）
- 错误统一 `AppError`，**不**让 sqlx / reqwest 错误直接冒泡到 L2

### 2.3 Domain Layer (L3) — `src-tauri/src/domain/`

| Module | 路径 | 职责 | 关键类型/函数 |
|---|---|---|---|
| `llm::traits` | `domain/llm/mod.rs` | LLM 抽象 + 公共类型 | `LlmClient` trait, `CallRequest`, `CallOutcome`, `CallError`, `ProviderKind`, `CostRate`, `err::*` |
| `llm::clients::openai` | `domain/llm/clients/openai.rs` | OpenAI chat_completions | `OpenAIClient { api_base }` |
| `llm::clients::anthropic` | `domain/llm/clients/anthropic.rs` | Anthropic Messages API | `AnthropicClient::with_base(api_base)` |
| `llm::clients::google` | `domain/llm/clients/google.rs` | Google generateContent | `GoogleClient::with_base(api_base)` |
| `llm::clients::deepseek` | `domain/llm/clients/deepseek.rs` | DeepSeek (OpenAI compat + R1 max_tokens) | `DeepSeekClient::new()` |
| `llm::clients::custom` | `domain/llm/clients/custom.rs` | OpenAI/Anthropic 兼容代理 | `CustomClient::new_openai_compat()`, `::new_anthropic_compat()` |
| `llm::clients::common` | `domain/llm/clients/common.rs` | 共享 body builder + response parser | `build_body()`, `parse_response()`, `classify_status()` |
| `llm::dispatch` | `domain/llm/dispatch.rs` | key 轮询 + retry/backoff | `dispatch()`, `KeyHandle`, `RetryPolicy`, `DispatchOutcome`, `CallLog` |
| `llm::prompts` | `domain/llm/prompts.rs` | 3 个 prompt 模板 + JSON 解析 | `build_market_analysis_request()`, `parse_recommendation()`, 3 个 `PROMPT_VERSION_*` 常量 |
| `llm::cost` | `domain/llm/cost.rs` | 成本计算 | `CostRate::compute()` |
| `consensus` | `domain/consensus.rs` | 4-LLM 共识算法 | `consensus_from(recs) -> (prob, side, conf)` (weighted median) |
| `polymarket` | `domain/polymarket.rs` | Polymarket CLOB REST 客户端 | `place_order()`, `get_market()`, `get_orderbook()` |
| `signal` | `domain/signal.rs` | 信号生成 / 评分 / 过期 | `compute_signal()`, `expire_old_signals()` |
| `bet` | `domain/bet.rs` | 下单 / 结算 / P&L | `place_bet_mode_a()`, `place_bet_mode_b()`, `settle_bet()` |
| `copy` | `domain/copy.rs` | 目标地址监控 + 镜像 | `poll_target(addr)`, `mirror_order(order)` |
| `pnl` | `domain/pnl.rs` | 聚合统计 | `daily_pnl()`, `win_rate()`, `brier_score()` |
| `lab` | `domain/lab.rs` | 模型版本管理 | `train_job()`, `promote_model()` |
| `wallet` | `domain/wallet.rs` | 钱包业务规则 | `validate_pk()`, `derive_address()` |

**关键不变量**：
- **L3 完全可单测**：所有逻辑可纯函数化（不依赖 reqwest / Tauri / sqlx 实例）
- L3 接口**不**返回 `serde_json::Value`（用 typed struct）
- L3 错误用 `Result<T, AppError>`（不引入第三方错误类型）
- L3 模块**不**互相 import 跨子域（`llm` 不引 `signal`）

### 2.4 Application Layer (L2) — `src-tauri/src/commands/`

L2 是**用例编排**：每个 IPC handler 是"一句话办一件事"，负责：
1. 验证 + 解析入参
2. 查 / 写 DB (L4 `db::pool`)
3. 调 domain 函数 (L3)
4. 写 audit_log (L4 `audit_log` 表)
5. 返回 DTO

| Module | 路径 | 职责 | IPC 数量 |
|---|---|---|---|
| `wallet` | `commands/wallet.rs` | M4 钱包 CRUD | 2 |
| `market` | `commands/market.rs` | M1 市场查询 + sync | 2 |
| `signal` | `commands/signal.rs` | M2 信号查询 + 重算 | 2 |
| `bet` | `commands/bet.rs` | M3 mode A/B 下单 + 持仓 | 3 |
| `copy` | `commands/copy.rs` | M5 目标 + 事件 | 3 |
| `pnl` | `commands/pnl.rs` | M6 dashboard_kpis | 1 |
| `lab` | `commands/lab.rs` | M7 模型版本 | 1 (planned) |
| `llm` | `commands/llm.rs` | M10 llm_analyze (4-provider fan-out) + stats | 11 |
| `llm_mgmt` | `commands/llm_mgmt.rs` | M11 provider/key CRUD + test + traffic + stats | 12 |
| `secrets` | `commands/secrets.rs` | M11 PM CLOB 凭证 + wallet pk | 5 |
| `scheduler` | `commands/scheduler.rs` | 手动触发 health/brief | 3 |
| `brief` | `commands/brief.rs` | M12 Daily Brief get/dismiss/refresh | 4 |

**关键不变量**：
- 每个 handler **必须**写 `audit_log`（横切 X1）
- 每个 handler **必须**在 `state: State<'_, AppState>` 取 db pool
- handler **不**直接做业务计算，只编排

### 2.5 Presentation Layer (L1) — `src/`

| Module | 路径 | 职责 | 关键 API |
|---|---|---|---|
| `ipc` | `src/ipc.ts` | Tauri invoke 封装 + 类型化 DTO | `invoke<T>('cmd_name', { args })` + 39 个 typed wrapper |
| `routes/` | `src/routes/` | 18 页面（hash router） | `Dashboard`, `Markets`, ..., `Onboarding` |
| `components/base/` | `src/components/base/` | Button / Input / Card / Toggle | 8 组件 |
| `components/feedback/` | `src/components/feedback/` | Toast / Modal / Skeleton / Empty / Error | 7 组件 |
| `components/data/` | `src/components/data/` | KpiCard / Table / Chart / Sparkline | 10 组件 |
| `components/business/` | `src/components/business/` | ProviderRow / SignalCard / ConsensusCard | 5 组件 |
| `components/shell/` | `src/components/shell/` | Sidebar / Topbar / UserMenu / CommandPalette | 2 组件 |
| `state/` | `src/state/` | Zustand stores | `theme`, `notifications`, `cache` |
| `hooks/` | `src/hooks/` | React hooks | `useInvoke`, `useTheme`, `useShortcuts` |
| `lib/` | `src/lib/` | 工具 | `format.ts`, `i18n.ts` |
| `styles/` | `src/styles/` | Tailwind + CSS 变量 + 3 主题 | `themes.css` |
| `types/` | `src/types/` | TS 类型 (mirror Rust DTO) | `llm.ts`, `market.ts` |

**关键不变量**：
- L1 **不**直接调 `fetch` 走 HTTP；所有外部调用走 `invoke` → L2 IPC
- L1 **不**直接访问 keychain / filesystem；需要 L2 提供 IPC
- 3 主题用 `[data-theme='dark'|'light'|'matrix']` CSS 变量切换，DOM 结构**完全一致**

---

## 3. 完整目标目录树

```
polyrocket/
│
├── README.md                          # 项目一句话定位 + quickstart
├── package.json                        # pnpm workspace + scripts
├── pnpm-lock.yaml
├── tsconfig.json
├── tsconfig.node.json
├── vite.config.ts
├── tailwind.config.js                  # 3 主题 token 矩阵
├── postcss.config.js
├── .env.example                        # dev-only 模板
├── .gitignore                          # 已含 .env
├── .npmrc                              # allowBuilds 配置
│
├── docs/                               # 治理文档（governance）
│   ├── overview.md                     # ⭐ 本文件：架构总览
│   ├── polyrocket-modules.md           # 17 module 业务说明
│   ├── polyrocket-flows.md             # 20 flow (mermaid)
│   ├── polyrocket-ui-design.md         # 18 页 × 3 主题 UI 规范
│   ├── polyrocket-llm-analysis.md      # M10 业务 + 13.x 真实 HTTP
│   ├── polyrocket-llm-management.md    # M11 配置 + 13/14 secrets + scheduler
│   ├── prototype.html                  # 设计原型 (single-file 3091 行)
│   ├── previews/                       # 54 PNG (3 主题 × 18 页)
│   │   ├── dark/   *.png  (18)
│   │   ├── light/  *.png  (18)
│   │   └── matrix/ *.png  (18)
│   ├── polyradar-dev-governance.md      # 外部治理文件
│   └── *.md
│
├── src/                               # ┌────────────────────────────┐
│   │                                  # │ L1 PRESENTATION (React)   │
│   │                                  # └────────────────────────────┘
│   ├── main.tsx                        # entry: ReactDOM.createRoot
│   ├── App.tsx                         # 根组件: Sidebar + Topbar + Router + Toast
│   │
│   ├── ipc.ts                          # 39 个 typed invoke wrapper
│   │
│   ├── routes/                         # 18 页面 (hash router)
│   │   ├── Dashboard.tsx
│   │   ├── Markets.tsx
│   │   ├── MarketDetail.tsx
│   │   ├── Signals.tsx
│   │   ├── Copy.tsx
│   │   ├── PnL.tsx
│   │   ├── History.tsx
│   │   ├── Lab.tsx
│   │   ├── Analysis.tsx
│   │   ├── LlmPerf.tsx
│   │   ├── LlmMgmt.tsx
│   │   ├── Brief.tsx
│   │   ├── Wallets.tsx
│   │   ├── Notifications.tsx
│   │   ├── Audit.tsx
│   │   ├── Help.tsx
│   │   ├── Settings.tsx
│   │   ├── Onboarding.tsx
│   │   └── _shared/                    # 页面间共享组件 (PageHeader, KpiGrid, EmptyState)
│   │
│   ├── components/
│   │   ├── base/                       # 8 组件
│   │   │   ├── Button.tsx
│   │   │   ├── IconButton.tsx
│   │   │   ├── Input.tsx
│   │   │   ├── Card.tsx
│   │   │   ├── Pill.tsx
│   │   │   ├── Toggle.tsx
│   │   │   ├── Kbd.tsx
│   │   │   └── Spinner.tsx
│   │   ├── feedback/                   # 7 组件
│   │   │   ├── Toast.tsx
│   │   │   ├── Modal.tsx
│   │   │   ├── Skeleton.tsx
│   │   │   ├── EmptyState.tsx
│   │   │   ├── ErrorState.tsx
│   │   │   ├── ConfirmDialog.tsx
│   │   │   └── Banner.tsx
│   │   ├── data/                       # 10 组件
│   │   │   ├── KpiCard.tsx
│   │   │   ├── Table.tsx
│   │   │   ├── Sparkline.tsx
│   │   │   ├── LineChart.tsx
│   │   │   ├── DonutChart.tsx
│   │   │   ├── Heatmap.tsx
│   │   │   ├── ScatterPlot.tsx
│   │   │   ├── ProgressBar.tsx
│   │   │   ├── SegmentedControl.tsx
│   │   │   └── SearchInput.tsx
│   │   ├── business/                   # 5 组件
│   │   │   ├── ProviderRow.tsx
│   │   │   ├── SignalCard.tsx
│   │   │   ├── ConsensusCard.tsx
│   │   │   ├── BetForm.tsx
│   │   │   └── OrderbookTable.tsx
│   │   └── shell/                      # 2 组件
│   │       ├── Sidebar.tsx
│   │       ├── Topbar.tsx
│   │       ├── UserMenu.tsx
│   │       └── CommandPalette.tsx
│   │
│   ├── state/                          # Zustand stores
│   │   ├── theme.ts                    # dark/light/matrix + persistence
│   │   ├── notifications.ts            # toast queue
│   │   ├── cache.ts                    # IPC response cache (5min TTL)
│   │   └── shortcuts.ts                # global hotkeys
│   │
│   ├── hooks/                          # React hooks
│   │   ├── useInvoke.ts                # 包装 invoke + loading/error
│   │   ├── useTheme.ts
│   │   ├── useShortcuts.ts
│   │   ├── useToast.ts
│   │   └── useDebounce.ts
│   │
│   ├── lib/                            # 工具
│   │   ├── format.ts                   # 数字/百分比/时间/币
│   │   ├── i18n.ts                     # zh/en runtime switch
│   │   ├── audit.ts                    # 上报 client 端 error to Sentry
│   │   └── csv.ts                      # 导出 CSV
│   │
│   ├── types/                          # TS 类型
│   │   ├── llm.ts
│   │   ├── market.ts
│   │   ├── signal.ts
│   │   ├── bet.ts
│   │   └── shared.ts
│   │
│   └── styles/
│       ├── index.css                   # Tailwind base
│       ├── themes.css                  # 3 主题 CSS 变量
│       └── tokens.css                  # spacing / radius / shadow / motion
│
├── src-tauri/                          # Rust 后端
│   ├── Cargo.toml                      # workspace + deps
│   ├── Cargo.lock
│   ├── tauri.conf.json                 # bundle / window / permissions
│   ├── build.rs                        # tauri-build
│   ├── icons/                          # app icon (mac/win/linux)
│   └── src/
│       ├── main.rs                     # binary entry
│       ├── lib.rs                      # ┌───────────────────────────┐
│       │                              # │ L2-L5 全在此 crate        │
│       ├── platform/                   # │                           │
│       │   ├── mod.rs                  # │ L5 PLATFORM              │
│       │   ├── keyring/                # │                           │
│       │   │   ├── mod.rs              # │                           │
│       │   │   ├── macos.rs            # │                           │
│       │   │   ├── windows.rs          # │                           │
│       │   │   └── linux.rs            # │                           │
│       │   ├── env.rs                  # │                           │
│       │   └── paths.rs                # │                           │
│       │                              # └───────────────────────────┘
│       ├── infra/                      # ┌───────────────────────────┐
│       │   ├── mod.rs                  # │ L4 INFRA                 │
│       │   ├── db/                     # │                           │
│       │   │   ├── mod.rs              # │                           │
│       │   │   ├── pool.rs             # │                           │
│       │   │   └── settings.rs         # │                           │
│       │   ├── http/mod.rs             # │                           │
│       │   ├── error.rs                # │                           │
│       │   ├── state.rs                # │                           │
│       │   └── scheduler.rs            # │                           │
│       │                              # └───────────────────────────┘
│       ├── domain/                     # ┌───────────────────────────┐
│       │   ├── mod.rs                  # │ L3 DOMAIN                │
│       │   ├── llm/                    # │                           │
│       │   │   ├── mod.rs              # │                           │
│       │   │   ├── clients/            # │                           │
│       │   │   │   ├── mod.rs          # │                           │
│       │   │   │   ├── openai.rs       # │                           │
│       │   │   │   ├── anthropic.rs    # │                           │
│       │   │   │   ├── google.rs       # │                           │
│       │   │   │   ├── deepseek.rs     # │                           │
│       │   │   │   ├── custom.rs       # │                           │
│       │   │   │   └── common.rs       # │                           │
│       │   │   ├── dispatch.rs         # │                           │
│       │   │   ├── prompts.rs          # │                           │
│       │   │   └── cost.rs             # │                           │
│       │   ├── consensus.rs            # │                           │
│       │   ├── polymarket.rs           # │                           │
│       │   ├── signal.rs               # │                           │
│       │   ├── bet.rs                  # │                           │
│       │   ├── copy.rs                 # │                           │
│       │   ├── pnl.rs                  # │                           │
│       │   ├── lab.rs                  # │                           │
│       │   └── wallet.rs               # │                           │
│       │                              # └───────────────────────────┘
│       ├── commands/                   # ┌───────────────────────────┐
│       │   ├── mod.rs                  # │ L2 APPLICATION           │
│       │   ├── wallet.rs               # │ (39 IPC handlers)        │
│       │   ├── market.rs               # │                           │
│       │   ├── signal.rs               # │                           │
│       │   ├── bet.rs                  # │                           │
│       │   ├── copy.rs                 # │                           │
│       │   ├── pnl.rs                  # │                           │
│       │   ├── lab.rs                  # │                           │
│       │   ├── llm.rs                  # │                           │
│       │   ├── llm_mgmt.rs             # │                           │
│       │   ├── secrets.rs              # │                           │
│       │   ├── scheduler.rs            # │                           │
│       │   └── brief.rs                # │                           │
│       │                              # └───────────────────────────┘
│       └── bin/                        # CLI 工具
│           └── dev_smoke.rs            # 端到端 LLM 冒烟测试
│
├── src-tauri/tests/                    # integration tests (L2-L4)
│   ├── llm_clients_openai.rs           # 5 测试
│   ├── llm_clients_anthropic.rs        # 4 测试
│   ├── llm_clients_google.rs           # 4 测试
│   └── llm_dispatch.rs                 # 5 测试 (含 retry + custom)
│
├── scripts/                            # 工具脚本 (与 L1-L5 解耦)
│   ├── snapshot_pages.py               # 54 PNG 截图 (Playwright)
│   ├── check-doc-sync.mjs              # doc-sync 校验
│   └── dev.sh                          # pnpm tauri dev 包装
│
└── .github/                            # CI
    └── workflows/
        ├── rust.yml                    # cargo check + test + clippy
        ├── ui.yml                      # pnpm typecheck + screenshot
        └── doc-sync.yml                # docs check
```

---

## 4. 层间数据流

### 4.1 端到端数据流：用户点 "Run LLM analysis"

```mermaid
sequenceDiagram
    autonumber
    actor U as User
    participant L1a as L1: Analysis.tsx
    participant L1b as L1: ipc.ts
    participant L2 as L2: commands::llm::llm_analyze
    participant L3a as L3: domain::llm::dispatch
    participant L3b as L3: domain::llm::clients
    participant L3c as L3: domain::llm::prompts
    participant L3d as L3: domain::consensus
    participant L4a as L4: infra::db::pool
    participant L4b as L4: infra::http
    participant L5 as L5: platform::keyring
    participant PM as Polymarket CLOB
    participant OA as OpenAI API

    U->>L1a: click "Run analysis"
    L1a->>L1b: invoke('llm_analyze', { market_id })
    L1b->>L2: HTTP IPC over Tauri channel

    L2->>L4a: SELECT markets + signals
    L2->>L3c: build_market_analysis_request(model, ctx)
    L2->>L3c: parse_recommendation(text)

    par for each of 4 providers
        L2->>L3a: dispatch(client, keys, req, cost, policy)
        L3a->>L5: get_key("llm/openai/prod-1")
        L5-->>L3a: sk-... (return secret)
        L3a->>L4b: shared reqwest::Client
        L3a->>L3b: OpenAIClient::call(http, secret, req, cost)
        L3b->>OA: POST /v1/chat/completions
        OA-->>L3b: 200 OK + token counts
        L3b-->>L3a: CallOutcome { text, tokens, cost, ... }
    end

    L3a-->>L2: DispatchOutcome { CallLog, ... }
    L2->>L4a: INSERT llm_call_logs
    L2->>L3c: parse_recommendation(text) for each
    L2->>L3d: consensus_from(recs) → (prob, side, conf)
    L2->>L4a: INSERT llm_recommendations + UPDATE llm_analyses
    L2->>L4a: INSERT audit_log 'llm.analyze'

    L2-->>L1b: LlmAnalysisDto
    L1b-->>L1a: typed result
    L1a-->>U: render 4 LLM cards + consensus + decision bar
```

### 4.2 关键不变量

| # | 规则 | 谁强制 |
|---|---|---|
| 1 | secret 字符串**只能**经 L5 keyring 进/出 Rust 进程 | 编译期 (rust-analyzer lints) |
| 2 | L2 调 L3 必须是 `async fn` 签名（不阻塞） | tokio |
| 3 | L3 函数**不**返回 `serde_json::Value` | clippy lint |
| 4 | 所有 L2 handler **必须**写 audit_log | check-doc-sync mjs |
| 5 | L1 路由组件**不**内联 IPC 逻辑，必须走 `ipc.ts` wrapper | ESLint rule |
| 6 | DB migration 只走 L4 `db/` 模块 | 代码 review |

---

## 5. 现状 → 目标 迁移路线

### 5.1 现状（v0.2 → v0.4）

| 层 | 路径 | 状态 |
|---|---|---|
| L5 platform | `src-tauri/src/platform/{keyring,env,paths}/` | ✅ v0.3a 完成 |
| L4 infra | `src-tauri/src/infra/{error,state,http,db,scheduler}/` | ✅ v0.3b 完成 |
| L3 domain | `src-tauri/src/domain/{llm,polymarket,consensus,signal,bet,copy,pnl,lab,wallet}/` | ✅ v0.3c + v0.4 真实现 |
| L2 application | `src-tauri/src/commands/` (13 个文件) | ✅ v0.3d + v0.4 audit 模块 |
| L1 presentation | `src/` (18 routes × 13 components × 5 stores) | ✅ v0.4 真 React |
| **CI** | `scripts/check-doc-sync.mjs` + `scripts/check-layers.mjs` | ✅ v0.3f 完成（pre-commit governance） |

### 5.2 v0.4 — 功能模块全实现 (13 模块)

| # | 模块 | L3 增量 | L1 路由 | IPCs | Tests |
|---|---|---|---|---|---|
| Foundation | ipc.ts (41) + 13 组件 + types/ | — | — | — | — |
| M1 | Markets | +Category classify + parse_liquidity/volume + closing_bucket | `/markets` `/markets/:id` | list/sync | 6 |
| M2 | Signals | +score + filter_active + sort_by_edge_abs + best_for_market + stats | `/signals` | list/recompute | 8 |
| M3 | Bets | +BetSide + shares_for_size + pnl + is_open | `/history` | place/list | 7 |
| M4 | Wallets | +WalletType + validate_address/chain/label + short_address | `/wallets` | list/add | 13 |
| M5 | Copy | +validate_target_args + should_mirror + is_duplicate_tx | `/copy` | list/add/events | 10 |
| M6 | PnL | +win_rate + brier_score + realized_pnl + categorize + summarize | `/pnl` | kpis | 11 |
| M7 | ModelLab | +RunStatus + validate_version + is_older + is_better | `/lab` | llmPerformance | 7 |
| M8 | Dashboard | — (consumes M1-M7) | `/dashboard` (rewrite) | — | — |
| M9 | Settings | — (UI prefs store) | `/settings` | — | — |
| M10 | LLM Analysis | — (already in M3c) | `/analysis` | 12 | — |
| M11 | LLM Mgmt | — (already in M3c) | `/llm-mgmt` | 14 + 1 audit | — |
| M12 | Daily Brief | — (already in M3c) | `/brief` | 4 | — |
| M13 | Onboarding | — (UI store) | `/onboarding` | — | — |
| X1 | Audit | — (Rust commands/audit.rs) | `/audit` | +2 new | 3 |
| X2 | Notifications | — (toast store) | `/notifications` | — | — |
| Help | — (static page) | `/help` | — | — |

### 5.2 目标（v0.3+）

| 阶段 | 范围 | 状态 | 工时估 |
|---|---|---|---|
| **v0.3a** | 拆 L5：建 `platform/{keyring,env,paths}/`，把 `keyring.rs` 拆分 mac/win/linux | ✅ **done** | 0.5d |
| **v0.3b** | 拆 L4：建 `infra/{db,http,error,state,scheduler}/` | ✅ **done** | 0.5d |
| **v0.3c** | 拆 L3：建 `domain/{llm,consensus,polymarket,signal,bet,copy,pnl,lab,wallet}/`，**新增 8 个 stub domain**（把 L2 里的业务逻辑下移） | ✅ **done** | 2d |
| **v0.3d** | 拆 L2：`commands/` 不动，但每个文件顶部加 `// L2: <module>` 注释 | ✅ **done** | 0.2d |
| **v0.3e** | L1 真 React 化：把 prototype.html 18 页面切到 `src/routes/`，用 TanStack Router + Zustand + React Query，**不**依赖 prototype.html | 🔄 pending | 3d |
| **v0.3f** | CI 加层依赖校验脚本（`scripts/check-layers.mjs`） | ✅ **done** | 0.5d |

**总计 ~7d** 完成 5 层严格分目录。

### 5.4 v0.4 — L3 填实 + L1 真 React 化

| 阶段 | 模块 | commit | tests |
|---|---|---|---|
| M-Foundation | ipc.ts (41) + 13 组件 + types/ | `94ae480` | — |
| M1 | Markets domain + Markets/MarketDetail 路由 | `8a2e107` | +6 |
| M2 | Signals domain + Signals 路由 | `3c242ca` | +8 |
| M3 | Bets domain + History 路由 | `2c27f32` | +7 |
| M4 | Wallets domain + Wallets 路由 | `94435c6` | +13 |
| M5 | Copy domain + Copy 路由 | `4b7dcc8` | +10 |
| M6 | PnL domain + PnL 路由 | `89102bc` | +11 |
| M7 | ModelLab domain + ModelLab 路由 | `5ac9060` | +7 |
| M8 | Dashboard 重写 (consumes M1-M7) | `afb1764` | — |
| M9 | Settings 路由 + prefs-store | `4c8689e` | — |
| M10-M12 | Analysis + LlmPerf + LlmMgmt + Brief 路由 | `e8d24f3` | — |
| M13+X | Onboarding + Notifications + Audit + Help | `10c50fa` | +3 |
| **Total v0.4** | **13 模块** | **12 commits** | **+65 unit (48→113)** |

v0.4 增量：
- 13 个 L1 路由新增/重写
- 41 个 typed IPC wrapper
- 13 个 L3 域模块的纯函数实现
- 65 个新 unit test
- 2 个新 Rust command (audit)
- 1 个新 L1 store (prefs) + toast store + onboarding store

### 5.3 迁移原则

| # | 原则 |
|---|---|
| 1 | **不重写** — 现有代码按 `mv` 重组路径，import 跟着改 |
| 2 | **L3 优先** — 业务逻辑从 L2 IPC handler 下移到 L3 domain，handler 变薄 |
| 3 | **增量 commit** — 每层迁移一个 commit，PR reviewable |
| 4 | **CI 守门** — check-layers 脚本先实现并强制，目录迁移可分阶段 |

---

## 6. 关键约束（治理）

| 主题 | 约束 | 引用 |
|---|---|---|
| **主题** | 三主题仅 CSS 变量不同，DOM 结构完全一致 | `polyrocket-ui-design.md §1` |
| **秘密存储** | OS keyring 是唯一 secret 存储，`.env` 仅 dev | `polyrocket-llm-management.md §13` |
| **错误码** | 8 个 stable polyrocket code (auth/rate_limit/timeout/network/parse/model_not_found/quota/unknown) | `polyrocket-llm-analysis.md §13.9` |
| **doc-sync** | 改 `src/**` 或 `src-tauri/**` 必须同步改 docs | `scripts/check-doc-sync.mjs` |
| **test** | L3 全部覆盖单测；L2 全部覆盖 integration | `src-tauri/tests/` (21/21 pass) |
| **snapshot** | UI 改动必须 `python3 scripts/snapshot_pages.py` 重生成 54 PNG | `scripts/snapshot_pages.py` |

---

## 7. 阅读指南

| 你想了解... | 看哪一节 |
|---|---|
| 总览 / 5 层图 | §1 |
| 每个 module 在哪一层 | §2 |
| 完整目录结构 | §3 |
| 数据怎么从 UI 流到 OS | §4 |
| 怎么从现状迁到目标 | §5 |
| 治理约束 | §6 |
| 某个 module 的业务 | `polyrocket-modules.md` |
| 某个流程怎么走 | `polyrocket-flows.md` |
| UI 怎么设计 | `polyrocket-ui-design.md` |
| LLM 真实怎么调 | `polyrocket-llm-analysis.md §13` |
| 密钥怎么管 | `polyrocket-llm-management.md §13-§14` |

---

## 8. 变更日志

- **v1.0** (2026-06-16) — 初版。
  - §0-§1 设计目标 + 5 层架构图
  - §2 19 个 module 按层定位（5 + 6 + 17 + 12 + 12）
  - §3 完整目标目录树（按层严格区分）
  - §4 端到端数据流 + 6 条不变量
  - §5 现状 → 目标 迁移路线（v0.3a-f, 6 阶段, ~7d）
  - §6 治理约束（6 主题）
  - §7 阅读指南
