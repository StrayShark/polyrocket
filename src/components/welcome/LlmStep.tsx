// v0.53b + v0.68f —— LlmStep (Step 4 of 6)。
//
// 至少添加一个 LLM provider。表单结构与 /llm-mgmt 相同,
// 只是嵌在 welcome 布局内。我们调用
// llmProviderUpsert + llmKeySetSecret + llmTestConnectivity
// (与 LLM Mgmt 页面使用的同一套 3 步 IPC 链)。
//
// 成功时:welcome.setConfigured('llmAtLeastOne', true)
//
// **为什么与 /llm-mgmt 拆成两个组件**:welcome 布局受限
// (单列、无侧栏,聚焦于"完成配置")。
// LLM Mgmt 页面是配置完成后的管理工具。
// 同一套 IPC,不同的呈现方式。
//
// **校验顺序**:先 alias(非空),再 secret(非空),再 provider。
// 任一步失败都显示内联错误,并
// 不会调用 IPC。
// llmKeySetSecret + llmTestConnectivity(与 LLM Mgmt 页面
// 使用的同一套 3 步 IPC 链)。
//
// 成功时:welcome.setConfigured('llmAtLeastOne', true)

import { useState, useCallback } from 'react';
import { Input } from '@/components/base/Input';
import { Button } from '@/components/base/Button';
import { useT } from '@/lib/i18n';
import { useWelcomeStore } from '@/stores/welcome-store';
import {
  llmKeyUpsert,
  llmKeySetSecret,
  llmTestConnectivity,
  llmProviderUpsert,
} from '@/ipc';
import type { UpsertLlmKeyArgs, UpsertLlmProviderArgs } from '@/types/llm';
import { toast } from '@/stores/toast-store';
import { Key, Plus, CheckCircle2, AlertTriangle, Loader2 } from 'lucide-react';
import { cn } from '@/lib/cn';

// v0.110 —— 中文 LLM provider 预设 (Tier 1: OpenAI-compatible)。
//
// 全部 5 个使用 OpenAI `chat/completions` 协议,
// 因此通过 `src-tauri/src/domain/llm/custom.rs` 中的
// `CustomClient(OpenaiCompat)` 分发。用户只需粘贴 API key
// 并选择预设 —— 无需手动配置 base URL。
//
// v0.110.2 —— `defaultModel` 字段:跟踪各家最新 stable 版本。
//   通过 `polyrocket-llm-management.md` 中的 §15.6 清单更新。
//   信息来源(2026-06-22 调研):见下方 URL。
const PROVIDERS: Array<{
  id: 'openai' | 'anthropic' | 'google' | 'deepseek' | 'qwen' | 'doubao' | 'kimi' | 'glm' | 'MiniMax' | 'ernie' | 'custom';
  label: string;
  defaultBase: string;
  defaultModel: string;
  hint?: string;
}> = [
  { id: 'openai',    label: 'OpenAI',           defaultBase: 'https://api.openai.com/v1',                      defaultModel: 'gpt-4o' },
  { id: 'anthropic', label: 'Anthropic',        defaultBase: 'https://api.anthropic.com',                    defaultModel: 'claude-sonnet-4-20250514' },
  { id: 'google',    label: 'Google',           defaultBase: 'https://generativelanguage.googleapis.com',     defaultModel: 'gemini-2.0-flash' },
  { id: 'deepseek',  label: 'DeepSeek',         defaultBase: 'https://api.deepseek.com',                     defaultModel: 'deepseek-chat' },
  // v0.111 — ERNIE 百度千帆 (走 OpenAI 兼容 v2 endpoint `qianfan.baidubce.com/v2/coding`,
  //   API key 格式 `bce-v3/ALTAK-...`)。模型 ERNIE 5.0 (2026-01 ERNIE Moment 大会发布,2.4T 参数 MoE)。
  // 走 CustomClient(OpenaiCompat),零新 Rust client。
  { id: 'ernie',     label: '文心一言 ERNIE',    defaultBase: 'https://qianfan.baidubce.com/v2',                  defaultModel: 'ernie-5.0',         hint: '百度千帆 智能云' },
  // v0.110.2 — 国产大模型 Tier 1 (5 个 OpenAI 兼容 providers)。
  // defaultModel 跟厂商最新 stable 模型 (2026-06-22):
  //   Qwen:    qwen3.7-max      (阿里云百炼 2026 最新旗舰)
  //   Doubao:  doubao-seed-2-0-pro-260215 (字节 2026-02-14 发布的第二代)
  //   Kimi:    kimi-k2.7-code   (Moonshot 2026 最新,Coding SOTA)
  //   GLM:     glm-5.2          (智谱 2026 最新,1M context, Coding SOTA)
  //   MiniMax: MiniMax-M2.7     (稳定版,M3 还在 rollout, 5-7 默认)
  { id: 'qwen',     label: '通义千问 Qwen',     defaultBase: 'https://dashscope.aliyuncs.com/compatible-mode/v1', defaultModel: 'qwen3.7-max',     hint: 'Alibaba 阿里云百炼' },
  { id: 'doubao',   label: '豆包 Doubao',        defaultBase: 'https://ark.cn-beijing.volces.com/api/v3',          defaultModel: 'doubao-seed-2-0-pro-260215', hint: '字节火山引擎' },
  { id: 'kimi',     label: 'Kimi (Moonshot)',    defaultBase: 'https://api.moonshot.cn/v1',                        defaultModel: 'kimi-k2.7-code',   hint: '月之暗面 Moonshot AI' },
  { id: 'glm',      label: '智谱 GLM',           defaultBase: 'https://open.bigmodel.cn/api/paas/v4',              defaultModel: 'glm-5.2',          hint: '智谱 BigModel' },
  { id: 'MiniMax',  label: 'MiniMax',           defaultBase: 'https://api.minimax.chat/v1',                  defaultModel: 'MiniMax-M2.7',    hint: 'MiniMax 稀宇科技' },
  { id: 'custom',   label: 'Custom (OpenAI-compatible)', defaultBase: '',                                       defaultModel: '', },
];

