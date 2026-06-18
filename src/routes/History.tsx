import { useQuery } from '@tanstack/react-query';
import { useMemo, useState } from 'react';
import { Link } from 'react-router-dom';
import { History as HistoryIcon, RefreshCw, TrendingUp, TrendingDown, ExternalLink } from 'lucide-react';
import { listBets } from '@/ipc';
import { DataTable, type Column } from '@/components/data/DataTable';
import { Card } from '@/components/base/Card';
import { Pill } from '@/components/base/Pill';
import { Button } from '@/components/base/Button';
import { KpiCard } from '@/components/data/KpiCard';
import { Skeleton } from '@/components/feedback/Skeleton';
import { ErrorState } from '@/components/feedback/ErrorState';
import { EmptyState } from '@/components/feedback/EmptyState';
import { fmtUsdc, fmtDateTime, fmtRelativeTime } from '@/lib/format';
import { useT } from '@/lib/i18n';
import type { Bet, BetStatus } from '@/types/bet';

const STATUS_FILTERS: Array<BetStatus | 'all'> = ['all', 'open', 'won', 'lost', 'cancelled'];

export function History() {
  const { t } = useT();
  const [statusFilter, setStatusFilter] = useState<BetStatus | 'all'>('all');

  const { data, isLoading, error, refetch, isRefetching } = useQuery({
    queryKey: ['bets'],
    queryFn: () => listBets({ limit: 500 }),
    staleTime: 30_000,
  });

  const filtered = useMemo(() => {
    if (!data) return [];
    if (statusFilter === 'all') return data;
    return data.filter((b) => b.status === statusFilter);
  }, [data, statusFilter]);

  const summary = useMemo(() => {
    const all = data ?? [];
    const open = all.filter((b) => b.status === 'open');
    const won = all.filter((b) => b.status === 'won');
    const lost = all.filter((b) => b.status === 'lost');
    const totalPnl = [...won, ...lost].reduce((acc, b) => {
      const n = Number(b.pnl ?? 0);
      return acc + (Number.isFinite(n) ? n : 0);
    }, 0);
    const winRate = won.length + lost.length === 0
      ? 0
      : won.length / (won.length + lost.length);
    return {
      total: all.length,
      open: open.length,
      won: won.length,
      lost: lost.length,
      totalPnl,
      winRate,
    };
  }, [data]);

  const columns: Column<Bet>[] = [
    {
      key: 'market',
      header: 'Market',
      cell: (b) => (
        <Link
          to={`/markets/${b.market_id}`}
          className="text-fg hover:text-accent line-clamp-1 max-w-[400px]"
        >
          {b.market_id}
        </Link>
      ),
      sortable: true,
      sortValue: (b) => b.market_id,
    },
    {
      key: 'side',
      header: 'Side',
      width: '70px',
      cell: (b) =>
        b.side === 'YES' ? (
          <Pill kind="bull">
            <TrendingUp className="w-2.5 h-2.5" /> YES
          </Pill>
        ) : (
          <Pill kind="bear">
            <TrendingDown className="w-2.5 h-2.5" /> NO
          </Pill>
        ),
    },
    {
      key: 'mode',
      header: 'Mode',
      width: '80px',
      cell: (b) => <Pill kind="muted">{b.mode === 'A_jump' ? 'A' : 'B'}</Pill>,
    },
    {
      key: 'size',
      header: 'Size',
      align: 'right',
      width: '90px',
      cell: (b) => <span className="font-mono">${fmtUsdc(b.size)}</span>,
      sortable: true,
      sortValue: (b) => Number(b.size),
    },
    {
      key: 'price',
      header: 'Price',
      align: 'right',
      width: '80px',
      cell: (b) => <span className="font-mono">{b.price.toFixed(3)}</span>,
      sortable: true,
      sortValue: (b) => b.price,
    },
    {
      key: 'status',
      header: 'Status',
      width: '90px',
      cell: (b) => {
        const kind =
          b.status === 'won' ? 'bull' :
          b.status === 'lost' ? 'bear' :
          b.status === 'cancelled' ? 'muted' : 'accent';
        return <Pill kind={kind as 'bull' | 'bear' | 'muted' | 'accent'}>{b.status}</Pill>;
      },
    },
    {
      key: 'pnl',
      header: 'PnL',
      align: 'right',
      width: '100px',
      cell: (b) =>
        b.pnl == null ? (
          <span className="text-muted">—</span>
        ) : (
          <span
            className={
              'font-mono font-semibold ' +
              (Number(b.pnl) > 0 ? 'text-bull' : Number(b.pnl) < 0 ? 'text-bear' : 'text-muted')
            }
          >
            {Number(b.pnl) > 0 ? '+' : ''}${fmtUsdc(b.pnl)}
          </span>
        ),
      sortable: true,
      sortValue: (b) => Number(b.pnl ?? 0),
    },
    {
      key: 'placed_at',
      header: 'Placed',
      width: '120px',
      cell: (b) => (
        <span title={fmtDateTime(b.placed_at)}>{fmtRelativeTime(b.placed_at)}</span>
      ),
      sortable: true,
      sortValue: (b) => b.placed_at,
    },
    {
      key: 'tx',
      header: 'Tx',
      width: '40px',
      cell: (b) =>
        b.tx_hash ? (
          <a
            href={`https://polygonscan.com/tx/${b.tx_hash}`}
            target="_blank"
            rel="noopener noreferrer"
            className="text-muted hover:text-accent inline-flex"
            title={b.tx_hash}
          >
            <ExternalLink className="w-3.5 h-3.5" />
          </a>
        ) : (
          <span className="text-muted">—</span>
        ),
    },
    // v0.52c — Order type column. Shows
    // market / limit / stop_loss with a colored
    // pill. Post-only rows show a small badge.
    {
      key: 'order_type',
      header: 'Type',
      width: '110px',
      cell: (b) => (
        <div className="flex items-center gap-1">
          <Pill
            kind={
              b.order_type === 'limit'
                ? 'accent'
                : b.order_type === 'stop_loss'
                  ? 'warning'
                  : 'muted'
            }
          >
            {b.order_type === 'stop_loss'
              ? 'stop-loss'
              : b.order_type}
          </Pill>
          {b.post_only && (
            <span
              data-testid={`bet-post-only-${b.id}`}
              className="text-[9px] text-muted"
              title="post-only"
            >
              PO
            </span>
          )}
        </div>
      ),
      sortable: true,
      sortValue: (b) => b.order_type ?? 'market',
    },
    // v0.52c — Fill column. Shows fill_price vs
    // price when both are non-null; otherwise
    // shows '—' for pre-v0.51b rows. Slippage is
    // computed inline.
    {
      key: 'fill',
      header: 'Fill',
      width: '110px',
      align: 'right',
      cell: (b) => {
        if (b.fill_price == null) return <span className="text-muted">—</span>;
        const slip = b.fill_price - b.price;
        const slipPct = (slip / b.price) * 100;
        const cls =
          Math.abs(slipPct) < 0.01
            ? 'text-muted'
            : slipPct > 0
              ? 'text-bear'
              : 'text-bull';
        return (
          <span className={'font-mono ' + cls}>
            {b.fill_price.toFixed(4)}
            {b.partial && (
              <span
                data-testid={`bet-partial-${b.id}`}
                className="ml-1 text-[9px] text-warn"
                title="partial fill"
              >
                ⚠
              </span>
            )}
          </span>
        );
      },
    },
  ];

  return (
    <div className="space-y-4">
      <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
        <KpiCard
          label={t('history.kpi.total')}
          value={summary.total.toString()}
          icon={HistoryIcon}
        />
        <KpiCard
          label={t('history.kpi.open')}
          value={summary.open.toString()}
        />
        <KpiCard
          label={t('history.kpi.winrate')}
          value={`${Math.round(summary.winRate * 100)}%`}
          delta={summary.won + summary.lost > 0 ? { text: `${summary.won}W/${summary.lost}L`, positive: summary.winRate >= 0.5 } : null}
        />
        <KpiCard
          label={t('history.kpi.pnl')}
          value={`$${fmtUsdc(summary.totalPnl)}`}
          delta={{ text: t(summary.totalPnl >= 0 ? 'history.profit' : 'history.loss'), positive: summary.totalPnl >= 0 }}
        />
      </div>

      <Card padding="sm">
        <div className="flex items-center gap-2 flex-wrap">
          {STATUS_FILTERS.map((s) => (
            <button
              key={s}
              onClick={() => setStatusFilter(s)}
              className={
                'h-7 px-2.5 rounded text-[11px] font-medium border transition-colors ' +
                (statusFilter === s
                  ? 'bg-accent/15 text-accent border-accent/30'
                  : 'bg-surface-2 text-muted border-border hover:text-fg')
              }
            >
              {t(`history.filter.${s}`)}
            </button>
          ))}
          <div className="flex-1" />
          <Button
            variant="ghost"
            size="sm"
            iconLeft={<RefreshCw className={'w-3 h-3 ' + (isRefetching ? 'animate-spin' : '')} />}
            onClick={() => refetch()}
            disabled={isRefetching}
          >
            {t('history.refresh')}
          </Button>
        </div>
      </Card>

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
          icon={<HistoryIcon className="w-5 h-5" />}
          title={t('history.empty.title')}
          description={
            data && data.length === 0
              ? t('history.empty.all')
              : t('history.empty.filtered', { status: t(`history.filter.${statusFilter}`) })
          }
        />
      ) : (
        <DataTable
          rows={filtered}
          columns={columns}
          rowKey={(b) => b.id}
          pageSize={25}
        />
      )}
    </div>
  );
}
