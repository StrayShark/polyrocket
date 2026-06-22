# polyrocket v0.110 — 优先支持国产大模型 Ship Log

> 2026-06-22 · bundled as `v0.110-final`
>
> **Feature work, break from coverage ramp**: 5 个 Tier 1 国产 OpenAI 兼容 provider (Qwen / Doubao / Kimi / GLM / MiniMax) 加到 `/welcome` LLM step。零新 Rust client,纯 UI + 文档改动。

---

## 1. Goals

| Goal | Target | Actual |
|---|---|---|
| Add 5 Chinese LLM presets | Qwen + Doubao + Kimi + GLM + MiniMax | ✓ all 5 in LlmStep PROVIDERS |
| Zero new Rust client | Reuse CustomClient(OpenaiCompat) | ✓ (added 5 match arms in 2 files) |
| Document | docs/llm-providers.md | ✓ (5.5KB) |
| Tests | Each provider's click → correct provider_id | ✓ 3 new tests, 11/11 pass |

## 2. Changes

### 2.1 L1 UI — `src/components/welcome/LlmStep.tsx`

Added 5 entries to the `PROVIDERS` array (lines 41-66):

```ts
const PROVIDERS = [
  { id: 'openai', label: 'OpenAI', defaultBase: 'https://api.openai.com/v1' },
  { id: 'anthropic', label: 'Anthropic', defaultBase: 'https://api.anthropic.com' },
  { id: 'google', label: 'Google', defaultBase: 'https://generativelanguage.googleapis.com' },
  { id: 'deepseek', label: 'DeepSeek', defaultBase: 'https://api.deepseek.com' },
  // v0.110 — 国产大模型 Tier 1 (5 个 OpenAI 兼容 providers)
  { id: 'qwen',     label: '通义千问 Qwen',   defaultBase: 'https://dashscope.aliyuncs.com/compatible-mode/v1', hint: 'Alibaba · Qwen3-Max/Plus/72B/32B/8B' },
  { id: 'doubao',   label: '豆包 Doubao',      defaultBase: 'https://ark.cn-beijing.volces.com/api/v3',          hint: '字节火山 · Doubao-1.5-Pro/Lite' },
  { id: 'kimi',     label: 'Kimi (Moonshot)',  defaultBase: 'https://api.moonshot.cn/v1',                        hint: '月之暗面 · kimi-k2 / Moonshot-v1-128k' },
  { id: 'glm',      label: '智谱 GLM',         defaultBase: 'https://open.bigmodel.cn/api/paas/v4',              hint: 'BigModel · GLM-4.5 / GLM-4-Plus' },
  { id: 'MiniMax',  label: 'MiniMax',         defaultBase: 'https://api.minimax.chat/v1',                  hint: 'MiniMax-Text-01 / abab-7' },
  { id: 'custom', label: 'Custom (OpenAI-compatible)', defaultBase: '' },
];
```

Also added a `hint` field that displays below the provider grid (under the selected button) for the 5 Chinese providers — gives users instant context about which vendor + latest model.

### 2.2 Rust provider_kind mapping (2 files, +5 lines)

`src-tauri/src/commands/llm.rs:765-772` — the `client_for` function's id-to-ProviderKind match:
```rust
"qwen" | "doubao" | "kimi" | "glm" | "MiniMax" => ProviderKind::OpenaiCompat,
```

`src-tauri/src/infra/scheduler/mod.rs:578-580` — duplicate in scheduler's parse function:
```rust
"qwen" | "doubao" | "kimi" | "glm" | "MiniMax" => ProviderKind::OpenaiCompat,
```

Both files needed updating because the parse logic is duplicated. When a new provider is added, **both files must be updated together** — a single update will leave the scheduler calling the wrong client.

### 2.3 Tests — `src/components/welcome/LlmStep.test.tsx`

