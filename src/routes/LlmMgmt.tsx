import { useState } from 'react';
import { useQuery, useQueryClient, useMutation } from '@tanstack/react-query';
import { Plus, Trash2, TestTube2, CheckCircle2, XCircle, Key, Database, Eye, EyeOff } from 'lucide-react';
import { llmProviderList, llmKeyList, llmKeySetSecret, llmKeyDelete, llmTestConnectivity, secretsStatus } from '@/ipc';
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
import type { LlmProvider, LlmProviderKey } from '@/types/llm';

export function LlmMgmt() {
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
        toast.success(`${r.provider_id} OK`, `latency ${r.latency_ms}ms`);
      } else {
        toast.error(`${r.provider_id} failed`, r.error_message ?? `code ${r.error_code}`);
      }
      queryClient.invalidateQueries({ queryKey: ['llm-health'] });
    },
  });

  const deleteMut = useMutation({
    mutationFn: (keyId: string) => llmKeyDelete(keyId),
    onSuccess: () => {
      toast.success('Key deleted');
      queryClient.invalidateQueries({ queryKey: ['llm-keys'] });
      queryClient.invalidateQueries({ queryKey: ['secrets-status'] });
    },
  });

  return (
    <div className="space-y-4">
      {/* Keyring status */}
      <Card padding="sm">
        <div className="flex items-center gap-3 text-[11px] flex-wrap">
          <span className="text-muted">OS keyring:</span>
          {secrets.isLoading ? (
            <Skeleton className="h-3 w-32" />
          ) : (
            <>
              <Pill kind="muted">
                <Key className="w-2.5 h-2.5" /> {secrets.data?.llm_keys ?? 0} LLM keys
              </Pill>
              <Pill kind={secrets.data?.pm_api ? 'bull' : 'muted'}>
                <Database className="w-2.5 h-2.5" /> PM API
              </Pill>
              <Pill kind={secrets.data?.wallet_pk ? 'bull' : 'muted'}>
                wallet PK × {secrets.data?.wallet_pk ?? 0}
              </Pill>
            </>
          )}
          <span className="text-muted text-[10px]">
            (macOS Keychain / Windows Credential Manager / Linux Secret Service)
          </span>
        </div>
      </Card>

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
        {/* Providers */}
        <Card title="Providers" description="Enabled LLM providers">
          {providers.isLoading ? (
            <div className="space-y-2">
              {Array.from({ length: 3 }).map((_, i) => (
                <Skeleton key={i} className="h-12" />
              ))}
            </div>
          ) : providers.error ? (
            <ErrorState message={String(providers.error)} onRetry={() => providers.refetch()} />
          ) : !providers.data || providers.data.length === 0 ? (
            <EmptyState title="No providers" description="Add a provider in v0.4.1 settings." />
          ) : (
            <div className="space-y-1.5">
              {providers.data.map((p) => (
                <ProviderRow
                  key={p.id}
                  provider={p}
                  selected={selectedProvider === p.id}
                  onClick={() => setSelectedProvider(p.id === selectedProvider ? null : p.id)}
                  onTest={() => testMut.mutate({ providerId: p.id })}
                />
              ))}
            </div>
          )}
        </Card>

        {/* Keys for selected provider */}
        <Card
          title={selectedProvider ? `Keys for ${selectedProvider}` : 'Keys'}
          description={
            selectedProvider
              ? 'OS-keyring-backed API keys for this provider'
              : 'Select a provider to view its keys'
          }
          action={
            selectedProvider && (
              <Button variant="primary" size="sm" iconLeft={<Plus className="w-3 h-3" />} onClick={() => setAddKeyOpen(true)}>
                Add key
              </Button>
            )
          }
        >
          {!selectedProvider ? (
            <EmptyState
              icon={<Eye className="w-5 h-5" />}
              title="No provider selected"
              description="Click a provider on the left to see its keys."
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
              title="No keys"
              description="Add a key to start using this provider."
              action={
                <Button variant="primary" size="sm" iconLeft={<Plus className="w-3 h-3" />} onClick={() => setAddKeyOpen(true)}>
                  Add first key
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
                    if (confirm(`Delete key ${k.alias}?`)) deleteMut.mutate(k.id);
                  }}
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
}: {
  provider: LlmProvider;
  selected: boolean;
  onClick: () => void;
  onTest: () => void;
}) {
  const statusKind =
    p.health_status === 'ok' ? 'bull' :
    p.health_status === 'slow' ? 'warning' :
    p.health_status === 'failing' ? 'bear' : 'muted';
  return (
    <div
      onClick={onClick}
      className={
        'rounded-md border p-2.5 cursor-pointer transition-colors ' +
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
          Test
        </Button>
      </div>
    </div>
  );
}

function KeyRow({
  keyRow: k,
  onTest,
  onDelete,
}: {
  keyRow: LlmProviderKey;
  onTest: () => void;
  onDelete: () => void;
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
          <CheckCircle2 className="w-2.5 h-2.5" /> in keyring
        </Pill>
      ) : (
        <Pill kind="bear">
          <XCircle className="w-2.5 h-2.5" /> no secret
        </Pill>
      )}
      <Button variant="ghost" size="xs" iconLeft={<TestTube2 className="w-3 h-3" />} onClick={onTest}>
        Test
      </Button>
      <Button variant="ghost" size="xs" iconLeft={<Trash2 className="w-3 h-3" />} onClick={onDelete}>
        Delete
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
      toast.success('Key added');
      onAdded();
    },
    onError: (e: Error) => toast.error('Add key failed', e.message),
  });

  return (
    <Modal
      open
      onClose={onClose}
      title={`Add key for ${providerId}`}
      size="md"
      footer={
        <>
          <Button variant="ghost" size="sm" onClick={onClose}>Cancel</Button>
          <Button
            variant="primary"
            size="sm"
            loading={upsertMut.isPending}
            disabled={!alias.trim()}
            onClick={() => upsertMut.mutate()}
          >
            Add
          </Button>
        </>
      }
    >
      <div className="space-y-3">
        <Field label="Alias (e.g. prod-1, fallback)">
          <Input
            placeholder="prod-1"
            value={alias}
            onChange={(e) => setAlias(e.target.value)}
          />
        </Field>
        <Field label="Priority (lower = tried first)">
          <Input
            type="number"
            min={1}
            max={100}
            value={priority}
            onChange={(e) => setPriority(Math.max(1, Number(e.target.value) || 1))}
          />
        </Field>
        <Field label="API key / secret (stored in OS keyring only)">
          <div className="flex gap-1">
            <Input
              type={showSecret ? 'text' : 'password'}
              placeholder="sk-…"
              value={secret}
              onChange={(e) => setSecret(e.target.value)}
              className="font-mono"
            />
            <Button variant="ghost" size="sm" iconLeft={showSecret ? <EyeOff className="w-3 h-3" /> : <Eye className="w-3 h-3" />} onClick={() => setShowSecret((s) => !s)}>
              {showSecret ? 'Hide' : 'Show'}
            </Button>
          </div>
        </Field>
        <div className="text-[11px] text-muted pt-2 border-t border-border">
          Secret is written to the OS keyring under{' '}
          <code className="font-mono text-fg">polyrocket/llm/{providerId}/&lt;alias&gt;</code> and never persisted in SQLite or any plain file.
        </div>
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
