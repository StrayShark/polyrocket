import { useQuery } from '@tanstack/react-query';
import { Link } from 'react-router-dom';
import {
  Activity,
  ExternalLink,
  Plus,
  TrendingUp,
  TrendingDown,
  Zap,
  Target,
  BarChart3,
} from 'lucide-react';
import { dashboardKpis, listActiveSignals, listBets } from '@/ipc';
import { Card } from '@/components/base/Card';
import { Pill } from '@/components/base/Pill';
import { Button } from '@/components/base/Button';
import { KpiCard } from '@/components/data/KpiCard';
import { Skeleton } from '@/components/feedback/Skeleton';
import { ErrorState } from '@/components/feedback/ErrorState';
import { EmptyState } from '@/components/feedback/EmptyState';
import { fmtUsdc, fmtPct, fmtEdge, fmtRelativeTime } from '@/lib/format';

export function Dashboard() {
  const kpis = useQuery({
    queryKey: ['kpis'],
    queryFn: () => dashboardKpis(),
    refetchInterval: 30_000,
  });
  const signals = useQuery({
    queryKey: ['signals', 'active', { limit: 50 }],
    queryFn: () => listActiveSignals({ limit: 50 }),
  });
  const bets = useQuery({
    queryKey: ['bets', { limit: 50 }],
    queryFn: () => listBets({ limit: 50 }),
  });

  const openBets = (bets.data ?? []).filter((b) => b.status === 'open').slice(0, 5);
  const topSignals = (signals.data ?? []).slice(0, 5);

  if (kpis.error) return <ErrorState message={String(kpis.error)} onRetry={() => kpis.refetch()} />;

  return (
    <div className="space-y-4">
      {/* Top KPIs */}
      <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
        <KpiCard
          label="Total Equity"
          value={`$${fmtUsdc(kpis.data?.total_equity_usdc)}`}
          icon={BarChart3}
          hint={`${kpis.data?.open_positions ?? 0} open`}
        />
        <KpiCard
          label="Open PnL"
          value={`$${fmtUsdc(kpis.data?.open_pnl_usdc)}`}
          icon={
            kpis.data && Number(kpis.data.open_pnl_usdc) >= 0 ? TrendingUp : TrendingDown
          }
          delta={
            kpis.data
              ? {
                  text: Number(kpis.data.open_pnl_usdc) >= 0 ? 'profit' : 'loss',
                  positive: Number(kpis.data.open_pnl_usdc) >= 0,
                }
              : null
          }
        />
        <KpiCard
          label="Win Rate (30d)"
          value={fmtPct(kpis.data?.win_rate_30d)}
          icon={Target}
        />
        <KpiCard
          label="Brier Score"
          value={kpis.data ? kpis.data.brier_score.toFixed(3) : '—'}
          icon={Activity}
          hint={
            kpis.data
              ? kpis.data.brier_score < 0.2
                ? 'good'
                : kpis.data.brier_score < 0.25
                ? 'fair'
                : 'poor'
              : ''
          }
        />
      </div>

      {/* Two columns: signals + open positions */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
        {/* Top signals */}
        <Card
          title="Top signals"
          description="Highest-edge active signals across all markets"
          action={
            <Link to="/signals">
              <Button variant="ghost" size="xs">View all</Button>
            </Link>
          }
        >
          {signals.isLoading ? (
            <div className="space-y-2">
              {Array.from({ length: 3 }).map((_, i) => (
                <Skeleton key={i} className="h-14" />
              ))}
            </div>
          ) : topSignals.length === 0 ? (
            <EmptyState
              icon={<Zap className="w-5 h-5" />}
              title="No active signals"
              description="Run recompute_signals to generate predictions."
            />
          ) : (
            <div className="space-y-2">
              {topSignals.map((s) => (
                <div
                  key={s.id}
                  className="rounded-md border border-border bg-surface-2 p-2.5 flex items-center gap-3"
                >
                  <div className="flex-1 min-w-0">
                    <Link
                      to={`/markets/${s.market_id}`}
                      className="text-[12px] text-fg hover:text-accent line-clamp-1"
                    >
                      {s.market_question ?? s.market_id}
                    </Link>
                    <div className="text-[10px] text-muted mt-0.5">
                      model {s.model_version} · {s.horizon_hours}h horizon
                    </div>
                  </div>
                  <Pill kind={s.edge > 0 ? 'bull' : 'bear'}>
                    {s.edge > 0 ? 'YES' : 'NO'}
                  </Pill>
                  <div
                    className={
                      'font-mono font-semibold text-[12px] ' +
                      (s.edge > 0 ? 'text-bull' : 'text-bear')
                    }
                  >
                    {fmtEdge(s.edge)}
                  </div>
                </div>
              ))}
            </div>
          )}
        </Card>

        {/* Open positions */}
        <Card
          title="Open positions"
          description="Unsettled bets"
          action={
            <Link to="/history">
              <Button variant="ghost" size="xs">View all</Button>
            </Link>
          }
        >
          {bets.isLoading ? (
            <div className="space-y-2">
              {Array.from({ length: 3 }).map((_, i) => (
                <Skeleton key={i} className="h-14" />
              ))}
            </div>
          ) : openBets.length === 0 ? (
            <EmptyState
              icon={<Activity className="w-5 h-5" />}
              title="No open positions"
              description="Place a bet from a Market Detail page."
              action={
                <Link to="/markets">
                  <Button variant="primary" size="sm" iconLeft={<Plus className="w-3 h-3" />}>
                    Browse markets
                  </Button>
                </Link>
              }
            />
          ) : (
            <div className="space-y-2">
              {openBets.map((b) => (
                <div
                  key={b.id}
                  className="rounded-md border border-border bg-surface-2 p-2.5 flex items-center gap-3"
                >
                  <div className="flex-1 min-w-0">
                    <Link
                      to={`/markets/${b.market_id}`}
                      className="text-[12px] text-fg hover:text-accent line-clamp-1"
                    >
                      {b.market_id}
                    </Link>
                    <div className="text-[10px] text-muted mt-0.5">
                      ${fmtUsdc(b.size)} @ {b.price.toFixed(3)} · {fmtRelativeTime(b.placed_at)}
                    </div>
                  </div>
                  <Pill kind={b.side === 'YES' ? 'bull' : 'bear'}>{b.side}</Pill>
                  <Pill kind="muted">{b.mode === 'A_jump' ? 'A' : 'B'}</Pill>
                  {b.tx_hash && (
                    <a
                      href={`https://polygonscan.com/tx/${b.tx_hash}`}
                      target="_blank"
                      rel="noopener noreferrer"
                      className="text-muted hover:text-accent"
                    >
                      <ExternalLink className="w-3 h-3" />
                    </a>
                  )}
                </div>
              ))}
            </div>
          )}
        </Card>
      </div>
    </div>
  );
}