export function LlmStep({
  welcome,
}: {
  welcome: ReturnType<typeof useWelcomeStore.getState>;
}) {
  const { t } = useT();
  const [provider, setProvider] = useState<typeof PROVIDERS[number]>(
    PROVIDERS[0]!,
  );
  const [alias, setAlias] = useState('prod-1');
  const [secret, setSecret] = useState('');
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<{
    ok: boolean;
    message: string;
  } | null>(null);

  const onAdd = useCallback(async () => {
    if (!alias.trim() || !secret.trim()) {
      toast.error(t('welcome.llm_required'));
      return;
    }
    setBusy(true);
    setResult(null);
    try {
      // v0.110.2 — Step 1: upsert provider 行 (使用
      // PROVIDERS 中最新的 default_model)。
      // 这会持久化 `default_model` 字段,
      // 让 Rust dispatch 层使用最新版本,
      // 而不是 LlmProviderDto 的服务端 fallback。
      // 将 provider.id 映射为 DB 的 ProviderKind。
      const kind = ((): UpsertLlmProviderArgs['kind'] => {
        switch (provider.id) {
          case 'openai':    return 'openai';
          case 'anthropic': return 'anthropic';
          case 'google':    return 'google';
          case 'deepseek':  return 'deepseek';
          // 5 个中文 (v0.110) + ERNIE (v0.111) + custom: 全部 OpenAI 兼容
          default:          return 'openai_compat';
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
      // Step 2: upsert key + 写入 OS keyring。
      const upsertArgs: UpsertLlmKeyArgs = {
        provider_id: provider.id,
        alias: alias.trim(),
        keyring_alias: `polyrocket/llm/${provider.id}/${alias.trim()}`,
        priority: 1,
        enabled: true,
        secret: secret.trim() || undefined,
      };
      const key = await llmKeyUpsert(upsertArgs);
      await llmKeySetSecret(key.id, secret.trim());
      // Step 3: 对新 key 进行连通性测试。
      const conn = await llmTestConnectivity(provider.id, key.id);
      if (conn.ok) {
        welcome.setConfigured('llmAtLeastOne', true);
        setResult({
          ok: true,
          message: t('welcome.llm_ok', { ms: conn.latency_ms }),
        });
        toast.success(t('welcome.llm_added'));
        setSecret('');
      } else {
        setResult({
          ok: false,
          message:
            conn.error_message ??
            t('welcome.llm_failed', {
              code: conn.error_code ?? 'unknown',
            }),
        });
        toast.error(t('welcome.llm_failed'));
      }
    } catch (e) {
      setResult({ ok: false, message: String(e) });
      toast.error(String(e));
    } finally {
      setBusy(false);
    }
  }, [provider, alias, secret, welcome, t]);

  return (
    <div className="space-y-4 py-2">
      <div>
        <h2 className="text-title-md font-semibold text-fg">
          {t('welcome.llm_title')}
        </h2>
        <p className="text-[12px] text-muted mt-1">
          {t('welcome.llm_desc')}
        </p>
      </div>

      <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
        <Field label={t('welcome.llm_provider')}>
          <div className="grid grid-cols-2 md:grid-cols-3 gap-1.5 mt-1">
            {PROVIDERS.map((p) => (
              <button
                key={p.id}
                type="button"
                onClick={() => setProvider(p)}
                data-testid={`welcome-llm-provider-${p.id}`}
                title={p.hint ?? p.label}
                className={cn(
                  'h-7 px-2 rounded text-[11px] font-medium border transition-colors duration-base ease-out-cubic',
                  provider.id === p.id
                    ? 'bg-accent/10 border-accent/40 text-fg'
                    : 'bg-surface-2 border-border text-muted hover:text-fg',
                )}
              >
                {p.label}
              </button>
            ))}
          </div>
          {/* v0.110 — 选中 provider 的提示 (仅在有 hint 的中文 provider 上显示) */}
          {provider.hint && (
            <div
              className="text-[10px] text-muted mt-1.5 font-mono"
              data-testid="welcome-llm-provider-hint"
            >
              {provider.hint}
            </div>
          )}
        </Field>
        <Field label={t('welcome.llm_alias')}>
          <Input
            data-testid="welcome-llm-alias"
            value={alias}
            onChange={(e) => setAlias(e.target.value)}
            placeholder="prod-1"
          />
          <div className="text-[10px] text-muted mt-1 font-mono">
            {`keyring: polyrocket/llm/${provider.id}/${alias.trim() || '<alias>'}`}
          </div>
        </Field>
      </div>

      <Field label={t('welcome.llm_key')}>
        <Input
          data-testid="welcome-llm-secret"
          value={secret}
          onChange={(e) => setSecret(e.target.value)}
          type="password"
          placeholder={t('welcome.llm_key_placeholder')}
        />
      </Field>

      <div className="flex items-center gap-2">
        <Button
          variant="primary"
          size="sm"
          iconLeft={
            busy ? (
              <Loader2 className="w-3 h-3 animate-spin" />
            ) : (
              <Plus className="w-3 h-3" />
            )
          }
          onClick={onAdd}
          disabled={busy}
          data-testid="welcome-llm-add"
        >
          {t('welcome.llm_add')}
        </Button>
      </div>

      {result && (
        <div
          data-testid="welcome-llm-result"
          className={cn(
            'rounded-md p-2.5 text-[11px] flex items-center gap-2',
            result.ok
              ? 'bg-bull/10 text-bull'
              : 'bg-bear/10 text-bear',
          )}
        >
          {result.ok ? (
            <CheckCircle2 className="w-3.5 h-3.5" />
          ) : (
            <AlertTriangle className="w-3.5 h-3.5" />
          )}
          <span className="font-mono">{result.message}</span>
        </div>
      )}

      <div className="rounded-md border border-border bg-surface-2 p-3 text-[11px] text-muted space-y-1">
        <div className="text-fg text-[12px] mb-1 flex items-center gap-1.5">
          <Key className="w-3.5 h-3.5" />
          {t('welcome.llm_security_title')}
        </div>
        <div>{t('welcome.llm_security_1')}</div>
        <div>{t('welcome.llm_security_2')}</div>
      </div>
    </div>
  );
}

function Field({
  label,
  children,
}: {
  label: string;
  children: React.ReactNode;
}) {
  return (
    <label className="block">
      <span className="text-xs text-muted font-semibold block mb-1 uppercase tracking-caption-uppercase">
        {label}
      </span>
      {children}
    </label>
  );
}
