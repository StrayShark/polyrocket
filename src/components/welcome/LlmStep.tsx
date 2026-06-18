// v0.53b — LlmStep (Step 4 of 6).
//
// Add at least one LLM provider. The form mirrors
// what /llm-mgmt does, but inside the welcome
// layout. We invoke llmProviderUpsert +
// llmKeySetSecret + llmTestConnectivity (the same
// 3-step IPC chain the LLM Mgmt page uses).
//
// On success: welcome.setConfigured('llmAtLeastOne', true)

import { useState, useCallback } from 'react';
import { Card } from '@/components/base/Card';
import { Input } from '@/components/base/Input';
import { Button } from '@/components/base/Button';
import { useT } from '@/lib/i18n';
import { useWelcomeStore } from '@/stores/welcome-store';
import {
  llmKeyUpsert,
  llmKeySetSecret,
  llmTestConnectivity,
  type UpsertLlmKeyArgs,
} from '@/ipc';
import { toast } from '@/stores/toast-store';
import { Key, Plus, CheckCircle2, AlertTriangle, Loader2 } from 'lucide-react';
import { cn } from '@/lib/cn';

const PROVIDERS: Array<{
  id: 'openai' | 'anthropic' | 'google' | 'deepseek' | 'custom';
  label: string;
  defaultBase: string;
}> = [
  { id: 'openai', label: 'OpenAI', defaultBase: 'https://api.openai.com/v1' },
  { id: 'anthropic', label: 'Anthropic', defaultBase: 'https://api.anthropic.com' },
  { id: 'google', label: 'Google', defaultBase: 'https://generativelanguage.googleapis.com' },
  { id: 'deepseek', label: 'DeepSeek', defaultBase: 'https://api.deepseek.com' },
  { id: 'custom', label: 'Custom (OpenAI-compatible)', defaultBase: '' },
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
      // Run a connectivity test against the new key.
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
        <h2 className="text-[18px] font-semibold text-fg">
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
                className={cn(
                  'h-7 px-2 rounded text-[11px] font-medium border transition-colors',
                  provider.id === p.id
                    ? 'bg-accent/10 border-accent/40 text-fg'
                    : 'bg-surface-2 border-border text-muted hover:text-fg',
                )}
              >
                {p.label}
              </button>
            ))}
          </div>
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
      <span className="text-[10px] text-muted block mb-1 uppercase tracking-wider">
        {label}
      </span>
      {children}
    </label>
  );
}
