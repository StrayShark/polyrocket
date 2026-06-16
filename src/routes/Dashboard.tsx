import { useQuery } from '@tanstack/react-query';
import { useMemo } from 'react';
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
  LineChart as LineChartIcon,
} from 'lucide-react';
import { dashboardKpis, listActiveSignals, listBets } from '@/ipc';
import { Card } from '@/components/base/Card';
import { Pill } from '@/components/base/Pill';
import { Button } from '@/components/base/Button';
import { KpiCard } from '@/components/data/KpiCard';
import { Sparkline, cumulativeSum } from '@/components/data/Sparkline';
import { BarChart } from '@/components/data/BarChart';
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

  // Equity curve: cumulative PnL across settled bets (newest first → reverse)
  const settledPnls = useMemo(() => {
    const settled = (bets.data ?? [])
      .filter((b) => b.status === 'won' || b.status === 'lost')
      .filter((b) => b.pnl != null)
      .sort((a, b) => a.placed_at - b.placed_at);
    return settled.map((b) => Number(b.pnl));
  }, [bets.data]);
  const equityCurve = useMemo(() => cumulativeSum(settledPnls), [settledPnls]);
  const totalPnl = equityCurve.length > 0 ? equityCurve[equityCurve.length - 1] : 0;

  // Calibration by edge bucket: avg signal edge per 5% bucket
  const calibrationBuckets = useMemo(() => {
    const buckets: Array<{ label: string; avgEdge: number; n: number }> = [];
    for (let lo = 0; lo < 50; lo += 5) {
      const inBucket = (signals.data ?? []).filter((s) => {
        const a = Math.abs(s.edge) * 100;
        return a >= lo && a < lo + 5;
      });
      const avg = inBucket.length === 0
        ? 0
        : inBucket.reduce((acc, s) => acc + s.edge, 0) / inBucket.length;
      buckets.push({ label: `${lo}-${lo + 5}%`, avgEdge: avg, n: inBucket.length });
    }
    return buckets;
  }, [signals.data]);

  // Recent activity: latest 6 audit-y events (resolved bets + new signals)
  const recentActivity = useMemo(() => {
    const items: Array<{ kind: 'bet' | 'signal'; at: number; text: string }> = [];
    for (const b of (bets.data ?? []).slice(0, 20)) {
      if (b.status === 'won' || b.status === 'lost') {
        items.push({
          kind: 'bet',
          at: b.placed_at,
          text: `${b.status.toUpperCase()} ${b.side} $${fmtUsdc(b.size)} @ ${b.price.toFixed(3)} (${b.pnl && Number(b.pnl) >= 0 ? '+' : ''}$${fmtUsdc(b.pnl)})`,
        });
      }
    }
    for (const s of (signals.data ?? []).slice(0, 20)) {
      items.push({
        kind: 'signal',
        at: s.computed_at,
        text: `${s.edge > 0 ? 'YES' : 'NO'} ${fmtEdge(s.edge)} on ${(s.market_question ?? s.market_id).slice(0, 40)}`,
      });
    }
    return items.sort((a, b) => b.at - a.at).slice(0, 6);
  }, [bets.data, signals.data]);

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

      {/* Charts row: equity curve + calibration */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
        <Card
          title="Equity curve"
          description="Cumulative realized PnL across settled bets"
          action={
            <span
              className={
                'text-[14px] font-mono font-semibold ' +
                (totalPnl >= 0 ? 'text-bull' : 'text-bear')
              }
            >
              {totalPnl >= 0 ? '+' : ''}${fmtUsdc(totalPnl)}
            </span>
          }
        >
          {equityCurve.length === 0 ? (
            <EmptyState
              icon={<LineChartIcon className="w-5 h-5" />}
              title="No settled bets"
              description="Resolve some bets to see the equity curve."
            />
          ) : (
            <div className="flex flex-col gap-2">
              <Sparkline
                values={equityCurve}
                width={520}
                height={100}
                yPad={0.15}
                refLines={[heightForZero(equityCurve, 100, 0.15)]}
              />
              <div className="flex items-center justify-between text-[10px] text-muted">
                <span>{equityCurve.length} settled bets</span>
                <span>
                  from {fmtUsdc(equityCurve[0])} → {fmtUsdc(totalPnl)}
                </span>
              </div>
            </div>
          )}
        </Card>

        <Card
          title="Signal calibration"
          description="Avg |edge| per 5% bucket — wider bars = stronger conviction"
        >
          {calibrationBuckets.every((b) => b.n === 0) ? (
            <EmptyState
              icon={<BarChart3 className="w-5 h-5" />}
              title="No signals"
              description="Recompute signals to see calibration."
            />
          ) : (
            <div className="flex justify-center">
              <BarChart
                data={calibrationBuckets.map((b) => ({ label: b.label, value: b.avgEdge * 100 }))}
                width={520}
                height={100}
                signedColor
              />
            </div>
          )}
        </Card>
      </div>

      {/* Recent activity timeline */}
      <Card title="Recent activity" description="Latest 6 events: settled bets + new signals">
        {recentActivity.length === 0 ? (
          <EmptyState
            icon={<Activity className="w-5 h-5" />}
            title="No activity"
            description="Activity will appear as bets settle and signals compute."
          />
        ) : (
          <div className="space-y-1.5">
            {recentActivity.map((it, i) => (
              <div
                key={i}
                className="flex items-center gap-3 rounded-md border border-border bg-surface-2 px-3 py-2"
              >
                <Pill kind={it.kind === 'bet' ? 'bull' : 'accent'}>{it.kind}</Pill>
                <span className="flex-1 text-[12px] text-fg font-mono truncate">{it.text}</span>
                <span className="text-[10px] text-muted shrink-0">{fmtRelativeTime(it.at)}</span>
              </div>
            ))}
          </div>
        )}
      </Card>

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

/** Compute the y-pixel of zero on the sparkline (for the ref line). */
function heightForZero(values: number[], h: number, yPad: number): number {
  if (values.length === 0) return h / 2;
  const min = Math.min(...values);
  const max = Math.max(...values);
  const range = max - min;
  const pad = range === 0 ? h * 0.1 : range * yPad;
  const yMin = min - pad;
  const yMax = max + pad;
  const yRange = yMax - yMin;
  return h - ((0 - yMin) / yRange) * h;
}
