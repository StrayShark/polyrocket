// v0.78 — /bankroll route (L3).
//
// Pulls active signals + computes a bankroll allocation preview.
// Shows BankrollCard + AllocationTable + config sliders + apply
// button. The "apply" step is v0.78e (writes to bets table).
//
// For v0.78c the apply button is a no-op (toast "coming soon") so
// the user can see the deterministic allocation before commit.

import { useState, useMemo } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { Wallet, Sparkles, AlertCircle } from 'lucide-react';
import {
  listActiveSignals,
  listWallets,
  computeAllocationPreview,
  applyAllocation,
  setBankrollConfig,
  type ComputeAllocationArgs,
} from '@/ipc';
import { Card } from '@/components/base/Card';
import { Button } from '@/components/base/Button';
import { Skeleton } from '@/components/feedback/Skeleton';
import { EmptyState } from '@/components/feedback/EmptyState';
import { ErrorState } from '@/components/feedback/ErrorState';
import { BankrollCard } from '@/components/feedback/BankrollCard';
import { AllocationTable } from '@/components/feedback/AllocationTable';
import { fmtUsdc } from '@/lib/format';
import { useT } from '@/lib/i18n';
import { toast } from '@/stores/toast-store';
import type { BankrollConfigDto, AllocationResult } from '@/types/bankroll';

const DEFAULT_CONFIG: BankrollConfigDto = {
  kelly_multiplier: 0.25,
  max_per_signal_pct: 0.10,
  reserve_pct: 0.20,
  min_edge_pct: 0.05,
  max_total_exposure_pct: 0.80,
  min_confidence: 0.60,
};

export function Bankroll() {
  const { t } = useT();
  const queryClient = useQueryClient();
  const [config, setConfig] = useState<BankrollConfigDto>(DEFAULT_CONFIG);
  const [bankrollInput, setBankrollInput] = useState('1000');

  const walletsQuery = useQuery({
    queryKey: ['wallets'],
    queryFn: () => listWallets(),
  });
  const activeWalletQuery = useQuery({
    queryKey: ['wallets'],
    queryFn: () => listWallets(),
  });
  const signalsQuery = useQuery({
    queryKey: ['signals-active'],
    queryFn: () => listActiveSignals({ limit: 50 }),
  });

  const bankrollUsdc = useMemo(() => {
    // v0.78c — use the input as bankroll. v0.78e reads from
    // bankroll_snapshot (which sums wallet balance minus open bets).
    return bankrollInput || '0';
  }, [bankrollInput]);

  const allocationQuery = useQuery({
    queryKey: ['bankroll-allocation', bankrollUsdc, config, signalsQuery.data],
    queryFn: () => {
      const args: ComputeAllocationArgs = {
        bankroll_usdc: bankrollUsdc,
        config,
        signals: signalsQuery.data ?? [],
        market_liquidity: null,
      };
      return computeAllocationPreview(args);
    },
    enabled: !!signalsQuery.data && parseFloat(bankrollUsdc) > 0,
  });

  // v0.78e — apply mutation writes to allocation_batches
  const applyMut = useMutation({
    mutationFn: (result: AllocationResult) => {
      if (!activeWallet) throw new Error('no active wallet');
      return applyAllocation(activeWallet.id ?? 'default', result, bankrollUsdc, config);
    },
    onSuccess: (batchId) => {
      toast.success('Allocation applied', `Batch ${batchId.slice(0, 8)}…`);
      // Also persist the config for this wallet
      if (activeWallet?.id) {
        setBankrollConfig(activeWallet.id, config).catch(() => {
          /* config persist is best-effort */
        });
      }
      queryClient.invalidateQueries({ queryKey: ['bankroll-batches'] });
    },
    onError: (e: Error) => {
      toast.error('Apply failed', e.message);
    },
  });

  const updateConfig = (key: keyof BankrollConfigDto, value: number) => {
    setConfig((c) => ({ ...c, [key]: value }));
  };

  const activeWallet = activeWalletQuery.data?.[0];
  const walletLabel = activeWallet?.label ?? '—';

  return (
    <div className="p-6 space-y-4" data-testid="bankroll-page">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-[18px] font-semibold text-fg">
            {t('bankroll.title', { default: 'Bankroll Allocation' })}
          </h1>
          <p className="text-[12px] text-muted mt-1">
            {t('bankroll.subtitle', {
              default:
                'AI-driven Kelly-based allocation across active signals.',
            })}
          </p>
        </div>
        <Button
          size="sm"
          variant="primary"
          iconLeft={<Sparkles className="w-3 h-3" />}
          onClick={() => allocationQuery.refetch()}
          loading={allocationQuery.isFetching}
          data-testid="bankroll-compute"
        >
          Recompute
        </Button>
      </div>

      <Card>
        <div className="space-y-3">
          <div className="flex items-center gap-3">
            <Wallet className="w-4 h-4 text-muted" />
            <label className="text-[11px] text-muted uppercase tracking-wide">
              Bankroll (USDC)
            </label>
            <input
              type="number"
              className="ml-auto w-32 h-7 px-2 rounded-md text-[12px] bg-surface text-fg border border-border font-mono"
              value={bankrollInput}
              onChange={(e) => setBankrollInput(e.target.value)}
              data-testid="bankroll-input"
            />
          </div>
          {walletsQuery.error && (
            <div className="flex items-center gap-2 text-[11px] text-bear">
              <AlertCircle className="w-3 h-3" />
              Failed to load wallets
            </div>
          )}
          {activeWalletQuery.data && (
            <BankrollCard
              availableUsdc={bankrollUsdc}
              reservedUsdc={allocationQuery.data?.reserved_usdc ?? '0'}
              allocatedUsdc={'0'}
              walletLabel={walletLabel}
            />
          )}
        </div>
      </Card>

      <Card title="Allocation Config">
        <ConfigSliders
          config={config}
          onChange={updateConfig}
        />
      </Card>

      <Card title="Allocation Preview">
        {signalsQuery.isLoading ? (
          <Skeleton className="h-32" />
        ) : signalsQuery.error ? (
          <ErrorState message={String(signalsQuery.error)} />
        ) : (signalsQuery.data ?? []).length === 0 ? (
          <EmptyState
            icon={<Sparkles className="w-5 h-5" />}
            title="No active signals"
            description="No signals pass the edge/confidence threshold."
          />
        ) : allocationQuery.isLoading ? (
          <Skeleton className="h-32" />
        ) : allocationQuery.data ? (
          <AllocationResultView
            result={allocationQuery.data}
            onApply={() => {
              if (allocationQuery.data) {
                applyMut.mutate(allocationQuery.data);
              }
            }}
            applying={applyMut.isPending}
          />
        ) : null}
      </Card>
    </div>
  );
}

