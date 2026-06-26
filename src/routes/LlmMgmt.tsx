// polyrocket — LlmMgmt（v0.68f 密度）。
//
// /llm-mgmt 是 LLM provider + key 管理页。
// 两个关注点，一个界面：
//
//   1. **Providers** —— 已注册的 LLM provider（OpenAI、
//      Anthropic、Google、DeepSeek、自定义 OpenAI-compatible）。
//      每个 provider 有默认 `api_base` 和一组 key
//      （每个 alias 一个）。每个 provider 显示 health 状态
//      （ok / slow / down）。通过 modal 添加，垃圾桶图标删除。
//
//   2. **Keys** —— 每个 provider 的 key alias。每个 key 包含：
//      - alias（例如 "prod-1"、"trade-A"）—— 人类可读标签
//      - secret —— 存储于 OS keyring，从不持久化到磁盘
//      - health —— 上次 `llm_test_connectivity` 结果
//      - 轮换策略（priority / round-robin）
//
// **Sidecar 拓扑**：`llm_test_connectivity` 针对该 key
// 的 `api_base` 发起真实 API 调用。响应中包含 `latency_ms`
// 和 `error_code`，供 Rust 端的熔断逻辑使用。
//
// **用于 env 导入的文件选择器**：Add Key modal 中的
// 「import from .env」按钮通过 `pickFile` + `extractSecretFromEnv`
// 读取 env 格式的文件（`KEY=VALUE` 或纯 `sk-...`）。
// 之后更新 keyring。

import { useState } from 'react';
import { useQuery, useQueryClient, useMutation } from '@tanstack/react-query';
import { Plus, Trash2, TestTube2, CheckCircle2, XCircle, Key, Database, Eye, EyeOff, FolderSearch } from 'lucide-react';
import { llmProviderList, llmKeyList, llmKeySetSecret, llmKeyDelete, llmTestConnectivity, secretsStatus, pickFile } from '@/ipc';
import { extractSecretFromEnv, readFileText } from '@/lib/env-file';
import { Card } from '@/components/base/Card';
import { Pill } from '@/components/base/Pill';
import { Button } from '@/components/base/Button';
import { Input } from '@/components/base/Input';
import { Modal } from '@/components/feedback/Modal';
import { Skeleton } from '@/components/feedback/Skeleton';
import { ErrorState } from '@/components/feedback/ErrorState';
import { EmptyState } from '@/components/feedback/EmptyState';
import { toast } from '@/stores/toast-store';
import { fmtLatency } from '@/lib/format';
import { useT } from '@/lib/i18n';
import type { LlmProvider, LlmProviderKey } from '@/types/llm';

/**
 * `/llm-mgmt` 路由 —— LLM provider / key 管理中心（M11）。
 *
 * **3 个 section**：
 *   1. **Providers** —— provider 列表（kind / enabled / health / cost / quota）
 *   2. **Keys** —— 当前选中 provider 的 key 列表（含 keyring 状态）
 *   3. **Test** —— connectivity 测试 + 实时 health 更新
 *
 * **数据流**：
 *   1. 挂载时调用 `llmProviderList()` + `secretsStatus()`
 *   2. 选 provider → `llmKeyList(providerId)`
 *   3. Add/Edit key → `llmKeyUpsert` mutation（v0.57d+ file picker 路径）
 *   4. Test → `llmTestConnectivity` mutation + 立即刷新 health
 *
 * **关键 IPC**：`llmKeySetSecret`（轮换 secret）/ `llmKeyDelete` / `pickFile` + `extractSecretFromEnv`。
 */
