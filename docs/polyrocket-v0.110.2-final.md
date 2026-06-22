# polyrocket v0.110.2 — `default_model` 落库 + 国产 provider 最新 model ID 落地

> 2026-06-22 · bundled as `v0.110.2-final`
>
> **v0.110.2 — 修正 v0.110 的关键 gap**: v0.110 加了 5 个国产 OpenAI 兼容 provider 到
> LlmStep UI,但 `onAdd` handler 没把 `default_model` 字段写进 SQLite。Rust dispatch
> 层会回退到 `LlmProviderDto::default()` (server-side fallback, 2025 年 5 月的
> 旧 model ID),用户实际跑的 model 跟 UI 显示的"最新"对不上。
>
> **v0.110.2 解决**: (1) `onAdd` 头部加 `llmProviderUpsert` 调用,把 `defaultModel`
> 真正持久化到 SQLite; (2) 5 个国产 provider 的 `defaultModel` 字段由 placeholder
> 改为联网查证的最新 stable model ID (Qwen `qwen3.7-max` / Doubao
> `doubao-seed-2-0-pro-260215` / Kimi `kimi-k2.7-code` / GLM `glm-5.2` /
> MiniMax `MiniMax-M2.7`)。详见 `polyrocket-llm-management.md` §15.7。

---

## 1. 关键问题: `defaultModel` 字段没有真正生效

### 1.1 v0.110 实际行为

v0.110 在 LlmStep.tsx 加了 5 个国产 provider, 每个带 `defaultModel` 字段:

```ts
{ id: 'qwen', label: '通义千问 Qwen', defaultBase: '...', defaultModel: 'Qwen3-Max' }
```

但 `onAdd` handler 只调了 3 个 IPC:

```ts
const key = await llmKeyUpsert(upsertArgs);     // writes key row
await llmKeySetSecret(key.id, secret.trim());    // writes OS keyring
const conn = await llmTestConnectivity(...);     // pings API
```

**没有调 `llmProviderUpsert`**, 所以 `llm_providers` 表里的 `default_model`
字段保持 SQL schema default (`LlmProviderDto::default()` 的 string fallback)。
用户 `/llm-mgmt` 看到 "default_model: Qwen3-Max" 是 UI 初始值, Rust 实际用的
是后端默认的旧 ID, **UI 和 dispatch 层不对齐**。

### 1.2 v0.110.2 修正

在 `onAdd` 头部加 `llmProviderUpsert` 调用:

```ts
// Step 1: upsert the provider row (with the latest default_model from PROVIDERS)
const kind = ((): UpsertLlmProviderArgs['kind'] => {
  switch (provider.id) {
    case 'openai':    return 'openai';
    case 'anthropic': return 'anthropic';
    case 'google':    return 'google';
    case 'deepseek':  return 'deepseek';
    default:          return 'openai_compat';  // qwen/doubao/kimi/glm/MiniMax/custom
  }
})();
const providerUpsertArgs: UpsertLlmProviderArgs = {
  id: provider.id,
  display_name: provider.label,
  kind,
  api_base: provider.defaultBase || undefined,
  default_model: provider.defaultModel,
  enabled: true,
  timeout_ms: 30000,
  max_retries: 2,
};
await llmProviderUpsert(providerUpsertArgs);
// Step 2: key + secret + connectivity
```

**5 个 `kind` mapping** 已经在 v0.110 的 `commands/llm.rs:765-772` 加过 match
arms, v0.110.2 复用, 零新 Rust 代码。

### 1.3 5 个国产 provider 最新 model ID (联网查证)

> 调研方法: `web_search` (MCP `matrix` 提供) + `webfetch` 抓厂商官方文档。
> 调研日期 2026-06-22。详见 `polyrocket-llm-management.md` §15.7。

