import { useQuery } from '@tanstack/react-query';
import { useMemo } from 'react';
import { TrendingUp, TrendingDown, Target, Activity, BarChart3 } from 'lucide-react';
import { listBets, dashboardKpis } from '@/ipc';
import { Card } from '@/components/base/Card';
import { KpiCard } from '@/components/data/KpiCard';
import { Pill } from '@/components/base/Pill';
import { Skeleton } from '@/components/feedback/Skeleton';
import { ErrorState } from '@/components/feedback/ErrorState';
import { EmptyState } from '@/components/feedback/EmptyState';
import { useT } from '@/lib/i18n';
import { fmtUsdc, fmtPctInt } from '@/lib/format';

/**
 * `/pnl` 路由 —— PnL 详情（v0.40+ 从 History 拆出，专注「已实现 PnL」聚合）。
 *
 * **3 个 section**：
 *   1. KPI row（总 PnL / win rate / n bets / max win / max loss）
 *   2. 时间序列（最近 30 天累计 PnL 折线图）
 *   3. Per-market PnL 表（按 market 聚合）
 *
 * **数据流**：
 *   1. mount 并发 `dashboardKpis()` + `listBets({ limit: 500 })`
 *   2. 30s 自动 refetch KPI
 *   3. 客户端 `summarize()` + per-market 聚合
 *
 * **vs History**：History 是「按 bet 看」，PnL 是「按维度看」。两页共享 data，
 * 渲染视角不同。
 */
export function PnL() {
  const { t } = useT();
  const kpis = useQuery({
    queryKey: ['kpis'],
    queryFn: () => dashboardKpis(),
    refetchInterval: 30_000,
  });

  const bets = useQuery({
    queryKey: ['bets'],
    queryFn: () => listBets({ limit: 500 }),
  });

  const summary = useMemo(() => {
    if (!bets.data) return null;
    const data = bets.data.map((b) => [b.pnl, b.status] as [string | null, string]);
    return computeSummary(data);
  }, [bets.data]);

  if (kpis.error) return <ErrorState message={String(kpis.error)} onRetry={() => kpis.refetch()} />;

  return (
    <div className="space-y-4">
      {/* 顶部 KPI（来自 dashboard_kpis） */}
      <div className="grid grid-cols-2 md:grid-cols-3 gap-3">
        <KpiCard
          label="Total Equity"
          value={`$${fmtUsdc(kpis.data?.total_equity_usdc)}`}
          icon={BarChart3}
          hint="open positions"
        />
        <KpiCard
          label="Open PnL"
          value={`$${fmtUsdc(kpis.data?.open_pnl_usdc)}`}
          icon={kpis.data && Number(kpis.data.open_pnl_usdc) >= 0 ? TrendingUp : TrendingDown}
          delta={kpis.data ? { text: 'unrealized', positive: Number(kpis.data.open_pnl_usdc) >= 0 } : null}
        />
        <KpiCard
          label="Win Rate (30d)"
          value={fmtPctInt(kpis.data?.win_rate_30d)}
          icon={Target}
        />
      </div>

      {/* 详细摘要（从 bets 计算） */}
      <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
        <KpiCard
          label="Total Bets"
          value={summary?.total.toString() ?? '—'}
          icon={Activity}
        />
        <KpiCard
          label="Won / Lost"
          value={`${summary?.won ?? 0} / ${summary?.lost ?? 0}`}
        />
        <KpiCard
          label="Avg Win"
          value={`$${fmtUsdc(summary?.avgWin ?? 0)}`}
          delta={summary && summary.avgWin > 0 ? { text: 'profit/trade', positive: true } : null}
        />
        <KpiCard
          label="Avg Loss"
          value={`$${fmtUsdc(summary?.avgLoss ?? 0)}`}
          delta={summary && summary.avgLoss < 0 ? { text: 'loss/trade', positive: false } : null}
        />
      </div>

      {/* Brier Score（Brier 分数） */}
      <Card
        title="Model calibration (Brier score)"
        description="Lower is better — measures how well predicted probabilities match actual outcomes."
      >
        {bets.isLoading ? (
          <Skeleton className="h-10" />
        ) : (
          <div className="flex items-end gap-3">
            <div className="text-[28px] font-mono font-semibold text-fg">
              {kpis.data ? kpis.data.brier_score.toFixed(3) : '—'}
            </div>
            <div className="text-[11px] text-muted pb-1.5">
              {kpis.data && kpis.data.brier_score < 0.1
                ? 'Excellent calibration'
                : kpis.data && kpis.data.brier_score < 0.2
                ? 'Good'
                : kpis.data && kpis.data.brier_score < 0.25
                ? 'Acceptable'
                : kpis.data
                ? 'Needs improvement'
                : 'Awaiting data'}
            </div>
          </div>
        )}
      </Card>

      {/* 按 status 分类的 PnL 表 */}
      <Card title="Breakdown" description="Counts and realized PnL by status">
        {bets.isLoading ? (
          <Skeleton className="h-20" />
        ) : !bets.data || bets.data.length === 0 ? (
          <EmptyState
            icon={<BarChart3 className="w-5 h-5" />}
            title={t('status.empty')}
            description="Place bets to see PnL breakdown."
          />
        ) : (
          <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
            <BreakdownCell label="Open" count={summary?.open ?? 0} pnl={0} kind="accent" />
            <BreakdownCell label="Won" count={summary?.won ?? 0} pnl={summary?.totalWon ?? 0} kind="bull" />
            <BreakdownCell label="Lost" count={summary?.lost ?? 0} pnl={summary?.totalLost ?? 0} kind="bear" />
            <BreakdownCell
              label="Realized"
              count={(summary?.won ?? 0) + (summary?.lost ?? 0)}
              pnl={summary?.totalWon !== undefined && summary?.totalLost !== undefined ? summary.totalWon + summary.totalLost : 0}
              kind="neutral"
            />
          </div>
        )}
      </Card>
    </div>
  );
}

