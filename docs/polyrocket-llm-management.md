# polyrocket — LLM 管理模块需求 (LMN)

> 版本：v1.3 · 2026-06-22
> 配套：[`polyrocket-llm-analysis.md`](./polyrocket-llm-analysis.md)（M10 业务） · [`polyrocket-modules.md`](./polyrocket-modules.md)（模块全景）
> 模块代号：**M11 — LLM Management**
> 状态：v0.2 路线 + §15 LLM 最新版本承诺 (v0.110+)

---

## 0. 背景

`polyrocket-llm-analysis.md` 定义了 M10（多 LLM 并行分析）的**业务**。但**没有定义 LLM 怎么被管**：

- API key 怎么存？怎么加？怎么换？
- provider 怎么测通不通？延迟多少？
- 调了 1000 次共花多少钱？有没有碰到 rate limit？
- 一个 provider 挂了，用户怎么知道？
- GPT-4o key 用了 OpenAI 官方的，但 Anthropic 走的是第三方代理，怎么统一管理？

**M11 解决**：把所有 LLM 资源（provider + key + proxy + 流量 + 健康）**统一管理**，让 M10 / M12 只关心「调谁」，不关心「key 在哪 / 限速到没 / 这次通了没」。

---

## 1. 模块边界

```
┌──────────────────────────────────────────────────────────────┐
│ M11 LLM Management                                              │
│                                                                │
│  Provider config (llm_providers)                               │
│    ├─ Identity (id, display_name, default_model)                │
│    ├─ Endpoint (api_base, supports_streaming, request_format)  │
│    ├─ Auth (multiple keys in keyring, rotation strategy)        │
│    ├─ Quota (per-day / per-month, hard limit)                  │
│    ├─ Health (last_check_at, last_status, latency_p50/p95)     │
│    └─ Cost (cost_per_1k_in/out, total spend tracked)            │
│                                                                │
│  Connectivity test (F15)                                       │
│    ├─ One-shot test: send minimal request, report latency       │
│    ├─ Continuous probe (every 5min) when provider enabled       │
│    └─ Status states: ok / slow / failing / unreachable / unknown│
│                                                                │
│  Traffic monitoring (F16)                                      │
│    ├─ Per-call log: tokens, cost, latency, status               │
│    ├─ Per-provider rollup: 1h / 24h / 30d                       │
│    ├─ Rate limit detection (429 / quota_exceeded)              │
│    └─ Anomaly detection: spike / sudden cost / repeated fail    │
│                                                                │
│  Win-rate stats (F13, refine)                                  │
│    ├─ Per-LLM × per-Category × per-Time                        │
│    ├─ Per-Prompt-version (A/B compare)                          │
│    ├─ Export to JSON / CSV                                     │
│    └─ Decision-type breakdown                                  │
└──────────────────────────────────────────────────────────────┘
```

---

## 2. LLM Provider 配置（详细）

### 2.1 数据模型：`llm_providers` 升级

**已有**（v1.0）：`id / display_name / enabled / api_base / key_alias / default_model / timeout_ms / cost_per_1k_in / cost_per_1k_out`

**v0.2 增字段**：

| 字段 | 类型 | 说明 |
|---|---|---|
| `provider_kind` | text | `'openai' \| 'anthropic' \| 'google' \| 'deepseek' \| 'openai_compat' \| 'anthropic_compat'` |
| `request_format` | text | `'chat_completions' \| 'messages' \| 'generate_content'` |
| `supports_streaming` | bool | 是否支持 SSE（v0.3 用） |
| `rate_limit_rpm` | int | provider 文档里的 RPM 上限；用于 backoff 计算 |
| `rate_limit_tpm` | int | TPM 上限（token per minute） |
| `quota_daily_cents` | real | 用户硬性日预算（cents） |
| `quota_monthly_cents` | real | 用户硬性月预算（cents） |
| `health_status` | text | 当前健康状态（实时更新） |
| `health_latency_p50_ms` | int | 健康探针历史 P50 延迟 |
| `health_latency_p95_ms` | int | P95 延迟 |
| `last_health_check_at` | int | 上次探针时间戳 |
| `last_health_error` | text | 最近一次错误（如果有） |
| `key_rotation_strategy` | text | `'failover' \| 'round_robin' \| 'manual'` |
| `request_timeout_ms` | int | 单次请求超时（默认 30s） |
| `max_retries` | int | 失败重试次数（默认 2） |
| `notes` | text | 用户备注 |

### 2.2 多 Key 管理（`llm_provider_keys`）

**新表**：

```sql
CREATE TABLE llm_provider_keys (
  id              TEXT PRIMARY KEY,    -- uuid
  provider_id     TEXT NOT NULL REFERENCES llm_providers(id),
  alias           TEXT NOT NULL,       -- 'prod-1' / 'backup-azure' / 'dev'
  keyring_alias   TEXT NOT NULL,       -- 对应 OS keyring 里的 alias
  enabled         INTEGER NOT NULL DEFAULT 1,
  priority        INTEGER NOT NULL DEFAULT 0,  -- 数字越小越优先
  weight          INTEGER NOT NULL DEFAULT 1,  -- round_robin 时权重
  last_used_at    INTEGER,
  last_error      TEXT,
  last_error_at   INTEGER,
  total_calls     INTEGER NOT NULL DEFAULT 0,
  total_errors    INTEGER NOT NULL DEFAULT 0,
  notes           TEXT,
  created_at      INTEGER NOT NULL,
  updated_at      INTEGER NOT NULL,
  UNIQUE(provider_id, alias)
);
CREATE INDEX llm_keys_provider_idx ON llm_provider_keys(provider_id, priority);
```