| Provider | v0.110 placeholder | v0.110.2 latest stable | 厂商文档 |
|---|---|---|---|
| Qwen | `Qwen3-Max` (2025 旧) | **`qwen3.7-max`** (Q3 2026) | [Alibaba Model Studio](https://help.aliyun.com/zh/model-studio/getting-started/models) |
| Doubao | `Doubao-1.5-Pro` (2025 旧) | **`doubao-seed-2-0-pro-260215`** (2026-02-14) | [字节火山方舟](https://www.volcengine.com/docs/82379) |
| Kimi | `kimi-k2` (2025 旧) | **`kimi-k2.7-code`** (2026 Coding SOTA) | [Moonshot 平台](https://platform.moonshot.cn/docs/intro) |
| GLM | `GLM-4.5` (2025 旧) | **`glm-5.2`** (2026 Q2, 1M ctx) | [BigModel 模型概览](https://open.bigmodel.cn/cn/guide/start/model-overview) |
| MiniMax | `MiniMax-Text-01` (2025 旧) | **`MiniMax-M2.7`** (stable) | [MiniMax API 文档](https://api.minimax.chat/document) |

---

## 2. 改动清单

```
src/components/welcome/LlmStep.tsx              M     +50 lines (5 defaultModel 字段 + llmProviderUpsert onAdd 步骤)
src/components/welcome/LlmStep.test.tsx        M     +110 lines (5 new tests, +1 mock)
docs/llm-providers.md                          M     头部 v0.110 → v0.110.2 + §1 表格 current defaultModel 列填实 + §5 文件改动
docs/polyrocket-llm-management.md              M     §15.7 placeholder → 真实表格 + v1.3.1 changelog
docs/overview.md                               M     头部 v2.64 → v2.65 + 2 changelog entries (v2.64 v0.110 + v2.65 v0.110.2)
docs/coding-spec.md                            M     头部 v2.23 → v2.24 + 2 changelog rows
docs/polyrocket-v0.110.2-final.md              NEW   this file
```

**No new Rust files**, **no new IPC**, **no codegen change**. v0.110.2 是
"修正已有功能 + 文档同步" 性质的 round, 与 v0.110 配合把国产 LLM 政策完整落地。

---

## 3. 测试

### 3.1 新增 5 个测试 (LlmStep.test.tsx 11 → 16)

| Test | Asserts |
|---|---|
| `successful add: provider upsert + key upsert + setSecret + test + setConfigured` | openai: kind=openai, default_model=gpt-4o, api_base=https://api.openai.com/v1 |
| `GLM add uses glm-5.2 as default_model` | glm: kind=openai_compat, default_model=glm-5.2, api_base=open.bigmodel.cn/api/paas/v4 |
| `Qwen add uses qwen3.7-max as default_model` | qwen: default_model=qwen3.7-max |
| `Doubao add uses doubao-seed-2-0-pro-260215` | doubao: default_model=doubao-seed-2-0-pro-260215 |
| `Kimi add uses kimi-k2.7-code` | kimi: default_model=kimi-k2.7-code |
| `MiniMax add uses MiniMax-M2.7` | MiniMax: default_model=MiniMax-M2.7 |

### 3.2 测试 setup 关键 fix

LlmStep.test.tsx v0.110 原版在 `beforeEach` 漏了 `mockLlmProviderUpsert.mockReset()`,
导致 `mock.calls[0]` 跨 test 共享 (前一个 test 的 qwen upsert 数据被后一个 test 读)。
v0.110.2 修:

```ts
beforeEach(() => {
  mockLlmKeyUpsert.mockReset();
  mockLlmKeySetSecret.mockReset();
  mockLlmTestConnectivity.mockReset();
  mockLlmProviderUpsert.mockReset();        // v0.110.2 — was missing
  mockLlmProviderUpsert.mockResolvedValue(undefined);
});
```

### 3.3 测试结果

```
$ pnpm vitest run src/components/welcome/LlmStep.test.tsx
Test Files  1 passed (1)
Tests  16 passed (16)
Duration  411ms
```

---

## 4. 跟 §15 SLA 的关系

v0.110.1 (`8f30baf`) 新增 `polyrocket-llm-management.md §15` "LLM 最新版本承诺与跟进策略",
定义 2 周硬 SLA:

| 阶段 | 时间 | 行动 |
|---|---|---|
| D+0 | 厂商发布新版 | 厂商 changelog / 公告 |
| D+3 | maintainer 评估 | 评估 breaking change / 端点 / API shape |
| D+7 | maintainer 落地 | 改 `LlmStep.tsx` `defaultModel` + 改 `§15.7` 表格 + 改 `llm-providers.md` §1 |
| D+14 | CI + 用户验证 | connectivity test + 用户复审 |

v0.110.2 是 §15 SLA 的**第一次真实"填写 → 落库"演练**:

1. v0.110 留下 placeholder (2025 年的旧 model IDs)
2. v0.110.1 把"hint 字段"从 stale model ID 改为 vendor 名 (LlmStep UX 修复)
3. v0.110.2 由 Mavis 联网查证 5 个最新 stable model ID + 改 `defaultModel` 字段 + 改 `§15.7` 表格 + 改 `llm-providers.md` §1

**未来 SLA 自动化**: Mavis cron 每 14 天触发, 自动 fetch 5 家厂商模型列表,
如果发现比 `§15.7` 表格里更新的 stable version, 自动开 PR 改 `LlmStep.tsx` +
改 `§15.7` + 改 `llm-providers.md` §1。用户复审后 merge 即可。

---

## 5. 完整 file diff 摘要

```
$ git diff --stat v0.110.1..v0.110.2 (this commit)

 src/components/welcome/LlmStep.tsx              | 50 ++++++++++++++++++++++---
 src/components/welcome/LlmStep.test.tsx        | 110 +++++++++++++++++++++++++++++++++++++++++++++++--
 docs/llm-providers.md                          | 18 +++++++----
 docs/polyrocket-llm-management.md              | 22 ++++++++++---
 docs/overview.md                               | 4 +++-
 docs/coding-spec.md                            | 4 +++-
 6 files changed, 199 insertions(+), 9 deletions(-)
```

---

## 6. CI 状态

- **5/5 jobs green locally**: governance / L1+vitest / Rust cargo / Python pytest / Playwright e2e
- **No codegen drift** (no codegen change this round)
- **No new coverage work** (LlmStep tests count from 11 → 16, but coverage was 100% on this file already, project totals unchanged at 89.92/86.66/85.01/91.05)
- **Test count**: 1072 → 1077 (+5, all real)

---

## 7. 后续工作

- **Mavis cron setup**: 添加每 14 天触发一次的 cron, 自动 fetch 5 家厂商模型列表, 触发 §15 SLA
- **§15.5 自动检测**: v0.3+ 路线图, 跑 LLM health probe 时 detect 厂商新模型 (heuristic 匹配)
- **Tier 2 adapters**: ERNIE 千帆 / Hunyuan 混元 TC3 / Spark 讯飞 WebSocket — 需新 Rust client
- **§15.7 SLA dashboard**: 在 /llm-mgmt 页面加 "Latest from vendor" link, 一键跳转 5 家厂商 changelog

---

## 8. 关联文档

- [`docs/llm-providers.md`](./llm-providers.md) — 10 provider matrix + current defaultModel 表
- [`docs/polyrocket-llm-management.md`](./polyrocket-llm-management.md) — §15 LLM 最新版本承诺 + §15.7 当前最新版本表
- [`docs/overview.md`](./overview.md) — v2.65 header + v2.64 v0.110 + v2.65 v0.110.2 changelog
- [`docs/coding-spec.md`](./coding-spec.md) — v2.24 header + v2.23 v0.110 + v2.24 v0.110.2 row
- [`docs/polyrocket-v0.110-final.md`](./polyrocket-v0.110-final.md) — 上一 round ship log
- [`docs/polyrocket-v0.110.1.md`](./polyrocket-v0.110.1.md) — 上一 round docs (LlmStep hint vendor-only)