export function LlmMgmt() {
  const { t } = useT();
  const [selectedProvider, setSelectedProvider] = useState<string | null>(null);
  const [addKeyOpen, setAddKeyOpen] = useState(false);
  const queryClient = useQueryClient();

  const providers = useQuery({
    queryKey: ['llm-providers'],
    queryFn: () => llmProviderList(),
  });
  const keys = useQuery({
    queryKey: ['llm-keys', selectedProvider],
    queryFn: () => llmKeyList(selectedProvider ?? undefined),
    enabled: !!selectedProvider,
  });
  const secrets = useQuery({
    queryKey: ['secrets-status'],
    queryFn: () => secretsStatus(),
  });

  const testMut = useMutation({
    mutationFn: ({ providerId, keyId }: { providerId: string; keyId?: string }) =>
      llmTestConnectivity(providerId, keyId),
    onSuccess: (r) => {
      if (r.ok) {
        toast.success(
          t('llmmgmt.test.ok_toast', { provider: r.provider_id }),
          t('llmmgmt.test.ok_latency', { ms: r.latency_ms }),
        );
      } else {
        toast.error(t('llmmgmt.test.failed', { provider: r.provider_id }), r.error_message ?? `code ${r.error_code}`);
      }
      queryClient.invalidateQueries({ queryKey: ['llm-health'] });
    },
  });

  const deleteMut = useMutation({
    mutationFn: (keyId: string) => llmKeyDelete(keyId),
    onSuccess: () => {
      toast.success(t('llmmgmt.keys.toast.deleted'));
      queryClient.invalidateQueries({ queryKey: ['llm-keys'] });
      queryClient.invalidateQueries({ queryKey: ['secrets-status'] });
    },
  });

  return (
    <div className="space-y-4">
      {/* Keyring 状态 */}
      <Card padding="sm">
        <div className="flex items-center gap-3 text-[11px] flex-wrap">
          <span className="text-muted">{t('llmmgmt.keyring')}</span>
          {secrets.isLoading ? (
            <Skeleton className="h-3 w-32" />
          ) : (
            <>
              <Pill kind="muted">
                <Key className="w-2.5 h-2.5" /> {t('llmmgmt.keys_count', { n: secrets.data?.llm_keys ?? 0 })}
              </Pill>
              <Pill kind={secrets.data?.pm_api ? 'bull' : 'muted'}>
                <Database className="w-2.5 h-2.5" /> {t('llmmgmt.pm_api')}
              </Pill>
              <Pill kind={secrets.data?.wallet_pk ? 'bull' : 'muted'}>
                {t('llmmgmt.wallet_pk', { n: secrets.data?.wallet_pk ?? 0 })}
              </Pill>
            </>
          )}
          <span className="text-muted text-[10px]">
            {t('llmmgmt.keyring_backends')}
          </span>
        </div>
      </Card>

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
        {/* Providers（提供者） */}
        <Card title={t('llmmgmt.providers.title')} description={t('llmmgmt.providers.desc')}>
          {providers.isLoading ? (
            <div className="space-y-2">
              {Array.from({ length: 3 }).map((_, i) => (
                <Skeleton key={i} className="h-12" />
              ))}
            </div>
          ) : providers.error ? (
            <ErrorState message={String(providers.error)} onRetry={() => providers.refetch()} />
          ) : !providers.data || providers.data.length === 0 ? (
            <EmptyState title={t('llmmgmt.providers.empty')} description={t('llmmgmt.providers.empty_desc')} />
          ) : (
            <div className="space-y-1.5">
              {providers.data.map((p) => (
                <ProviderRow
                  key={p.id}
                  provider={p}
                  selected={selectedProvider === p.id}
                  onClick={() => setSelectedProvider(p.id === selectedProvider ? null : p.id)}
                  onTest={() => testMut.mutate({ providerId: p.id })}
                  testLabel={t('llmmgmt.providers.test')}
                />
              ))}
            </div>
          )}
        </Card>

        {/* 所选 provider 的 Keys */}
        <Card
          title={selectedProvider ? t('llmmgmt.keys.title_for', { provider: selectedProvider }) : t('llmmgmt.keys.title')}
          description={
            selectedProvider
              ? t('llmmgmt.keys.desc_for')
              : t('llmmgmt.keys.desc')
          }
          action={
            selectedProvider && (
              <Button variant="primary" size="sm" iconLeft={<Plus className="w-3 h-3" />} onClick={() => setAddKeyOpen(true)}>
                {t('llmmgmt.keys.add')}
              </Button>
            )
          }
        >
          {!selectedProvider ? (
            <EmptyState
              icon={<Eye className="w-5 h-5" />}
              title={t('llmmgmt.keys.no_provider')}
              description={t('llmmgmt.keys.no_provider_desc')}
            />
          ) : keys.isLoading ? (
            <div className="space-y-2">
              {Array.from({ length: 2 }).map((_, i) => (
                <Skeleton key={i} className="h-12" />
              ))}
            </div>
          ) : !keys.data || keys.data.length === 0 ? (
            <EmptyState
              icon={<Key className="w-5 h-5" />}
              title={t('llmmgmt.keys.empty')}
              description={t('llmmgmt.keys.empty_desc')}
              action={
                <Button variant="primary" size="sm" iconLeft={<Plus className="w-3 h-3" />} onClick={() => setAddKeyOpen(true)}>
                  {t('llmmgmt.keys.add_first')}
                </Button>
              }
            />
          ) : (
            <div className="space-y-1.5">
              {keys.data.map((k) => (
                <KeyRow
                  key={k.id}
                  keyRow={k}
                  onTest={() => testMut.mutate({ providerId: k.provider_id, keyId: k.id })}
                  onDelete={() => {
                    if (confirm(t('llmmgmt.keys.confirm_delete', { alias: k.alias }))) deleteMut.mutate(k.id);
                  }}
                  testLabel={t('llmmgmt.providers.test')}
                  deleteLabel={t('llmmgmt.keys.delete')}
                  inKeyringLabel={t('llmmgmt.keys.in_keyring')}
                  noSecretLabel={t('llmmgmt.keys.no_secret')}
                />
              ))}
            </div>
          )}
        </Card>
      </div>

      {addKeyOpen && selectedProvider && (
        <AddKeyModal
          providerId={selectedProvider}
          onClose={() => setAddKeyOpen(false)}
          onAdded={() => {
            queryClient.invalidateQueries({ queryKey: ['llm-keys'] });
            queryClient.invalidateQueries({ queryKey: ['secrets-status'] });
            setAddKeyOpen(false);
          }}
        />
      )}
    </div>
  );
}