**为什么需要多 key**：
- 同一个 provider 可能有多账号（个人 + 公司），配额分开
- 一个 key 限速（429）自动 failover 到下一个
- Azure OpenAI / AWS Bedrock 走代理需要单独 endpoint
- 第三方代理（OpenRouter / OneAPI）作为兜底

### 2.3 Key 轮换策略

| 策略 | 行为 | 适用场景 |
|---|---|---|
| `failover`（默认） | 按 priority 升序选；当前 key 失败时切下一个 | 生产，稳定优先 |
| `round_robin` | 按 weight 轮询分摊配额 | 大量调用，避免单 key 超限 |
| `manual` | 用户指定用哪个；失败不切换 | 调试 / 对照实验 |

**自动 failover 触发条件**（任意一个）：
- HTTP 429（rate limit）
- HTTP 401/403（key 失效）
- HTTP 5xx
- 连接超时
- 解析失败（连续 3 次）

---

## 3. 连通性测试（详细）

### 3.1 用户主动测试

UI 在 Settings → LLM Management → 每个 provider 行右侧有 **"Test"** 按钮。点击后立即发送一个最小请求：

| Provider | 测试请求 |
|---|---|
| OpenAI | `POST /chat/completions` 携带 `gpt-4o-mini` model，1 token 回复 |
| Anthropic | `POST /v1/messages` with `claude-3-haiku-20240307`，max_tokens=1 |
| Google | `POST /v1beta/models/gemini-2.0-flash:generateContent` |
| DeepSeek | `POST /chat/completions` with `deepseek-chat`，max_tokens=1 |
| OpenAI-compatible | 复用 OpenAI 协议 |

### 3.2 测试返回

```typescript
interface ConnectivityTestResult {
  provider_id: string;
  success: boolean;
  latency_ms: number | null;
  http_status: number | null;
  model_used: string;
  error_code: 'auth' | 'rate_limit' | 'timeout' | 'network' | 'parse' | null;
  error_message: string | null;
  checked_at: number;
  // 额外 metadata（如果成功）
  test_response_text?: string;
  tokens_in?: number;
  tokens_out?: number;
}
```

**UI 展示**（toast + provider 行状态点）：
- ✅ 成功 + 延迟 < 1s → 绿色 dot
- ✅ 成功 + 延迟 1-3s → 黄色 dot
- ✅ 成功 + 延迟 > 3s → 橙色 dot
- ❌ 401/403 → 红色 + 「Key 无效，去配置」
- ❌ 429 → 红色 + 「Rate limit，等 60s 重试」
- ❌ timeout → 灰色 + 「Endpoint 不可达」

### 3.3 后台持续探针

**频率**：每 5 分钟一次（仅对 `enabled=1` 的 provider）

**存储**（`llm_health_checks`，见 §4）

**告警阈值**（满足任一触发用户通知）：
- 连续 3 次失败 → toast 通知
- P95 延迟 > 5s（vs baseline 2s）→ 通知「provider 变慢」
- 24h 失败率 > 10% → 通知

---

## 4. 健康检查存储（`llm_health_checks`）

**新表**：

```sql
CREATE TABLE llm_health_checks (
  id              INTEGER PRIMARY KEY AUTOINCREMENT,
  provider_id     TEXT NOT NULL REFERENCES llm_providers(id),
  key_id          TEXT,                  -- 哪把 key 测的
  checked_at      INTEGER NOT NULL,
  trigger         TEXT NOT NULL,          -- 'user' | 'probe' | 'auto_after_fail'
  success         INTEGER NOT NULL,
  latency_ms      INTEGER,
  http_status     INTEGER,
  error_code      TEXT,                  -- 同 ConnectivityTestResult.error_code
  error_message   TEXT,
  -- metadata
  model_used      TEXT,
  test_request_id TEXT
);
CREATE INDEX health_provider_time_idx ON llm_health_checks(provider_id, checked_at);
CREATE INDEX health_success_time_idx ON llm_health_checks(success, checked_at);
```

**前端用**：
- Provider 行右侧状态点（取最近一次）
- LLM Management 页"近 24h 健康"图（折线：成功/失败/延迟）
- "上次错误" tooltip

**保留期**：90 天（超过自动清理）

---

## 5. 流量监控（详细）

### 5.1 每次调用写日志（`llm_call_logs`）

**新表**：

```sql
CREATE TABLE llm_call_logs (
  id              INTEGER PRIMARY KEY AUTOINCREMENT,
  analysis_id     TEXT,                  -- 关联到 llm_analyses（如果是从分析触发）
  provider_id     TEXT NOT NULL REFERENCES llm_providers(id),
  key_id          TEXT REFERENCES llm_provider_keys(id),
  called_at       INTEGER NOT NULL,
  latency_ms      INTEGER NOT NULL,
  tokens_in       INTEGER NOT NULL DEFAULT 0,
  tokens_out      INTEGER NOT NULL DEFAULT 0,
  cost_cents      REAL NOT NULL DEFAULT 0,
  http_status     INTEGER NOT NULL,
  success         INTEGER NOT NULL,        -- 0/1
  error_code      TEXT,
  error_message   TEXT,
  -- 用于胜率统计
  prompt_version  TEXT,                  -- 用的哪个 prompt
  predicted_prob  REAL,                  -- 如果成功
  recommended_side TEXT,                -- 'YES'/'NO'/'skip'
  -- 审计
  caller          TEXT NOT NULL,         -- 'm10.llm_analyze' | 'm12.brief' | 'm7.model' | 'user.test'
  retry_count     INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX call_logs_provider_time_idx ON llm_call_logs(provider_id, called_at);
CREATE INDEX call_logs_analysis_idx ON llm_call_logs(analysis_id);
CREATE INDEX call_logs_success_time_idx ON llm_call_logs(success, called_at);
```

