import { useQuery, useQueryClient, useMutation } from '@tanstack/react-query';
import { useMemo, useState } from 'react';
import { Link } from 'react-router-dom';
import { Zap, RefreshCw, TrendingUp, TrendingDown } from 'lucide-react';
import { listActiveSignals, recomputeSignals } from '@/ipc';
import { DataTable, type Column } from '@/components/data/DataTable';
import { Card } from '@/components/base/Card';
import { Pill } from '@/components/base/Pill';
import { Button } from '@/components/base/Button';
import { Input } from '@/components/base/Input';
import { useT } from '@/lib/i18n';
import { KpiCard } from '@/components/data/KpiCard';
import { Skeleton } from '@/components/feedback/Skeleton';
import { ErrorState } from '@/components/feedback/ErrorState';
import { EmptyState } from '@/components/feedback/EmptyState';
import { toast } from '@/stores/toast-store';
import { fmtEdge, fmtConfidence, fmtDate, fmtRelativeTime } from '@/lib/format';
import type { Signal } from '@/types/signal';

export function Signals() {
  const { t } = useT();
  const [minEdgePct, setMinEdgePct] = useState(5);
  const [side, setSide] = useState<'all' | 'yes' | 'no'>('all');

  const { data, isLoading, error, refetch, isRefetching } = useQuery({
    queryKey: ['signals', 'active'],
    queryFn: () => listActiveSignals({ limit: 200 }),
    staleTime: 30_000,
  });

  const queryClient = useQueryClient();
  const recomputeMut = useMutation({
    mutationFn: () => recomputeSignals(),
    onSuccess: (n) => {
      toast.success(`Recompute queued`, n > 0 ? `${n} signals updated` : 'no new signals');
      queryClient.invalidateQueries({ queryKey: ['signals'] });
    },
    onError: (e: Error) => toast.error('Recompute failed', e.message),
  });

  const filtered = useMemo(() => {
    if (!data) return [];
    return data.filter((s) => {
      if (Math.abs(s.edge) * 100 < minEdgePct) return false;
      if (side === 'yes' && s.edge <= 0) return false;
      if (side === 'no' && s.edge >= 0) return false;
      return true;
    });
  }, [data, minEdgePct, side]);

  const summary = useMemo(() => {
    const all = data ?? [];
    const bullish = all.filter((s) => s.edge > 0);
    const bearish = all.filter((s) => s.edge < 0);
    const avgAbs = all.length === 0
      ? 0
      : all.reduce((acc, s) => acc + Math.abs(s.edge), 0) / all.length;
    return {
      total: all.length,
      bullish: bullish.length,
      bearish: bearish.length,
      avgAbsEdge: avgAbs,
    };
  }, [data]);

  const columns: Column<Signal>[] = [
    {
      key: 'question',
      header: 'Market',
      cell: (s) => (
        <Link
          to={s.market_id ? `/markets/${s.market_id}` : '/markets'}
          className="text-fg hover:text-accent line-clamp-2 max-w-[480px]"
        >
          {s.market_question ?? s.market_id}
        </Link>
      ),
      sortable: true,
      sortValue: (s) => s.market_question ?? s.market_id,
    },
    {
      key: 'side',
      header: 'Side',
      width: '80px',
      cell: (s) =>
        s.edge > 0 ? (
          <Pill kind="bull">
            <TrendingUp className="w-2.5 h-2.5" /> YES
          </Pill>
        ) : (
          <Pill kind="bear">
            <TrendingDown className="w-2.5 h-2.5" /> NO
          </Pill>
        ),
      sortable: true,
      sortValue: (s) => s.edge,
    },
    {
      key: 'edge',
      header: 'Edge',
      align: 'right',
      width: '90px',
      cell: (s) => (
        <span
          className={'font-mono font-semibold ' + (s.edge > 0 ? 'text-bull' : 'text-bear')}
        >
          {fmtEdge(s.edge)}
        </span>
      ),
      sortable: true,
      sortValue: (s) => s.edge,
    },
    {
      key: 'confidence',
      header: 'Conf',
      align: 'right',
      width: '80px',
      cell: (s) => <span className="font-mono">{fmtConfidence(s.confidence)}</span>,
      sortable: true,
      sortValue: (s) => s.confidence,
    },
    {
      key: 'model_version',
      header: 'Model',
      width: '80px',
      cell: (s) => <Pill kind="muted">{s.model_version}</Pill>,
    },
    {
      key: 'horizon_hours',
      header: 'Horizon',
      align: 'right',
      width: '80px',
      cell: (s) => <span className="font-mono">{s.horizon_hours}h</span>,
      sortable: true,
      sortValue: (s) => s.horizon_hours,
    },
    {
      key: 'computed_at',
      header: 'Computed',
      width: '120px',
      cell: (s) => <span title={fmtDate(s.computed_at)}>{fmtRelativeTime(s.computed_at)}</span>,
      sortable: true,
      sortValue: (s) => s.computed_at,
    },
  ];

  return (
    <div className="space-y-4">
      {/* KPIs */}
      <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
        <KpiCard
          label={t('signals.title')}
          value={summary.total.toString()}
          icon={Zap}
          hint={`min edge ${minEdgePct}%`}
        />
        <KpiCard
          label="Bullish"
          value={summary.bullish.toString()}
          delta={summary.total > 0 ? { text: `${Math.round((summary.bullish / summary.total) * 100)}%`, positive: true } : null}
        />
        <KpiCard
          label="Bearish"
          value={summary.bearish.toString()}
          delta={summary.total > 0 ? { text: `${Math.round((summary.bearish / summary.total) * 100)}%`, positive: false } : null}
        />
        <KpiCard
          label="Avg |edge|"
          value={fmtEdge(summary.avgAbsEdge)}
        />
      </div>

      {/* Toolbar */}
      <Card padding="sm">
        <div className="flex items-center gap-3 flex-wrap">
          <label className="text-[11px] text-muted flex items-center gap-2">
            min |edge|
            <Input
              type="number"
              min={1}
              max={50}
              step={1}
              value={minEdgePct}
              onChange={(e) => setMinEdgePct(Math.max(1, Math.min(50, Number(e.target.value) || 0)))}
              className="w-16 h-7"
            />
            %
          </label>
          <div className="flex items-center gap-1">
            {(['all', 'yes', 'no'] as const).map((s) => (
              <button
                key={s}
                onClick={() => setSide(s)}
                className={
                  'h-7 px-2.5 rounded text-[11px] font-medium border transition-colors ' +
                  (side === s
                    ? 'bg-accent/15 text-accent border-accent/30'
                    : 'bg-surface-2 text-muted border-border hover:text-fg')
                }
              >
                {s}
              </button>
            ))}
          </div>
          <div className="flex-1" />
          <Button
            variant="ghost"
            size="sm"
            iconLeft={<RefreshCw className={'w-3 h-3 ' + (isRefetching ? 'animate-spin' : '')} />}
            onClick={() => refetch()}
            disabled={isRefetching}
          >
            Refresh
          </Button>
          <Button
            variant="primary"
            size="sm"
            loading={recomputeMut.isPending}
            onClick={() => recomputeMut.mutate()}
          >
            Recompute  // v0.13a — keep default; future use `t('signals.recompute')`
          </Button>
        </div>
      </Card>

      {/* Table */}
      {error ? (
        <ErrorState message={String(error)} onRetry={() => refetch()} />
      ) : isLoading ? (
        <Card>
          <div className="space-y-2">
            {Array.from({ length: 6 }).map((_, i) => (
              <Skeleton key={i} className="h-7" />
            ))}
          </div>
        </Card>
      ) : filtered.length === 0 ? (
        <EmptyState
          icon={<Zap className="w-5 h-5" />}
          title={t('status.empty')}
          description={
            summary.total === 0
              ? t('signals.empty')
              : `No signals match |edge| ≥ ${minEdgePct}%. Try lowering the threshold.`
          }
        />
      ) : (
        <DataTable
          rows={filtered}
          columns={columns}
          rowKey={(s) => String(s.id)}
          pageSize={20}
        />
      )}
    </div>
  );
}