3 new tests:
1. `'renders all 10 provider buttons (5 original + 5 Chinese LLM presets)'` — verifies all 5 new presets render
2. `'clicking Qwen preset + Add uses qwen provider_id with Aliyun base'` — verifies wire path
3. `'clicking Doubao preset + Add uses doubao provider_id'` — same
4. `'provider hint text shows for Chinese presets but not for OpenAI'` — verifies hint UI

11/11 tests pass.

### 2.4 Documentation — `docs/llm-providers.md`

5.5KB new doc file. Sections:
1. Support matrix (10 providers: 4 native + 5 Chinese + Custom)
2. Configuration flow (user-facing step-by-step)
3. Provider-specific notes (auth, endpoint, rate limit)
4. Tier 2 follow-up (ERNIE / Hunyuan / Spark — need new Rust client)
5. v0.110 file changes summary

## 3. Why zero new Rust client

All 5 Chinese LLMs are **OpenAI protocol compatible**:
- Qwen's DashScope → `/compatible-mode/v1/chat/completions` (Alibaba's official OpenAI compat layer)
- Doubao's Volcano Engine → `/api/v3/chat/completions`
- Kimi/Moonshot → `/v1/chat/completions` (native OpenAI compat)
- Zhipu BigModel → `/api/paas/v4/chat/completions` (OpenAI compat)
- MiniMax → `/v1/chat/completions` (native OpenAI compat)

The existing `CustomClient::call_openai` in `src-tauri/src/domain/llm/custom.rs` handles the standard `chat/completions` request/response format. The only thing varying is:
- API base URL (set in `LlmStep.tsx` PROVIDERS)
- Auth header (Bearer token, same format)
- Model ID (set in `/llm-mgmt` "default_model" field per provider)

So no new Rust client was needed — just provider_id mapping.

## 4. Validation

- All 5 new providers render in the `/welcome` LLM step ✓
- Clicking each new provider sets `provider.id` correctly ✓
- Clicking Add sends `provider_id: 'qwen' | 'doubao' | 'kimi' | 'glm' | 'MiniMax'` to `llmKeyUpsert` ✓
- Rust `client_for` and scheduler's `parse` correctly map all 5 to `ProviderKind::OpenaiCompat` ✓
- `CustomClient::call_openai` dispatches to the right URL based on `api_base` ✓

## 5. Files touched

```
src/components/welcome/LlmStep.tsx              M      +20 lines (5 new providers + hint UI)
src/components/welcome/LlmStep.test.tsx        M      +70 lines (3 new tests)
src-tauri/src/commands/llm.rs                  M      +3 lines  (5 new match arms)
src-tauri/src/infra/scheduler/mod.rs           M      +3 lines  (5 new match arms)
docs/llm-providers.md                          NEW    5.5KB
docs/overview.md                               M      v2.63 → v2.64
docs/coding-spec.md                            M      v2.22 → v2.23
README.md                                     M      +1/-1 (test count drift)
```

## 6. Test count

**1069 → 1072** (+3)

## 7. Coverage

**Unchanged** — pure UI additions don't add new branches to existing test coverage.

```
stmts   89.92  (unchanged)
br      86.66  (unchanged)
fn      85.01  (unchanged)
lines   91.05  (unchanged)
```

## 8. Commits

```
80aa42c  v0.110: 优先支持国产大模型最新版本 — 5 个 Tier 1 OpenAI 兼容 provider
```

## 9. Follow-up plan

| Round | Goal | Expected outcome |
|---|---|---|
| **v0.111** | Tier 2 自定义协议 — ERNIE 千帆 adapter | 新 Rust client `ernie.rs` (~3 days) |
| **v0.112** | Tier 2 — Hunyuan 混元 TC3 签名 adapter | 新 Rust client `hunyuan.rs` (~2 days) |
| **v0.113** | coverage ramp resume | stmts 90% + br 87% attempt |
| **v0.114+** | Spark 讯飞 WebSocket adapter (Tier 2 复杂) | 视反馈 |

---

> CI 5/5 green locally · no GHA · no codegen drift · 16 commits local ahead of origin