**粒度**：每次 HTTP 请求一行，**包括失败**。失败也要记（成本 0 但 latency 仍记）—— 这对诊断至关重要。

### 5.2 实时聚合（IPC 返回）

```typescript
interface LlmTrafficSummary {
  provider_id: string;
  window: '1h' | '24h' | '30d';
  calls_total: number;
  calls_success: number;
  calls_failed: number;
  success_rate: number;
  avg_latency_ms: number;
  p95_latency_ms: number;
  total_tokens_in: number;
  total_tokens_out: number;
  total_cost_cents: number;
  rate_limit_hits: number;  // 429 次数
  -- 与上次的差值（增量告警用）
  delta_calls_last_window: number;
  delta_cost_last_window: number;
}
```

### 5.3 用量限制（硬性 + 软性）

**硬性**（超过自动停用 provider，不发请求）：

- `quota_daily_cents` — 每日上限；00:00 UTC 重置
- `quota_monthly_cents` — 每月上限；1号 00:00 UTC 重置

**软性**（warn 但继续）：

- 80% / 95% / 100% 三档 toast 通知
- 100% 软超：UI 显示「⚠ 已超 80% 预算，今日还剩 $0.20」

**超额行为**：
- 硬性超额 → `enabled = 0`，`audit_log` 记 `'llm.quota.exceeded'`
- 用户在 Settings 手动重新启用

### 5.4 异常检测规则

| 异常 | 触发 | 通知 |
|---|---|---|
| **Rate limit** | 1h 内 429 > 5 次 | toast + provider 行变橙 |
| **Sudden cost spike** | 单笔 cost > 历史 p99 的 3 倍 | toast |
| **Repeated failure** | 连续 3 次失败 | toast + 临时禁用 |
| **Endpoint drift** | P95 延迟比 baseline 高 2x | toast |
| **Auth failure** | 401/403 一次 | toast + 标记 key 需重配 |

---

## 6. 胜率统计（v1.0 增强）

### 6.1 v1.0 已有

F13 的 4 个视图：Heatmap / Scatter / Timeseries / Decisions。

### 6.2 v0.2 增

| 新增 | 用途 |
|---|---|
| **Per-prompt-version 切分** | v3.1 vs v3.2 prompt 哪个更准？ |
| **置信度分桶细化** | `[<40%, 40-50%, 50-60%, 60-70%, 70-80%, >80%]` 6 桶 |
| **按调用延迟分桶** | 「快 LLM 准还是慢 LLM 准？」 |
| **按成本分桶** | 「贵的 LLM 准还是便宜的 LLM 准？」 |
| **导出 CSV / JSON** | 用户自己跑分析 |
| **回填** | 老数据导入（v0.1 bet 关联 LLM） |

### 6.3 新增 IPC

```
llm_stats_by_prompt({prompt_version, window_days?}): LlmStatsCell[]
llm_stats_by_confidence_band({provider_id?, window_days?}): LlmConfidenceBand[]
llm_stats_export({format: 'csv' | 'json', window_days?}): string (file content)
llm_stats_by_cost_efficiency({window_days?}): LlmCostEfficiencyRow[]
```

---

## 7. UI 规范

### 7.1 新增页面：`/llm-management`

Sidebar → Workspace → **LLM Management**（图标：`shield-check`）

```
┌─────────────────────────────────────────────────────────────┐
│ PageHeader                                                   │
│   h1 "LLM Management"                                         │
│   subtitle "Configure providers, monitor traffic, test        │
│             connectivity"                                     │
│   [+ Add provider]                                            │
├─────────────────────────────────────────────────────────────┤
│ Provider list (table)                                        │
│ ┌──────────────────────────────────────────────────────────┐ │
│ │ Status │ Provider      │ Model      │ Key │ Latency │ Cost│ │
│ ├──────────────────────────────────────────────────────────┤ │
│ │ ●     │ GPT-4o        │ gpt-4o     │ 2  │ 1.2s    │ $2.80│ │ ← 行点击展开详情
│ │ ●     │ Claude Sonnet │ sonnet-4   │ 1  │ 1.8s    │ $1.20│ │
│ │ ●     │ Gemini 2.5    │ 2.5-pro    │ 1  │ 0.9s    │ $0.80│ │
│ │ ●     │ DeepSeek V3   │ v3         │ 1  │ 2.1s    │ $0.10│ │
│ │ ○     │ Custom Proxy  │ —          │ 0  │ —       │ $0   │ │
│ └──────────────────────────────────────────────────────────┘ │
├─────────────────────────────────────────────────────────────┤
│ Expanded row (click any row)                                 │
│   ┌─ Identity ──────────────────────────────────────────────┐│
│   │ Provider:    OpenAI                                     ││
│   │ Model:       gpt-4o-2024-08-06                         ││
│   │ Endpoint:    https://api.openai.com/v1                  ││
│   │ Format:      chat_completions                           ││
│   │ Supports streaming: yes                                 ││
│   └──────────────────────────────────────────────────────────┘│
│   ┌─ Keys (2) ─────────────────────────────────────────────┐│
│   │ [●] prod-1   keyring: llm.openai.prod  priority 0      ││
│   │     1240 calls · 0 errors · last used 3m ago            ││
│   │ [●] backup   keyring: llm.openai.bk    priority 10     ││
│   │     18 calls · 0 errors · last used 12h ago            ││
│   │ [+ Add key]                                             ││
│   └──────────────────────────────────────────────────────────┘│
│   ┌─ Quota (30D) ──────────────────────────────────────────┐│
│   │ Daily:    $1.00 / $1.00  ████████████████████ 100%   ││
│   │ Monthly:  $4.90 / $50.00 ████░░░░░░░░░░░░░░░  9.8%   ││
│   └──────────────────────────────────────────────────────────┘│
│   ┌─ Health (24h) ──────────────────────────────────────────┐│
│   │  [SVG sparkline: 1440 probes, 0 failures, p50 1.2s]    ││
│   │  Last error: none                                       ││
│   │  [Test now]   [View full log]                          ││
│   └──────────────────────────────────────────────────────────┘│
│   Actions: [Edit] [Disable] [Delete]                         │
└─────────────────────────────────────────────────────────────┘
```

