import { useState } from 'react';
import { useQuery, useQueryClient, useMutation } from '@tanstack/react-query';
import { Plus, Copy as CopyIcon, RefreshCw, Eye, EyeOff, ExternalLink } from 'lucide-react';
import { listCopyTargets, addCopyTarget, recentCopyEvents } from '@/ipc';
import { Card } from '@/components/base/Card';
import { Pill } from '@/components/base/Pill';
import { Button } from '@/components/base/Button';
import { Input } from '@/components/base/Input';
import { Modal } from '@/components/feedback/Modal';
import { Skeleton } from '@/components/feedback/Skeleton';
import { ErrorState } from '@/components/feedback/ErrorState';
import { EmptyState } from '@/components/feedback/EmptyState';
import { toast } from '@/stores/toast-store';
import { fmtAddress, fmtRelativeTime, fmtUsdc, fmtDateTime } from '@/lib/format';
import type { CopyTarget, CopyEvent } from '@/types/shared';

export function Copy() {
  const [addOpen, setAddOpen] = useState(false);
  const queryClient = useQueryClient();

  const { data: targets, isLoading, error, refetch, isRefetching } = useQuery({
    queryKey: ['copy-targets'],
    queryFn: () => listCopyTargets(),
    staleTime: 60_000,
  });

  const { data: events } = useQuery({
    queryKey: ['copy-events', undefined, 50],
    queryFn: () => recentCopyEvents(undefined, 50),
    staleTime: 30_000,
  });

  return (
    <div className="space-y-4">
      <Card padding="sm">
        <div className="flex items-center justify-between">
          <div>
            <h2 className="text-[13px] font-semibold text-fg">Copy Trading</h2>
            <p className="text-[11px] text-muted mt-0.5">
              Watch whale addresses for on-chain trades. Mirror policy: only fires when our model edge &gt; min_edge.
            </p>
          </div>
          <div className="flex items-center gap-2">
            <Button
              variant="ghost"
              size="sm"
              iconLeft={<RefreshCw className={'w-3 h-3 ' + (isRefetching ? 'animate-spin' : '')} />}
              onClick={() => refetch()}
              disabled={isRefetching}
            >
              Refresh
            </Button>
            <Button variant="primary" size="sm" iconLeft={<Plus className="w-3 h-3" />} onClick={() => setAddOpen(true)}>
              Add target
            </Button>
          </div>
        </div>
      </Card>

      {error ? (
        <ErrorState message={String(error)} onRetry={() => refetch()} />
      ) : isLoading ? (
        <div className="space-y-2">
          {Array.from({ length: 3 }).map((_, i) => (
            <Skeleton key={i} className="h-16" />
          ))}
        </div>
      ) : targets && targets.length === 0 ? (
        <EmptyState
          icon={<CopyIcon className="w-5 h-5" />}
          title="No copy targets"
          description="Add a whale wallet to start mirroring their trades."
          action={
            <Button variant="primary" size="sm" iconLeft={<Plus className="w-3 h-3" />} onClick={() => setAddOpen(true)}>
              Add your first target
            </Button>
          }
        />
      ) : (
        <div className="space-y-2">
          {targets?.map((t) => (
            <TargetRow
              key={t.id}
              target={t}
              events={events?.filter((e) => e.target_id === t.id) ?? []}
            />
          ))}
        </div>
      )}

      {addOpen && <AddTargetModal onClose={() => setAddOpen(false)} onAdded={() => {
        queryClient.invalidateQueries({ queryKey: ['copy-targets'] });
        setAddOpen(false);
      }} />}
    </div>
  );
}

