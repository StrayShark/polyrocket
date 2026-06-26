// polyrocket — Wallets（v0.68f 密度）。
//
// /wallets 是 wallet 元数据管理器。两个关注点：
//
//   1. **列出已注册的 wallet** —— 名称、地址、
//      chain_id、类型（eoa / smart）、last_synced。
//      点击复制图标 → 地址复制到剪贴板。
//   2. **添加 wallet** —— 含 3 个字段的 modal：
//      address（0x... 40 hex）、label（可选）、
//      chain_id（默认 137 Polygon 或 80002 Amoy）。
//
// **文件选择器（v0.57d+）**：modal 上的「import from file」
// 按钮读取 JSON 文件（典型的 MetaMask/Rabby 导出），
// 通过 `extractAddressFromJson` 提取 0x 地址，
// 并预填 address 字段。免去用户输入 42 个
// hex 字符的麻烦。
//
// **私钥位于别处**：此页面不处理私钥。私钥通过
// /settings 中的 `polyrocket_wallet_set_pk` IPC 提交，
// 存储到 OS keyring 中的 `polyrocket/wallet/<label>`
// 下。

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
 *   1. 挂载时调用 `listWallets()`
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
            <h2 className="text-title-sm font-semibold text-fg">{t('wallets.title')}</h2>
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
          <div className="text-title-sm font-semibold text-fg">{w.label || t('wallets.card.no_label')}</div>
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
            {/* v0.57d — 原生文件选择器，用于从 JSON 文件
                导入 wallet 地址（MetaMask / Rabby /
                frame 的典型导出格式）。文件必须在
                某处包含 {"address": "0x..."}。 */}
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
              className="w-full h-8 px-2 rounded-md text-body-sm bg-surface text-fg border border-border"
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