### 7.2 Settings 加 1 section

`Settings → LLM`（已有 v0.1 的 4 个 provider 勾选）扩为：

```
┌─ LLM ─────────────────────────────────────────┐
│ Default analysis providers: [✓]GPT [✓]Claude  │
│                                  [✓]Gemini [ ]DS│
│ Default prompt version: [v3.2 ▾]              │
│ Concurrency: [4 in parallel]                  │
│ Failure tolerance: [continue with N-1 LLM]    │
│ Daily budget cap: $1.00                       │
│ Monthly budget cap: $50.00                    │
│ Default analysis size: 8 markets / refresh    │
│ [Go to LLM Management ↗]                      │
└───────────────────────────────────────────────┘
```

### 7.3 Toast 通知模板

| 事件 | 文案 |
|---|---|
| 连通性测试成功 | 「✓ {provider} · 1.2s」 |
| 连通性测试失败 | 「✗ {provider} · {error_code}」 |
| Rate limit | 「⚠ {provider} hit rate limit (5× in 1h)」 |
| Budget 80% | 「⚠ {provider} 80% of daily budget used」 |
| Auth 失败 | 「✗ {provider} · key invalid, reconfigure」 |
| 连续失败 | 「⚠ {provider} 3 fails in a row, temporarily disabled」 |

---

## 8. 安全 & 隐私

- **API key 永远走 OS keyring**（macOS Keychain / Windows Cred Mgr / Linux Secret Service）
- **key 引用**：DB 里只存 `keyring_alias` 字符串（`"llm.openai.prod"`），不存 key 明文
- **proxy 凭据**：如果 endpoint 是第三方代理，代理的 key 也存 keyring（`"proxy.openrouter"`）
- **审计**：每次 key 创建 / 删除 / 失败都写 `audit_log`
- **导出**：`llm_stats_export` **不导出** key / endpoint credentials

---

## 9. 性能

- **连通性测试**：单次 < 3s（含超时），串行
- **后台探针**：5min 周期，4 provider × 5min = 每 5min 4 次请求，每日 ~1150 次
  - v0.3 优化：自适应频率（fail 时加密，OK 时拉长到 15min）
- **流量查询**：`llm_call_logs` 按 provider_id + 时间索引，单 provider 1h 查询 < 5ms
- **保留策略**：
  - `llm_call_logs` 保留 90 天（明细）
  - 90 天后聚合到 `llm_traffic_hourly`（v0.3 加）

---

## 10. 错误处理

| 场景 | 行为 |
|---|---|
| 用户删除正在用的 provider | confirm dialog「This provider is used by 23 analyses. Delete anyway?」 |
| Key 全失败 | provider 状态变 red，所有在跑的 analysis 标 `partial` |
| Budget 超限 | 自动 disable + 通知；用户手动 re-enable |
| Endpoint 改后不兼容 | connectivity test 失败，状态红，UI 提示「format mismatch: this endpoint is not OpenAI-compatible」 |
| 导入老数据 | 走 `llm_stats_by_prompt` 看到「pre-v0.2: 47 bets without LLM data」 |

---

## 11. v0.2 实施清单

| 任务 | 工作量 | 依赖 |
|---|---|---|
| Schema: 扩 llm_providers + 新增 llm_provider_keys / llm_call_logs / llm_health_checks | 0.5d | 无 |
| Rust: `llm_provider_*` CRUD (4 IPC) | 1d | 1 |
| Rust: `llm_test_connectivity` (1 IPC, 4 provider 适配) | 1.5d | 1 |
| Rust: `llm_traffic_*` 实时聚合 (3 IPC) | 1d | 1 |
| Rust: `llm_stats_*` 扩展 (4 IPC) | 1d | 1 |
| Rust: 探针调度（tokio interval） | 0.5d | 1 |
| Frontend: `/llm-management` 页面 | 2d | 5 |
| Frontend: Settings LLM 扩展 | 0.5d | 5 |
| Frontend: Toast 通知集成 | 0.5d | 5 |
| 文档 + 测试 | 1d | all |