function ProviderRow({
  provider: p,
  selected,
  onClick,
  onTest,
  testLabel,
}: {
  provider: LlmProvider;
  selected: boolean;
  onClick: () => void;
  onTest: () => void;
  testLabel: string;
}) {
  const statusKind =
    p.health_status === 'ok' ? 'bull' :
    p.health_status === 'slow' ? 'warning' :
    p.health_status === 'failing' ? 'bear' : 'muted';
  return (
    <div
      onClick={onClick}
      className={
        'rounded-md border p-2.5 cursor-pointer transition-colors duration-base ease-out-cubic ' +
        (selected
          ? 'bg-accent/10 border-accent/40'
          : 'bg-surface-2 border-border hover:bg-surface-hover')
      }
    >
      <div className="flex items-center gap-2">
        <div className="flex-1 min-w-0">
          <div className="text-[12px] font-medium text-fg">{p.display_name}</div>
          <div className="text-[10px] text-muted mt-0.5">
            {p.kind} · {p.default_model}
          </div>
        </div>
        {p.health_status && (
          <Pill kind={statusKind as 'bull' | 'warning' | 'bear' | 'muted'}>
            {p.health_status}
            {p.health_latency_p50_ms != null && ` ${fmtLatency(p.health_latency_p50_ms)}`}
          </Pill>
        )}
        <Button variant="ghost" size="xs" iconLeft={<TestTube2 className="w-3 h-3" />} onClick={(e) => { e.stopPropagation(); onTest(); }}>
          {testLabel}
        </Button>
      </div>
    </div>
  );
}

function KeyRow({
  keyRow: k,
  onTest,
  onDelete,
  testLabel,
  deleteLabel,
  inKeyringLabel,
  noSecretLabel,
}: {
  keyRow: LlmProviderKey;
  onTest: () => void;
  onDelete: () => void;
  testLabel: string;
  deleteLabel: string;
  inKeyringLabel: string;
  noSecretLabel: string;
}) {
  return (
    <div className="rounded-md border border-border bg-surface-2 p-2.5 flex items-center gap-2">
      <div className="flex-1 min-w-0">
        <div className="text-[12px] font-medium text-fg">{k.alias}</div>
        <div className="text-[10px] text-muted mt-0.5 font-mono">
          {k.keyring_alias} · priority {k.priority}
        </div>
      </div>
      {k.has_secret ? (
        <Pill kind="bull">
          <CheckCircle2 className="w-2.5 h-2.5" /> {inKeyringLabel}
        </Pill>
      ) : (
        <Pill kind="bear">
          <XCircle className="w-2.5 h-2.5" /> {noSecretLabel}
        </Pill>
      )}
      <Button variant="ghost" size="xs" iconLeft={<TestTube2 className="w-3 h-3" />} onClick={onTest}>
        {testLabel}
      </Button>
      <Button variant="ghost" size="xs" iconLeft={<Trash2 className="w-3 h-3" />} onClick={onDelete}>
        {deleteLabel}
      </Button>
    </div>
  );
}