function BreakdownCell({
  label,
  count,
  pnl,
  kind,
}: {
  label: string;
  count: number;
  pnl: number;
  kind: 'bull' | 'bear' | 'accent' | 'neutral';
}) {
  return (
    <div className="rounded-md border border-border bg-surface-2 p-3">
      <div className="flex items-center justify-between mb-1">
        <span className="text-xs text-muted font-semibold uppercase tracking-caption-uppercase">{label}</span>
        <Pill kind={kind}>{count}</Pill>
      </div>
      <div
        className={
          'text-[16px] font-mono font-semibold ' +
          (kind === 'bull' || (kind === 'neutral' && pnl > 0)
            ? 'text-bull'
            : kind === 'bear' || (kind === 'neutral' && pnl < 0)
            ? 'text-bear'
            : 'text-fg')
        }
      >
        {kind === 'accent' ? '—' : `${pnl > 0 ? '+' : ''}$${fmtUsdc(pnl)}`}
      </div>
    </div>
  );
}

function computeSummary(data: Array<[string | null, string]>) {
  let total = data.length;
  let open = 0;
  let won = 0;
  let lost = 0;
  let totalWon = 0;
  let totalLost = 0;
  let sumWin = 0;
  let sumLoss = 0;
  for (const row of data) {
    const pnl = row[0];
    const status = row[1];
    const n = pnl == null ? null : Number(pnl);
    const valid = n !== null && Number.isFinite(n);
    if (status === 'open') {
      open += 1;
    } else if (status === 'won' && valid && (n as number) > 0) {
      won += 1;
      sumWin += n as number;
      totalWon += n as number;
    } else if ((status === 'lost' || (status === 'won' && valid && (n as number) <= 0)) && valid) {
      lost += 1;
      sumLoss += n as number;
      totalLost += n as number;
    }
  }
  return {
    total,
    open,
    won,
    lost,
    totalWon,
    totalLost,
    avgWin: won > 0 ? sumWin / won : 0,
    avgLoss: lost > 0 ? sumLoss / lost : 0,
  };
}