**总计 ~9.5 工日**

---

## 12. 与其他模块关系

| 模块 | 关系 |
|---|---|
| **M10 LLM Analysis** | 消费 M11 的 provider list + 调用接口 |
| **M12 Daily Brief** | 同上，且需要 `llm_traffic` 算 cost 扣减 |
| **M9 Settings** | 暴露简单配置（启用哪些、并发度） |
| **X1 AuditLog** | 所有 provider CRUD / key 操作 / 异常都写日志 |

---

## 13. Client-side key persistence（v1.1 重点）

API key **永远不入 SQLite、不入 .env、不入 git**。唯一受信任的存储是 OS keyring（macOS Keychain / Windows Credential Manager / Linux Secret Service）。SQLite 只存 `keyring_alias` 字符串（"key 放在 OS 哪里"），不存 secret。

### 13.1 两条写路径

| 路径 | 何时用 | 谁触发 | 写哪里 |
|---|---|---|---|
| **A. Client paste（主路径）** | 正常用法 | 用户在 Settings → LLM Management → Provider row → `Add key` 弹窗粘贴 | Rust IPC `llm_key_upsert { key, secret }` → `keyring::set_key(keyring_alias, secret)` + 写 `llm_provider_keys` 行 |
| **B. .env dev sync（仅开发）** | 本地调试、CI、headless 跑流量 | 启动时 Rust 检查 `POLYROCKET_ENV=dev && POLYROCKET_KEYRING_ONLY=0` | 解析 `.env` → 对每个未配置的 `llm/<provider>/<alias>` 调 `keyring::set_key` |

任何 **不是 dev 环境** 的启动，`.env` 一行都不读；`keyring::has_key()` 返回 false 的 provider 在 M10 调用时直接报 `auth` 错误，前端 toast 提示「未配置 key」并跳转到 Settings。

### 13.2 keyring alias 约定

```
llm/<provider_id>/<key_alias>          # LLM API key
  例如 llm/openai/prod-1
       llm/anthropic/backup
polyrocket/pm/api                      # Polymarket CLOB API key
polyrocket/pm/secret                   # Polymarket CLOB secret
polyrocket/pm/passphrase               # Polymarket CLOB passphrase
polyrocket/wallet/<alias>              # 钱包私钥
  例如 polyrocket/wallet/primary
```

Rust 端 `keyring.rs` 提供 builder：`keyring::llm_alias(provider, key)` / `keyring::pm_api_alias()` / `keyring::wallet_alias(alias)`，所有调用点都用 builder，避免字符串拼写漂移。

### 13.3 新增 IPC（client paste 路径）

| IPC | 入参 | 行为 |
|---|---|---|
| `llm_key_upsert { key, secret? }` | LlmProviderKeyDto + 可选 secret | secret 非空时写 keyring（覆盖同 alias 的旧值）；同时 upsert SQLite 行 |
| `llm_key_set_secret { key_id, secret }` | 现有 key 的 id + 新 secret | 仅替换 keyring 内容，不动 SQLite 行；audit_log `llm.key.secret.rotate` |
| `llm_key_delete { key_id }` | key id | 删 SQLite 行 + 删 keyring 条目（best-effort） |
| `llm_provider_delete { provider_id }` | provider id | 删所有 key + provider + 批量清 keyring（仅删 `llm/<pid>/*` 命名空间） |
| `llm_pm_set_credentials { api_key, api_secret, api_passphrase, host?, chain_id? }` | 三个明文 | 三个 alias 全部写 keyring；host/chain_id 落 `_polyrocket_settings` 表 |
| `llm_pm_clear_credentials` | — | 清三个 PM alias + `_polyrocket_settings` 中 `pm_*` |
| `polyrocket_wallet_set_pk { private_key, address, alias? }` | 0x... 64 hex + 0x... 地址 | 验长度/hex 合法性，写 `polyrocket/wallet/<alias>`；同时 upsert `wallets` 行（**不存 pk**） |
| `polyrocket_wallet_clear_pk { alias? }` | alias | 删 keyring + 清 wallets.keyring_alias |
| `secrets_status` | — | 返回 `Vec<SecretStatus>`，**只含 kind/alias/configured/label，不含 secret 本身**。UI 用它显示 ✓/✗ |

### 13.4 .env 启动同步规则（lib.rs `maybe_load_dev_env`）

```rust
fn maybe_load_dev_env() {
    if env != "dev"                       { return; }   // prod/staging 一律跳过
    if keyring_only                      { return; }   // 强制 keychain-only 模式
    let pairs = parse_env_file(".env");
    sync_env_to_keyring(&pairs);
}
```

`sync_env_to_keyring` 只对 **当前 keyring 没有该 alias** 的项写新值，已存在的 **绝不覆盖**（用户已在前端粘贴的优先级永远最高）。每个 alias 写完后只 log 名字 + 长度，**不 log 内容**。