function TargetRow({ target: t, events }: { target: CopyTarget; events: CopyEvent[] }) {
  return (
    <Card padding="sm">
      <div className="flex items-start gap-3">
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2">
            {t.enabled ? (
              <Pill kind="bull">
                <Eye className="w-2.5 h-2.5" /> watching
              </Pill>
            ) : (
              <Pill kind="muted">
                <EyeOff className="w-2.5 h-2.5" /> paused
              </Pill>
            )}
            {t.allocation_cap && (
              <Pill kind="accent">cap ${fmtUsdc(t.allocation_cap)}</Pill>
            )}
            <Pill kind="muted">min edge {(t.min_edge * 100).toFixed(0)}%</Pill>
            <span className="text-[10px] text-muted">
              created {fmtRelativeTime(t.created_at)}
            </span>
          </div>
          <div className="mt-1.5 flex items-center gap-1.5">
            <span className="text-[13px] font-medium text-fg">{t.label || '(no label)'}</span>
            <code className="text-[11px] font-mono text-muted">{fmtAddress(t.address)}</code>
            <button
              onClick={() => {
                navigator.clipboard.writeText(t.address);
                toast.success('Address copied');
              }}
              className="text-muted hover:text-fg"
              title="Copy full address"
            >
              <CopyIcon className="w-3 h-3" />
            </button>
          </div>
          {events.length > 0 && (
            <div className="mt-2 space-y-1">
              <div className="text-[10px] text-muted">recent events</div>
              {events.slice(0, 3).map((e) => (
                <div
                  key={e.id}
                  className="flex items-center gap-2 text-[11px] text-fg-secondary"
                >
                  <Pill kind={e.side === 'YES' ? 'bull' : 'bear'}>{e.side}</Pill>
                  <span className="font-mono">${fmtUsdc(e.size)} @ {e.price.toFixed(3)}</span>
                  <a
                    href={`https://polygonscan.com/tx/${e.tx_hash}`}
                    target="_blank"
                    rel="noopener noreferrer"
                    className="text-muted hover:text-accent inline-flex"
                  >
                    <ExternalLink className="w-3 h-3" />
                  </a>
                  <span className="text-muted" title={fmtDateTime(e.detected_at)}>
                    {fmtRelativeTime(e.detected_at)}
                  </span>
                </div>
              ))}
            </div>
          )}
        </div>
      </div>
    </Card>
  );
}

function AddTargetModal({ onClose, onAdded }: { onClose: () => void; onAdded: () => void }) {
  const [address, setAddress] = useState('');
  const [label, setLabel] = useState('');
  const [minEdgePct, setMinEdgePct] = useState(5);
  const [allocationCap, setAllocationCap] = useState('');

  const mut = useMutation({
    mutationFn: () => addCopyTarget({
      address: address.trim(),
      label: label.trim() || undefined,
      allocation_cap: allocationCap.trim() || undefined,
      min_edge: minEdgePct / 100,
    }),
    onSuccess: (t) => {
      toast.success(`Added target ${fmtAddress(t.address)}`);
      onAdded();
    },
    onError: (e: Error) => toast.error('Failed to add target', e.message),
  });

  const canSubmit = address.trim().startsWith('0x') && address.trim().length === 42;

  return (
    <Modal
      open
      onClose={onClose}
      title="Add copy target"
      size="md"
      footer={
        <>
          <Button variant="ghost" size="sm" onClick={onClose}>Cancel</Button>
          <Button
            variant="primary"
            size="sm"
            loading={mut.isPending}
            disabled={!canSubmit}
            onClick={() => mut.mutate()}
          >
            Add
          </Button>
        </>
      }
    >
      <div className="space-y-3">
        <Field label="Whale address (0x…40 hex)">
          <Input
            placeholder="0x…"
            value={address}
            onChange={(e) => setAddress(e.target.value)}
            invalid={address.length > 0 && !canSubmit}
            className="font-mono"
          />
        </Field>
        <Field label="Label (optional)">
          <Input
            placeholder="whale-1, fund-A, …"
            value={label}
            onChange={(e) => setLabel(e.target.value)}
          />
        </Field>
        <div className="grid grid-cols-2 gap-3">
          <Field label="Min edge %">
            <Input
              type="number"
              min={0}
              max={50}
              step={1}
              value={minEdgePct}
              onChange={(e) => setMinEdgePct(Math.max(0, Math.min(50, Number(e.target.value) || 0)))}
            />
          </Field>
          <Field label="Allocation cap (USDC)">
            <Input
              type="number"
              min={0}
              step={10}
              placeholder="no cap"
              value={allocationCap}
              onChange={(e) => setAllocationCap(e.target.value)}
            />
          </Field>
        </div>
        <div className="text-[11px] text-muted pt-2 border-t border-border">
          Mirror only fires when our model edge &gt; min_edge AND the whale's side
          agrees with our direction. Set allocation_cap to limit per-fill size.
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
