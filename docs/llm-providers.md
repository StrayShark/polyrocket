# LLM Providers — polyrocket v0.110

> 2026-06-22 · bundled as `v0.110-final`
>
> **v0.110 — 优先支持国产大模型最新版本**: 加 5 个 Tier 1 国产 OpenAI 兼容 provider
> (Qwen / Doubao / Kimi / GLM / MiniMax) 到 `/welcome` LLM step。零 Rust 改动,纯 UI + 文档。

---

## 1. 支持的 6 个 provider (4 个原生 + 5 个国产 OpenAI 兼容)

| # | Provider | 协议 | 厂商 | 入口 | 最新模型 (2026) | Tier |
|---|---|---|---|---|---|---|
| 1 | **OpenAI** | `chat/completions` | OpenAI | `https://api.openai.com/v1` | GPT-4o / GPT-4 Turbo / o1 / o3 | 1 (原生) |
| 2 | **Anthropic** | `messages` | Anthropic | `https://api.anthropic.com` | Claude 3.5/4 Sonnet/Opus/Haiku | 1 (原生) |
| 3 | **Google** | Gemini API | Google | `https://generativelanguage.googleapis.com` | Gemini 1.5/2.0 Pro/Flash | 1 (原生) |
| 4 | **DeepSeek** | `chat/completions` | DeepSeek | `https://api.deepseek.com` | DeepSeek-V3 / DeepSeek-R1 | 1 (原生) |
| **5** | **通义千问 (Qwen)** | OpenAI 兼容 | Alibaba 阿里 | `dashscope.aliyuncs.com/compatible-mode/v1` | **Qwen3-Max / Qwen3-Plus / Qwen3-72B/32B/8B** | 1 (国产 #1) |
| **6** | **豆包 (Doubao)** | OpenAI 兼容 | 字节火山 | `ark.cn-beijing.volces.com/api/v3` | **Doubao-1.5-Pro / Doubao-Lite** | 1 (国产 #2) |
| **7** | **Kimi (Moonshot)** | OpenAI 兼容 | 月之暗面 | `api.moonshot.cn/v1` | **kimi-k2 / Moonshot-v1-128k** | 1 (国产 #3) |
| **8** | **GLM (智谱)** | OpenAI 兼容 | BigModel | `open.bigmodel.cn/api/paas/v4` | **GLM-4.5 / GLM-4-Plus** | 1 (国产 #4) |
| **9** | **MiniMax** | OpenAI 兼容 | MiniMax | `api.minimax.chat/v1` | **MiniMax-Text-01 / abab-7** | 1 (国产 #5) |
| 10 | **Custom** | OpenAI 兼容 | any | (用户填) | 任何 OpenAI 兼容 (Groq / xAI / Mistral / Perplexity / Together / Fireworks / OpenRouter / Azure / Ollama / vLLM / llama.cpp) | 1 (通用) |

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

## 5. v0.110 文件改动

```
src/components/welcome/LlmStep.tsx              M     +20 lines (5 new providers + hint UI)
src/components/welcome/LlmStep.test.tsx        M     +70 lines (3 new tests)
docs/llm-providers.md                          NEW   this file
docs/overview.md                               M     v2.63 → v2.64
docs/coding-spec.md                            M     v2.22 → v2.23
docs/polyrocket-v0.110-final.md                NEW   ship log
```

---

> CI 5/5 green locally · no GHA · no codegen drift · only 1 Rust client needed (already exists: CustomClient for OpenAI compat)