| 环境变量 | 写入的 keyring alias |
|---|---|
| `OPENAI_API_KEY` | `llm/openai/openai-prod-1` |
| `OPENAI_BACKUP_KEY` | `llm/openai/openai-backup` |
| `ANTHROPIC_API_KEY` | `llm/anthropic/anthropic-prod-1` |
| `ANTHROPIC_BACKUP_KEY` | `llm/anthropic/anthropic-backup` |
| `GOOGLE_API_KEY` | `llm/google/google-prod-1` |
| `DEEPSEEK_API_KEY` | `llm/deepseek/deepseek-prod-1` |
| `CUSTOM_LLM_API_KEY` | `llm/custom/custom-prod-1` |
| `POLYMARKET_API_KEY` | `polyrocket/pm/api` |
| `POLYMARKET_API_SECRET` | `polyrocket/pm/secret` |
| `POLYMARKET_API_PASSPHRASE` | `polyrocket/pm/passphrase` |
| `POLYROCKET_WALLET_PRIVATE_KEY` | `polyrocket/wallet/<POLYROCKET_WALLET_ALIAS or "primary">` |

### 13.5 UI 行为

- **Settings → LLM Management → Provider row → Add key**：弹窗内 2 个字段：alias（如 `prod-1`）+ secret 密码框。提交后调 `llm_key_upsert { key, secret }`。密码框提交后立即清空（不留在 DOM）。
- **Key 行右侧 ✏️ 按钮**：再弹一次粘贴框，调 `llm_key_set_secret`（rotate）。
- **Key 行右侧 🗑 按钮**：确认对话框「This will also remove the secret from your OS keychain」，调 `llm_key_delete`。
- **Provider 行右侧 🗑 按钮**：确认对话框列出「Will remove N keys and their OS keychain entries」，调 `llm_provider_delete`。
- **顶部 banner** `secrets_status` 返回的列表里有 ✗ 的项：用红色 chip 显示「key missing — paste to enable」。

### 13.6 错误恢复

- keyring 写失败（OS 拒绝、keychain 被锁）：前端 toast 红色「OS keychain unavailable, retry or restart」；M10 调用返回 `error_code='auth'`，前端跳 Settings。
- keyring 条目被人手动清掉：M10 调用报 `auth`，前端提示「API key for `<provider>` no longer in OS keychain, please re-paste」。
- `.env` 解析失败：tracing warn，**不阻断启动**；前端照常提示粘贴。
- 数据库有 row 但 keyring 没 secret：`secrets_status` 返回 `configured: false`，UI 灰显 provider row + banner 提示。

---

## 14. 后台调度（v1.2 重点）

3 个独立 tokio task，进程启动时由 `lib.rs::run()` 的 `setup` 钩子 `scheduler::start(pool, http)` 启动，每个 task 自我恢复（DB / 网络错误不终止进程）。

### 14.1 3 个 task 概览

| Task | 间隔 | 起的作用 | 写入表 |
|---|---|---|---|
| `run_health_probe_loop` | `HEALTH_PROBE_INTERVAL_MIN` (默认 5) | 对每个 enabled provider 跑 1-token 连通性探针 | `llm_health_checks` + `llm_providers.health_*` + `audit_log` (auto_disable) |
| `run_daily_brief_loop` | 24h cron, `DAILY_BRIEF_HOUR_UTC` (默认 0) | 每日 00:00 UTC 刷新 Daily Brief | `daily_briefs` (upsert) + `audit_log` |
| `run_anomaly_loop` | `ANOMALY_WINDOW_MIN` (默认 60) | 滚动窗口异常检测（rate limit spike / cost spike / 低成功率 / 零活动） | `audit_log` (`anomaly.detected`) |

### 14.2 Health Probe 细节

每次 sweep：
1. 查 `llm_providers WHERE enabled = 1`
2. 对每个 provider 取 priority 最低的 1 个 key
3. 用 `CallRequest { max_tokens: 1, temperature: 0.0 }` 跑真实 LLM call
4. 写 `llm_health_checks` (trigger='background') + 更新 `llm_providers.health_status` + EMA 更新 `health_latency_p50_ms` (新值 = (旧值 × 4 + 新值) / 5)
5. 失败时检查 `llm_health_checks` 最近 3 条是否都 fail → `UPDATE llm_providers SET enabled = 0` + audit_log

**关键不变量**：
- probe 单个 provider 在独立 tokio::spawn 里跑，1 个慢/挂的 provider 不阻塞其他
- `enabled=0` 不会被 probe（避免无意义的 401）
- keyring 没有 secret 不会 panic，记 `auth` 错 + 触发 auto-disable streak
- 启动后 5s 延迟 first probe（避免冷启动时序问题）

### 14.3 Daily Brief cron 细节

```
启动
  ↓
seconds_until_next_brief(0, 0) = 距下次 00:00 UTC 的秒数
  ↓
sleep(...)
  ↓
run_daily_brief_once()
  ↓
sleep(24h)
  ↓ (loop)
run_daily_brief_once()
  ...
```

每次 `run_daily_brief_once`：
1. 查 `markets WHERE closes_at > now` 数活跃市场
2. 读 `DAILY_BRIEF_TOP_N` env (默认 8)
3. `INSERT INTO daily_briefs (...) ON CONFLICT(brief_date) DO UPDATE` — 同一天只 1 行
4. 写 audit_log `brief.refresh` (trigger=cron)

**v0.2 简化**：用活跃市场数 + stub JSON 填充，**不**算完整 M12 评分公式（v0.3 加）。triggers 是 user-callable：`scheduler_run_daily_brief_now`。

### 14.4 Anomaly Detection 细节

每 `ANOMALY_WINDOW_MIN` (默认 60) 分钟跑一次，4 种异常：