function AddKeyModal({
  providerId,
  onClose,
  onAdded,
}: {
  providerId: string;
  onClose: () => void;
  onAdded: () => void;
}) {
  const { t } = useT();
  const [alias, setAlias] = useState('');
  const [secret, setSecret] = useState('');
  const [showSecret, setShowSecret] = useState(false);
  const [priority, setPriority] = useState(1);

  const upsertMut = useMutation({
    mutationFn: async () => {
      const { llmKeyUpsert } = await import('@/ipc');
      const key = await llmKeyUpsert({
        provider_id: providerId,
        alias: alias.trim(),
        keyring_alias: `polyrocket/llm/${providerId}/${alias.trim()}`,
        priority,
        enabled: true,
        secret: secret.trim() || undefined,
      });
      if (secret.trim()) {
        await llmKeySetSecret(key.id, secret.trim());
      }
      return key;
    },
    onSuccess: () => {
      toast.success(t('llmmgmt.add.toast.added'));
      onAdded();
    },
    onError: (e: Error) => toast.error(t('llmmgmt.add.toast.failed'), e.message),
  });

  return (
    <Modal
      open
      onClose={onClose}
      title={t('llmmgmt.add.title', { provider: providerId })}
      size="md"
      footer={
        <>
          <Button variant="ghost" size="sm" onClick={onClose}>{t('llmmgmt.add.cancel')}</Button>
          <Button
            variant="primary"
            size="sm"
            loading={upsertMut.isPending}
            disabled={!alias.trim()}
            onClick={() => upsertMut.mutate()}
          >
            {t('llmmgmt.add.add')}
          </Button>
        </>
      }
    >
      <div className="space-y-3">
        <Field label={t('llmmgmt.add.alias')}>
          <Input
            placeholder={t('llmmgmt.add.alias_placeholder')}
            value={alias}
            onChange={(e) => setAlias(e.target.value)}
          />
        </Field>
        <Field label={t('llmmgmt.add.priority')}>
          <Input
            type="number"
            min={1}
            max={100}
            value={priority}
            onChange={(e) => setPriority(Math.max(1, Number(e.target.value) || 1))}
          />
        </Field>
        <Field label={t('llmmgmt.add.secret')}>
          <div className="flex gap-1">
            <Input
              type={showSecret ? 'text' : 'password'}
              placeholder={t('llmmgmt.add.secret_placeholder')}
              value={secret}
              onChange={(e) => setSecret(e.target.value)}
              className="font-mono"
            />
            <Button variant="ghost" size="sm" iconLeft={showSecret ? <EyeOff className="w-3 h-3" /> : <Eye className="w-3 h-3" />} onClick={() => setShowSecret((s) => !s)}>
              {showSecret ? t('llmmgmt.add.hide') : t('llmmgmt.add.show')}
            </Button>
            {/* v0.57d — 原生文件选择器，用于从 `.env`
                形态的文件导入 secret。文件必须正好
                包含一行 KEY=VALUE，且 key 必须匹配所选
                provider 对应的环境变量名
                （OPENAI_API_KEY、ANTHROPIC_API_KEY
                等）。选择器过滤为 .env、.key、.txt。*/}
            <Button
              variant="ghost"
              size="sm"
              iconLeft={<FolderSearch className="w-3 h-3" />}
              data-testid="llm-import-secret-from-file"
              onClick={async () => {
                try {
                  const picked = await pickFile(
                    [
                      { name: 'API key files', extensions: ['env', 'key', 'txt'] },
                    ],
                    false,
                  );
                  if (typeof picked === 'string') {
                    const content = await readFileText(picked);
                    const extracted = extractSecretFromEnv(content);
                    if (extracted) {
                      setSecret(extracted);
                      setShowSecret(true);
                      toast.success(t('llmmgmt.add.imported_from_file'));
                    } else {
                      toast.error(t('llmmgmt.add.import_no_key'));
                    }
                  }
                } catch (err) {
                  toast.error(String(err));
                }
              }}
            >
              {t('llmmgmt.add.import_file')}
            </Button>
          </div>
        </Field>
        <div
          className="text-[11px] text-muted pt-2 border-t border-border"
          // v0.14a — 提示文本包含一个 `<code>` 元素，
          // 其内为 keyring 路径。i18n 字符串直接嵌入
          // 标签（不拆分为 React 节点），以便中/英文
          // 都能以相同方式格式化。
          dangerouslySetInnerHTML={{
            __html: t('llmmgmt.add.notice', {
              path: `polyrocket/llm/${providerId}/&lt;alias&gt;`,
            }),
          }}
        />
      </div>
    </Modal>
  );
}

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div>
      <div className="text-[11px] text-muted mb-1">{label}</div>
      {children}
    </div>
  );
}
