# LLM Providers — polyrocket v0.110.2

> 2026-06-22 · bundled as `v0.110.2-final`
>
> **v0.110 — 优先支持国产大模型最新版本**: 加 5 个 Tier 1 国产 OpenAI 兼容 provider
> (Qwen / Doubao / Kimi / GLM / MiniMax) 到 `/welcome` LLM step。零 Rust 改动,纯 UI + 文档。
>
> **v0.110.2 — `default_model` 落库**: 之前 v0.110 的 `defaultModel` 字段只写在前端 `PROVIDERS` 数组,
> LlmStep onAdd 没把 `default_model` 写进 SQLite,dispatch 层会回退到 LlmProviderDto
> 服务端默认值。v0.110.2 在 onAdd 头部加 `llmProviderUpsert` 调用,把 `defaultModel`
> 真正持久化。同时把 5 个国产 provider 的 defaultModel 字段由 placeholder 改为
> 联网查证的最新 stable model ID (Qwen `qwen3.7-max` / Doubao `doubao-seed-2-0-pro-260215` /
> Kimi `kimi-k2.7-code` / GLM `glm-5.2` / MiniMax `MiniMax-M2.7`),详见
> `polyrocket-llm-management.md` §15.7。

---

## 1. 支持的 10 个 provider (4 个原生 + 5 个国产 OpenAI 兼容 + Custom)