| 异常 | 触发条件 | 用途 |
|---|---|---|
| `rate_limit_spike` | 当前窗口 rate_limit_hits ≥ 5 | 限流预警，调 quota |
| `cost_spike` | cost > prev_window × 3 | 异常花销，调查 loop bug |
| `zero_activity` | 当前窗口 0 call, 之前 > 0 | provider 死了，没任务来 |
| `low_success_rate` | success_rate < 0.7 且 call ≥ 3 | 综合健康度告警 |

每条异常写 `audit_log` action='anomaly.detected'，payload 包含：
```json
{
  "window_min": 60,
  "kinds": ["rate_limit_spike", "low_success_rate"],
  "calls": 87,
  "rate_limit_hits": 6,
  "cost_cents": 0.42,
  "success_rate": 0.65
}
```

前端 `/audit` 可看；`/notifications` 用 kind=system 显示 toast。

### 14.5 启动/停止

- **启动**：`lib.rs::run()` 的 `setup` 钩子调 `scheduler::start(pool, http)`，3 个 task 自动 spawn
- **停止**：当前未实现停止（task 跟进程同寿）。`SchedulerHandle.shutdown()` 存在但**不**被 lib.rs 调
- **测试**：用 `run_health_probe_now(pool, http)` / `run_daily_brief_now(pool)` 单次跑（v0.2 新增 public 函数）

### 14.6 env 调优

```bash
POLYROCKET_HEALTH_PROBE_INTERVAL_MIN=5      # 默认 5
POLYROCKET_DAILY_BRIEF_HOUR_UTC=0           # 默认 0
POLYROCKET_DAILY_BRIEF_TZ_OFFSET_MIN=0      # 默认 0 (UTC); 上海 = +480
POLYROCKET_ANOMALY_WINDOW_MIN=60            # 默认 60
DAILY_BRIEF_TOP_N=8                         # 默认 8
```

### 14.7 新增 IPC（手动触发）

| IPC | 用途 | 返回 |
|---|---|---|
| `scheduler_status` | 显示当前 config + 下次 brief 触发时间 | `{ health_probe_interval_sec, daily_brief_hour_utc, next_brief_run_at_unix_ms, ... }` |
| `scheduler_run_health_probe_now` | 立刻跑一次 probe sweep | `{ triggered_at_unix_ms, kind: 'health_probe', ok, error? }` |
| `scheduler_run_daily_brief_now` | 立刻跑一次 brief | `{ ..., kind: 'daily_brief', ... }` |

### 14.8 错误恢复

| 错误 | 行为 |
|---|---|
| DB 连接断开 | 整 loop 静默 catch 1 次 + warn，下个 tick 继续 |
| 单 provider HTTP 超时 | 仅该 provider 记 failing，其他继续 |
| keyring 不可用 | 记 `auth` 错，整 provider fail |
| audit_log INSERT 失败 | 不阻塞 scheduler 主路径，仅 tracing::warn |

---

## 15. LLM 最新版本承诺与跟进策略（v0.110+ 重点）

**承诺**：polyrocket 默认对接 **各家国产大模型的最新版本**。任何一个上游厂商发布新版本（GLM-5.2 / Qwen4 / Doubao-2 / kimi-k3 / MiniMax-Text-02 等），polyrocket 在 **2 周内**完成以下跟进。

### 15.1 为什么这是硬性策略

用户的核心需求是「用最新最强的国产 LLM 做 Polymarket 预测」。如果 polyrocket 锁死在 GLM-4.5 / Qwen3-Max 这类 2025 年的旧版本，会带来两个直接问题：

1. **预测质量落后**：GLM-5.2 相对 4.5 在推理 + 长上下文 + 中文金融术语上有显著提升（旧模型无 WebSearch / Tool Use / 1M context 等新能力）
2. **Benchmark 漂移**：用户拿 polyrocket 输出跟厂商最新 demo 对比会以为我们 bug

### 15.2 跟进 SLA

| 阶段 | 时间 | 谁负责 | 动作 |
|---|---|---|---|
| **D+0**：厂商发版 | — | — | 厂商 release notes / 官方公告 |
| **D+3**：评估 | Maintainer | 看 release notes + benchmark 变化 + 新能力（function call / vision / context window） | 决定是否纳入 polyrocket |
| **D+7**：落地 | Maintainer | (a) `llmStep.tsx` PROVIDERS 数组更新 `default_model` 字段 (b) `docs/llm-providers.md` 对照表更新 (c) `/llm-mgmt` 路由新增 "Latest version" 链接指向厂商公告 (d) CHANGELOG / ship log | 一键切到新模型 |
| **D+14**：验证 | CI + 用户反馈 | 跑 connectivity test + 1 轮 M10 多 LLM 投票 baseline 对比 | 决定是否回滚或继续 |

### 15.3 数据流：模型版本在 polyrocket 的存储路径

```
厂商官网 (智谱 / 阿里 / 字节 / 月之暗面 / MiniMax)
   ↓
docs/llm-providers.md (版本对照表 — 用户可见)
   ↓
src/components/welcome/LlmStep.tsx (PROVIDERS.defaultModel — 用户初始选择)
   ↓
SQLite llm_providers.default_model (持久化 — 用户运行时实际调用的模型)
   ↓
Rust dispatch → http POST { model: default_model } → 厂商 API
```

`default_model` 字段已经在 v0.86 设计好支持 per-provider 配置，**不需要 schema migration**。跟进流程只涉及修改 `LlmStep.tsx` 的默认值 + `llm-providers.md` 文档。

