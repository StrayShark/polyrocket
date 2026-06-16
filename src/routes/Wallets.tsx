import { useState } from 'react';
import { useQuery, useQueryClient, useMutation } from '@tanstack/react-query';
import { Plus, Wallet as WalletIcon, RefreshCw, Copy as CopyIcon } from 'lucide-react';
import { listWallets, addWallet } from '@/ipc';
import { Card } from '@/components/base/Card';
import { Pill } from '@/components/base/Pill';
import { Button } from '@/components/base/Button';
import { Input } from '@/components/base/Input';
import { Modal } from '@/components/feedback/Modal';
import { Skeleton } from '@/components/feedback/Skeleton';
import { ErrorState } from '@/components/feedback/ErrorState';
import { EmptyState } from '@/components/feedback/EmptyState';
import { toast } from '@/stores/toast-store';
import { fmtAddress, fmtRelativeTime, fmtDate } from '@/lib/format';
import type { Wallet, WalletType } from '@/types/wallet';
import { POLYGON_MAINNET } from '@/types/wallet';

export function Wallets() {
  const [addOpen, setAddOpen] = useState(false);
  const queryClient = useQueryClient();
  const { data, isLoading, error, refetch, isRefetching } = useQuery({
    queryKey: ['wallets'],
    queryFn: () => listWallets(),
    staleTime: 60_000,
  });

  return (
    <div className="space-y-4">
      <Card padding="sm">
        <div className="flex items-center justify-between">
          <div>
            <h2 className="text-[13px] font-semibold text-fg">Wallets</h2>
            <p className="text-[11px] text-muted mt-0.5">
              Register wallet addresses for trading. Private keys are stored in the OS keyring — never in SQLite.
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
              Add wallet
            </Button>
          </div>
        </div>
      </Card>

      {error ? (
        <ErrorState message={String(error)} onRetry={() => refetch()} />
      ) : isLoading ? (
        <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
          {Array.from({ length: 4 }).map((_, i) => (
            <Skeleton key={i} className="h-28" />
          ))}
        </div>
      ) : data && data.length === 0 ? (
        <EmptyState
          icon={<WalletIcon className="w-5 h-5" />}
          title="No wallets yet"
          description="Add a wallet to start trading. We support Polygon mainnet (chain 137) and Amoy testnet (80002)."
          action={
            <Button variant="primary" size="sm" iconLeft={<Plus className="w-3 h-3" />} onClick={() => setAddOpen(true)}>
              Add your first wallet
            </Button>
          }
        />
      ) : (
        <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
          {data?.map((w) => (
            <WalletCard key={w.id} wallet={w} />
          ))}
        </div>
      )}

      {addOpen && <AddWalletModal onClose={() => setAddOpen(false)} onAdded={() => {
        queryClient.invalidateQueries({ queryKey: ['wallets'] });
        setAddOpen(false);
      }} />}
    </div>
  );
}

function WalletCard({ wallet: w }: { wallet: Wallet }) {
  return (
    <Card>
      <div className="space-y-2">
        <div className="flex items-start justify-between gap-2">
          <div className="flex items-center gap-2">
            <Pill kind="muted">{w.wallet_type}</Pill>
            <Pill kind="accent">chain {w.chain_id}</Pill>
          </div>
          <span className="text-[10px] text-muted" title={fmtDate(w.created_at)}>
            created {fmtRelativeTime(w.created_at)}
          </span>
        </div>
        <div>
          <div className="text-[13px] font-medium text-fg">{w.label || '(no label)'}</div>
          <div className="flex items-center gap-1.5 mt-1">
            <code className="text-[11px] font-mono text-muted">{fmtAddress(w.address)}</code>
            <button
              onClick={() => {
                navigator.clipboard.writeText(w.address);
                toast.success('Address copied');
              }}
              className="text-muted hover:text-fg"
              title="Copy full address"
            >
              <CopyIcon className="w-3 h-3" />
            </button>
          </div>
        </div>
        {w.last_synced_at && (
          <div className="text-[10px] text-muted pt-1 border-t border-border">
            last synced {fmtRelativeTime(w.last_synced_at)}
          </div>
        )}
      </div>
    </Card>
  );
}

function AddWalletModal({ onClose, onAdded }: { onClose: () => void; onAdded: () => void }) {
  const [address, setAddress] = useState('');
  const [label, setLabel] = useState('');
  const [chainId, setChainId] = useState<number>(POLYGON_MAINNET);
  const [walletType, setWalletType] = useState<WalletType>('eoa');

  const mut = useMutation({
    mutationFn: () => addWallet({
      address: address.trim(),
      label: label.trim() || undefined,
      chain_id: chainId,
      wallet_type: walletType,
    }),
    onSuccess: (w) => {
      toast.success(`Added wallet ${fmtAddress(w.address)}`);
      onAdded();
    },
    onError: (e: Error) => toast.error('Failed to add wallet', e.message),
  });

  const canSubmit = address.trim().startsWith('0x') && address.trim().length === 42;

  return (
    <Modal
      open
      onClose={onClose}
      title="Add wallet"
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
        <Field label="Address (0x…40 hex chars)">
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
            placeholder="primary, trade-1, cold, …"
            value={label}
            onChange={(e) => setLabel(e.target.value)}
          />
        </Field>
        <div className="grid grid-cols-2 gap-3">
          <Field label="Chain ID">
            <select
              value={chainId}
              onChange={(e) => setChainId(Number(e.target.value))}
              className="w-full h-8 px-2 rounded-md text-[13px] bg-surface text-fg border border-border"
            >
              <option value={137}>137 — Polygon mainnet</option>
              <option value={80002}>80002 — Polygon Amoy testnet</option>
            </select>
          </Field>
          <Field label="Type">
            <div className="flex items-center gap-2 h-8">
              <button
                onClick={() => setWalletType('eoa')}
                className={
                  'h-7 px-3 rounded text-[11px] font-medium border ' +
                  (walletType === 'eoa'
                    ? 'bg-accent/15 text-accent border-accent/30'
                    : 'bg-surface-2 text-muted border-border hover:text-fg')
                }
              >
                EOA
              </button>
              <button
                onClick={() => setWalletType('smart')}
                className={
                  'h-7 px-3 rounded text-[11px] font-medium border ' +
                  (walletType === 'smart'
                    ? 'bg-accent/15 text-accent border-accent/30'
                    : 'bg-surface-2 text-muted border-border hover:text-fg')
                }
              >
                Smart
              </button>
            </div>
          </Field>
        </div>
        <div className="text-[11px] text-muted pt-2 border-t border-border">
          Note: private keys are stored in the OS keyring under alias{' '}
          <code className="font-mono text-fg">polyrocket/wallet/&lt;label&gt;</code> —
          never in SQLite or any plain file.
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
