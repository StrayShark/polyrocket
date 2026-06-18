import { useState } from 'react';
import { useQuery, useQueryClient, useMutation } from '@tanstack/react-query';
import { Plus, Wallet as WalletIcon, RefreshCw, Copy as CopyIcon, FolderSearch } from 'lucide-react';
import { listWallets, addWallet, pickFile } from '@/ipc';
import { readFileText } from '@/lib/env-file';
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
import { extractAddressFromJson } from '@/lib/wallet-file';
import { useT } from '@/lib/i18n';
import type { Wallet, WalletType } from '@/types/wallet';
import { POLYGON_MAINNET } from '@/types/wallet';

/**
 * `/wallets` 路由 —— wallet 元数据管理。
 *
 * **数据流**：
 *   1. mount `listWallets()`
 *   2. 表格渲染（address / label / chain_id / wallet_type / last_synced）
 *   3. 「Add Wallet」按钮打开 Modal（手动 paste / file picker）
 *
 * **Add 流程**：
 *   1. 用户输入 address + (label) + (chain_id 默认 137) + (type 默认 eoa)
 *   2. v0.57d+ 提供 file picker（`pickFile` + `extractAddressFromJson` 解析）
 *   3. 提交 → `addWallet` mutation → 失效 query
 *
 * **私钥永远不**走这条路径。私钥走 `polyrocket_wallet_set_pk` IPC（`/settings`）。
 */
export function Wallets() {
  const { t } = useT();
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
            <h2 className="text-[13px] font-semibold text-fg">{t('wallets.title')}</h2>
            <p className="text-[11px] text-muted mt-0.5">
              {t('wallets.subtitle')}
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
              {t('wallets.refresh')}
            </Button>
            <Button variant="primary" size="sm" iconLeft={<Plus className="w-3 h-3" />} onClick={() => setAddOpen(true)}>
              {t('wallets.add')}
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
          title={t('wallets.empty')}
          description={t('wallets.empty_desc')}
          action={
            <Button variant="primary" size="sm" iconLeft={<Plus className="w-3 h-3" />} onClick={() => setAddOpen(true)}>
              {t('wallets.add_first')}
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
  const { t } = useT();
  return (
    <Card>
      <div className="space-y-2">
        <div className="flex items-start justify-between gap-2">
          <div className="flex items-center gap-2">
            <Pill kind="muted">{w.wallet_type}</Pill>
            <Pill kind="accent">chain {w.chain_id}</Pill>
          </div>
          <span className="text-[10px] text-muted" title={fmtDate(w.created_at)}>
            {t('wallets.card.created', { rel: fmtRelativeTime(w.created_at) })}
          </span>
        </div>
        <div>
          <div className="text-[13px] font-medium text-fg">{w.label || t('wallets.card.no_label')}</div>
          <div className="flex items-center gap-1.5 mt-1">
            <code className="text-[11px] font-mono text-muted">{fmtAddress(w.address)}</code>
            <button
              onClick={() => {
                navigator.clipboard.writeText(w.address);
                toast.success(t('wallets.card.address_copied'));
              }}
              className="text-muted hover:text-fg"
              title={t('wallets.card.copy')}
            >
              <CopyIcon className="w-3 h-3" />
            </button>
          </div>
        </div>
        {w.last_synced_at && (
          <div className="text-[10px] text-muted pt-1 border-t border-border">
            {t('wallets.card.last_synced', { rel: fmtRelativeTime(w.last_synced_at) })}
          </div>
        )}
      </div>
    </Card>
  );
}

function AddWalletModal({ onClose, onAdded }: { onClose: () => void; onAdded: () => void }) {
  const { t } = useT();
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
      toast.success(t('wallets.add.toast.added', { addr: fmtAddress(w.address) }));
      onAdded();
    },
    onError: (e: Error) => toast.error(t('wallets.add.toast.failed'), e.message),
  });

  const canSubmit = address.trim().startsWith('0x') && address.trim().length === 42;

  return (
    <Modal
      open
      onClose={onClose}
      title={t('wallets.add.title')}
      size="md"
      footer={
        <>
          <Button variant="ghost" size="sm" onClick={onClose}>{t('wallets.add.cancel')}</Button>
          <Button
            variant="primary"
            size="sm"
            loading={mut.isPending}
            disabled={!canSubmit}
            onClick={() => mut.mutate()}
          >
            {t('wallets.add.add')}
          </Button>
        </>
      }
    >
      <div className="space-y-3">
        <Field label={t('wallets.add.address')}>
          <div className="flex gap-1">
            <Input
              placeholder={t('wallets.add.address_placeholder')}
              value={address}
              onChange={(e) => setAddress(e.target.value)}
              invalid={address.length > 0 && !canSubmit}
              className="font-mono"
            />
            {/* v0.57d — native file picker for
                importing a wallet address from a
                JSON file (the typical export
                format from MetaMask / Rabby /
                frame). The file must contain
                {"address": "0x..."} somewhere. */}
            <Button
              variant="ghost"
              size="sm"
              iconLeft={<FolderSearch className="w-3 h-3" />}
              data-testid="wallet-import-from-file"
              onClick={async () => {
                try {
                  const picked = await pickFile(
                    [
                      { name: 'Wallet JSON', extensions: ['json'] },
                    ],
                    false,
                  );
                  if (typeof picked === 'string') {
                    const content = await readFileText(picked);
                    const extracted = extractAddressFromJson(content);
                    if (extracted) {
                      setAddress(extracted);
                      toast.success(t('wallets.add.imported_from_file'));
                    } else {
                      toast.error(t('wallets.add.import_no_address'));
                    }
                  }
                } catch (err) {
                  toast.error(String(err));
                }
              }}
            >
              {t('wallets.add.import_file')}
            </Button>
          </div>
        </Field>
        <Field label={t('wallets.add.label')}>
          <Input
            placeholder={t('wallets.add.label_placeholder')}
            value={label}
            onChange={(e) => setLabel(e.target.value)}
          />
        </Field>
        <div className="grid grid-cols-2 gap-3">
          <Field label={t('wallets.add.chain_id')}>
            <select
              value={chainId}
              onChange={(e) => setChainId(Number(e.target.value))}
              className="w-full h-8 px-2 rounded-md text-[13px] bg-surface text-fg border border-border"
            >
              <option value={137}>{t('wallets.add.chain_polygon')}</option>
              <option value={80002}>{t('wallets.add.chain_amoy')}</option>
            </select>
          </Field>
          <Field label={t('wallets.add.type')}>
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
                {t('wallets.add.type_eoa')}
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
                {t('wallets.add.type_smart')}
              </button>
            </div>
          </Field>
        </div>
        <div
          className="text-[11px] text-muted pt-2 border-t border-border"
          dangerouslySetInnerHTML={{
            __html: t('wallets.add.notice', {
              path: 'polyrocket/wallet/&lt;label&gt;',
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