### 15.4 UI 上的版本透明性

在 `/llm-management` 页面（v0.2 计划）每个 provider 卡片要显示：

```
┌─ Alibaba 通义千问 (Qwen) ────────────────────┐
│  current model: qwen4-max-20260615            │
│  latest from vendor:  [→] (打开 Qwen 模型列表)│
│  last-updated:       2026-06-22              │
│  health: ● ok · 245ms                        │
└──────────────────────────────────────────────┘
```

- `current model` 字段直接显示 SQLite 里 `default_model` 的值
- `latest from vendor` 链接到厂商模型列表页 (Qwen → `https://help.aliyun.com/zh/model-studio/developer-reference/model-overview` / GLM → `https://open.bigmodel.cn/dev/api` / 等)
- 用户在卡片上直接点击 Edit → 改 model ID → 一键保存

### 15.5 自动检测（v0.3+ 路线）

**当前手动** (v0.110)：用户或 maintainer 手工改 default_model + 跑 connectivity test。

**未来自动** (v0.3 路线)：
- L1 启动时拉一次厂商的 `/v1/models` 端点（OpenAI compat 都有这 endpoint）
- 对比 `current_model` 跟 vendor 最新 stable / preview model
- 弹 toast: "新模型 Qwen4-Max-20260615 可用，[查看] [切换]"
- 用户点 [切换] → 写 SQLite + 跑 connectivity test

### 15.6 厂商版本追踪 checklist

每次新版本发布，maintainer 走一次这个 checklist：

- [ ] `LlmStep.tsx` PROVIDERS[i].defaultBase 不变（端点通常不变）
- [ ] `LlmStep.tsx` PROVIDERS[i].hint 更新 (新模型名 + 厂商 changelog 日期)
- [ ] `docs/llm-providers.md` §1 表格 + §3 各 provider notes 更新
- [ ] `docs/polyrocket-llm-management.md` §15.3 数据流图版本号更新（如有）
- [ ] `src-tauri/src/commands/llm_mgmt.rs` 若厂商改 endpoint / auth header / request shape 才需要改 (例如 Doubao 加 v4 端点)
- [ ] `src/components/welcome/LlmStep.test.tsx` "renders all 10 providers" 测试断言更新 (provider 数量变了)
- [ ] `scripts/run-ci-local.sh` 跑通，105+ 测试 5/5 绿
- [ ] `git push` (用户说了再推)

### 15.7 当前最新版本 (v0.110 起，需用户告知后填入)

> **空缺字段，待 maintainer 补全**。本节作为「跟进策略」的 placeholder，实际 model ID 由用户在 vendor 官方 release 公告后填入。

| Provider | 当前 `default_model` (placeholder) | 厂商最新公告 | D+0 公告日期 |
|---|---|---|---|
| Qwen (通义千问) | _待填_ | [Qwen 模型列表](https://help.aliyun.com/zh/model-studio/developer-reference/model-overview) | _待填_ |
| Doubao (豆包) | _待填_ | [豆包模型列表](https://www.volcengine.com/docs/82379) | _待填_ |
| Kimi (Moonshot) | _待填_ | [Moonshot 模型列表](https://platform.moonshot.cn/docs/intro) | _待填_ |
| GLM (智谱) | _待填_ (例: `glm-5.2`) | [BigModel 模型列表](https://open.bigmodel.cn/dev/api) | _待填_ |
| MiniMax | _待填_ | [MiniMax 模型列表](https://api.minimax.chat/document) | _待填_ |

**填表流程**：
1. 用户跑 5 个厂商官网，抄最新 stable model ID
2. 更新 `default_model` 字段 (上面这行)
3. 同步更新 `LlmStep.tsx` PROVIDERS (UI 初始值)
4. 更新 `docs/llm-providers.md` §3 各家 notes
5. 跑 CI，跑 connectivity test，commit + ship log

---

## 变更日志

- **v1.2** (2026-06-16) — 新增 §14 后台调度：3 个 tokio task（health probe / daily brief cron / anomaly detection）+ 3 个新 IPC（scheduler_status / scheduler_run_health_probe_now / scheduler_run_daily_brief_now）。3-fail auto-disable 落地。
- **v1.3** (2026-06-22) — 新增 §15 LLM 最新版本承诺与跟进策略：硬性 2 周 SLA，3 阶段流程（D+3 评估 / D+7 落地 / D+14 验证），15.3 数据流图明确 model ID 存储路径，15.4 UI 透明性规范，15.5 自动检测路线（v0.3+），15.6 vendor 跟进 checklist，15.7 当前最新版本表（待 maintainer 补全）。回应 v0.110 用户反馈 "国产模型只对接最新版本"：把版本跟进从"一次性 hardcode"升级为"持续 SLA"。
- **v1.1** (2026-06-16) — 新增 §13 Client-side key persistence：明确 client paste 为主路径、.env 仅 dev；新增 4 类 IPC（llm_key_set_secret / llm_pm_set_credentials / polyrocket_wallet_set_pk / secrets_status）；keyring alias builder 化；启动同步仅在 `POLYROCKET_ENV=dev && KEYRING_ONLY=0` 时执行。
- **v1.0** (2026-06-16) — 初版。基于用户反馈"LLM 管理 + 流量 + 连通性 + 胜率"需求重写。引入 M11 模块、3 张新表、4 类 IPC 扩展、连通性测试、流量监控、胜率统计增强。