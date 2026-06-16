# polyrocket — LLM 管理模块需求 (LMN)

> 版本：v1.0 · 2026-06-16
> 配套：[`polyrocket-llm-analysis.md`](./polyrocket-llm-analysis.md)（M10 业务） · [`polyrocket-modules.md`](./polyrocket-modules.md)（模块全景）
> 模块代号：**M11 — LLM Management**
> 状态：v0.2 路线

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

## 变更日志

- **v1.0** (2026-06-16) — 初版。基于用户反馈"LLM 管理 + 流量 + 连通性 + 胜率"需求重写。引入 M11 模块、3 张新表、4 类 IPC 扩展、连通性测试、流量监控、胜率统计增强。