function ConfigSliders({
  config,
  onChange,
}: {
  config: BankrollConfigDto;
  onChange: (key: keyof BankrollConfigDto, value: number) => void;
}) {
  const sliders: Array<{
    key: keyof BankrollConfigDto;
    label: string;
    min: number;
    max: number;
    step: number;
    format: (v: number) => string;
  }> = [
    {
      key: 'kelly_multiplier',
      label: 'Kelly multiplier',
      min: 0.05,
      max: 1.0,
      step: 0.05,
      format: (v) => `${(v * 100).toFixed(0)}% Kelly`,
    },
    {
      key: 'max_per_signal_pct',
      label: 'Max per signal',
      min: 0.01,
      max: 0.5,
      step: 0.01,
      format: (v) => `${(v * 100).toFixed(0)}% bankroll`,
    },
    {
      key: 'reserve_pct',
      label: 'Reserve',
      min: 0.0,
      max: 0.5,
      step: 0.05,
      format: (v) => `${(v * 100).toFixed(0)}% kept`,
    },
    {
      key: 'min_edge_pct',
      label: 'Min |edge|',
      min: 0.0,
      max: 0.3,
      step: 0.01,
      format: (v) => `${(v * 100).toFixed(0)}%`,
    },
    {
      key: 'max_total_exposure_pct',
      label: 'Max total exposure',
      min: 0.1,
      max: 1.0,
      step: 0.05,
      format: (v) => `${(v * 100).toFixed(0)}% of bankroll`,
    },
  ];

  return (
    <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
      {sliders.map((s) => (
        <div key={s.key} className="space-y-1" data-testid={`config-${s.key}`}>
          <div className="flex items-center justify-between text-[11px]">
            <span className="text-muted">{s.label}</span>
            <span className="font-mono text-fg">
              {s.format(config[s.key])}
            </span>
          </div>
          <input
            type="range"
            min={s.min}
            max={s.max}
            step={s.step}
            value={config[s.key]}
            onChange={(e) => onChange(s.key, Number(e.target.value))}
            className="w-full"
            data-testid={`config-slider-${s.key}`}
          />
        </div>
      ))}
    </div>
  );
}

function AllocationResultView({
  result,
  onApply,
  applying,
}: {
  result: AllocationResult;
  onApply: () => void;
  applying?: boolean;
}) {
  return (
    <div className="space-y-3">
      <div className="flex items-center justify-between text-[12px]">
        <div className="text-muted">
          Total allocated:{' '}
          <span
            className="text-fg font-mono"
            data-testid="alloc-total"
          >
            {fmtUsdc(parseFloat(result.total_allocated_usdc) || 0)}
          </span>{' '}
          ·{' '}
          <span className="text-muted">
            Reserved: {fmtUsdc(parseFloat(result.reserved_usdc) || 0)}
          </span>
        </div>
        <Button
          size="sm"
          variant="primary"
          onClick={onApply}
          disabled={result.per_market.length === 0 || applying}
          loading={applying}
          data-testid="alloc-apply"
        >
          {applying ? 'Applying…' : 'Apply allocation'}
        </Button>
      </div>
      {result.dropped_markets.length > 0 && (
        <div className="text-[11px] text-bear">
          Dropped (liquidity): {result.dropped_markets.join(', ')}
        </div>
      )}
      <AllocationTable items={result.per_market} />
    </div>
  );
}