| # | Provider | 协议 | 厂商 | 入口 | 当前 defaultModel (v0.110.2 落地) | Tier |
|---|---|---|---|---|---|---|
| 1 | **OpenAI** | `chat/completions` | OpenAI | `https://api.openai.com/v1` | `gpt-4o` | 1 (原生) |
| 2 | **Anthropic** | `messages` | Anthropic | `https://api.anthropic.com` | `claude-sonnet-4-20250514` | 1 (原生) |
| 3 | **Google** | Gemini API | Google | `https://generativelanguage.googleapis.com` | `gemini-2.0-flash` | 1 (原生) |
| 4 | **DeepSeek** | `chat/completions` | DeepSeek | `https://api.deepseek.com` | `deepseek-chat` | 1 (原生) |
| **5** | **通义千问 (Qwen)** | OpenAI 兼容 | Alibaba 阿里 | `dashscope.aliyuncs.com/compatible-mode/v1` | **`qwen3.7-max`** (Q3 2026) | 1 (国产 #1) |
| **6** | **豆包 (Doubao)** | OpenAI 兼容 | 字节火山 | `ark.cn-beijing.volces.com/api/v3` | **`doubao-seed-2-0-pro-260215`** (2026-02-14) | 1 (国产 #2) |
| **7** | **Kimi (Moonshot)** | OpenAI 兼容 | 月之暗面 | `api.moonshot.cn/v1` | **`kimi-k2.7-code`** (Coding SOTA) | 1 (国产 #3) |
| **8** | **GLM (智谱)** | OpenAI 兼容 | BigModel | `open.bigmodel.cn/api/paas/v4` | **`glm-5.2`** (1M ctx, 2026 Q2) | 1 (国产 #4) |
| **9** | **MiniMax** | OpenAI 兼容 | MiniMax | `api.minimax.chat/v1` | **`MiniMax-M2.7`** (稳定版) | 1 (国产 #5) |
| 10 | **Custom** | OpenAI 兼容 | any | (用户填) | (用户填) | 1 (通用) |

代码入口:
- 协议分发: `src-tauri/src/domain/llm/dispatch.rs`
- 4 个原生 client: `src-tauri/src/domain/llm/{openai,anthropic,google,deepseek}.rs`
- OpenAI 兼容 (含国产 5 个 + Custom): `src-tauri/src/domain/llm/custom.rs` (`CustomClient(OpenaiCompat)`)
- L1 UI 列表: `src/components/welcome/LlmStep.tsx:41-66` (10 个 provider 卡片)

---

## 2. 配置流程 (用户视角)

```
/welcome → LLM step
   ↓
选择 provider (10 个选项)
   ↓ (选 "通义千问 Qwen" 例子)
自动填 api_base = https://dashscope.aliyuncs.com/compatible-mode/v1
   ↓
填 alias (e.g. "aliyun-prod-1") + secret (sk-...)
   ↓
点击 Add
   ↓
keyring 写入 → connectivity test 跑通 → ok=true
   ↓
/llm-mgmt 路由 → "已配置 1 个 provider" 显示
   ↓
/llm-perf 路由可以看每个 provider 的性能
```

**注意**: 国产 5 个 provider 走 OpenAI 兼容协议,**不需要写新 Rust client**。`CustomClient::call_openai` 走标准 `chat/completions` 流程,只是 `api_base` 指向各厂商端点。模型 ID 在调用时由 Rust 侧从 `default_model` 字段取。

---

## 3. 各 provider 关键差异

| Provider | 认证 | 端点注意 | 限流 (默认) |
|---|---|---|---|
| **Qwen** | Bearer token (`sk-...`) | `/compatible-mode/v1` 是阿里为 OpenAI 兼容开放的 v1 端点 | 60 RPM / 1000 RPD (按 tier) |
| **Doubao** | API Key (火山控制台发) | `/api/v3` (不是 `/v1`) | 100 RPM (按模型) |
| **Kimi** | Bearer token (`sk-...`) | 标准 `/v1` | 60 RPM (按 tier) |
| **GLM** | Bearer token (`...`) | `/api/paas/v4` (不是 `/v1`) | 60 RPM (按 tier) |
| **MiniMax** | Bearer token (`ey...`) | 标准 `/v1` | 60 RPM (按 tier) |

如果遇到 404 / model not found,通常是模型 ID 写错。`/llm-mgmt` 的 provider 卡片有 "default_model" 字段可改。

---

## 4. 之后 (Tier 2 / Tier 3 — 留到 v0.111+)

- **Tier 2 自定义协议** (需写新 Rust client):
  - **ERNIE (百度 千帆)** — 自有 AK/SK 鉴权,不同 endpoint → `ernie.rs` (3 天工作)
  - **Hunyuan (腾讯 混元)** — TC3-HMAC-SHA256 签名 → `hunyuan.rs` (2 天)
  - **Spark (讯飞)** — WebSocket 协议 → `spark.rs` (5 天)

- **Tier 3 开源本地**:
  - Qwen3 / GLM-4.5 / DeepSeek-R1 / Yi 都开源
  - 走 Ollama / vLLM → 通过 OpenAI 协议 → 已经支持 (用 "Custom" preset + `http://localhost:11434/v1`)

---

## 5. v0.110.2 文件改动

```
src/components/welcome/LlmStep.tsx              M     +50 lines (PROVIDERS[i].defaultModel 字段 + llmProviderUpsert onAdd 步骤)
src/components/welcome/LlmStep.test.tsx        M     +110 lines (6 new tests,含 5 个 defaultModel 落地断言)
docs/llm-providers.md                          M     §1 表格更新 current defaultModel + 头部 v0.110 → v0.110.2
docs/polyrocket-llm-management.md              M     §15.7 placeholder → 真实 model ID 表格
docs/overview.md                               M     v2.64 → v2.65 (v0.110.2 入口)
docs/coding-spec.md                            M     v2.23 → v2.24 (v0.110.2 入口)
docs/polyrocket-v0.110.2-final.md              NEW   ship log
```

> 16 vitest tests in LlmStep.test.tsx, all green. 5/5 CI jobs green locally.
> No codegen drift. No new Rust client (CustomClient handles 5 OpenAI compat).

---

> CI 5/5 green locally · no GHA · no codegen drift · only 1 Rust client needed (already exists: CustomClient for OpenAI compat)