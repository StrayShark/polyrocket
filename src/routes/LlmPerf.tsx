import { useQuery } from '@tanstack/react-query';
import { BarChart3, TrendingUp, TrendingDown, Activity, Download } from 'lucide-react';
import { llmPerformance, llmStatsByConfidence, llmStatsByPrompt, llmStatsCostEfficiency, llmStatsExport } from '@/ipc';
import { Card } from '@/components/base/Card';
import { Pill } from '@/components/base/Pill';
import { KpiCard } from '@/components/data/KpiCard';
import { Button } from '@/components/base/Button';
import { Skeleton } from '@/components/feedback/Skeleton';
import { EmptyState } from '@/components/feedback/EmptyState';
import { fmtPct, fmtCents } from '@/lib/format';
import { downloadCsv, toCsv } from '@/lib/csv';
import { toast } from '@/stores/toast-store';

export function LlmPerf() {
  const perf = useQuery({ queryKey: ['llm-performance'], queryFn: () => llmPerformance() });
  const byConf = useQuery({ queryKey: ['llm-stats-by-conf'], queryFn: () => llmStatsByConfidence() });
  const byPrompt = useQuery({ queryKey: ['llm-stats-by-prompt'], queryFn: () => llmStatsByPrompt() });
  const costEff = useQuery({ queryKey: ['llm-stats-cost'], queryFn: () => llmStatsCostEfficiency() });

  const totalCost = (costEff.data ?? []).reduce((acc, c) => acc + c.cost_cents, 0);
  const totalWins = (costEff.data ?? []).reduce((acc, c) => acc + c.wins, 0);
  const avgRoi = (costEff.data ?? []).length === 0
    ? 0
    : (costEff.data ?? []).reduce((acc, c) => acc + c.roi, 0) / (costEff.data ?? []).length;

  return (
    <div className="space-y-4">
      <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
        <KpiCard label="Models tracked" value={(perf.data?.length ?? 0).toString()} icon={BarChart3} />
        <KpiCard label="Total cost" value={fmtCents(totalCost)} icon={Activity} />
        <KpiCard label="Total wins" value={totalWins.toString()} icon={TrendingUp} />
        <KpiCard
          label="Avg ROI"
          value={fmtPct(avgRoi)}
          icon={avgRoi >= 0 ? TrendingUp : TrendingDown}
          delta={{ text: avgRoi >= 0 ? 'profitable' : 'unprofitable', positive: avgRoi >= 0 }}
        />
      </div>

      {/* By confidence bucket */}
      <Card title="By confidence bucket" description="Win rate grouped by LLM confidence decile (0-10, 10-20, ...)">
        {byConf.isLoading ? (
          <Skeleton className="h-24" />
        ) : !byConf.data || byConf.data.length === 0 ? (
          <EmptyState title="No data" description="Run more analyses to populate this view." />
        ) : (
          <div className="space-y-1.5">
            {byConf.data.map((b) => (
              <div key={b.bucket} className="flex items-center gap-2 text-[12px]">
                <span className="w-20 text-muted font-mono">
                  {(b.bucket * 10).toFixed(0)}-{(b.bucket * 10 + 10).toFixed(0)}%
                </span>
                <div className="flex-1 h-3 bg-surface-2 rounded overflow-hidden">
                  <div
                    className={b.win_rate >= 0.5 ? 'h-full bg-bull' : 'h-full bg-bear'}
                    style={{ width: `${Math.max(2, b.win_rate * 100)}%` }}
                  />
                </div>
                <span className="font-mono w-16 text-right">{fmtPct(b.win_rate)}</span>
                <span className="text-muted w-12 text-right">n={b.n}</span>
              </div>
            ))}
          </div>
        )}
      </Card>

      {/* By prompt version */}
      <Card title="By prompt version" description="Which prompt template is performing best?">
        {byPrompt.isLoading ? (
          <Skeleton className="h-20" />
        ) : !byPrompt.data || byPrompt.data.length === 0 ? (
          <EmptyState title="No data" />
        ) : (
          <div className="space-y-1.5">
            {byPrompt.data.map((p) => (
              <div
                key={p.prompt_version}
                className="flex items-center gap-2 rounded-md border border-border bg-surface-2 p-2.5"
              >
                <Pill kind="accent">{p.prompt_version}</Pill>
                <span className="font-mono text-[12px] text-fg flex-1">win rate {fmtPct(p.win_rate)}</span>
                <span className="text-muted text-[11px]">n={p.n}</span>
              </div>
            ))}
          </div>
        )}
      </Card>

      {/* Cost efficiency */}
      <Card
        title="Cost efficiency by provider"
        description="ROI = wins / cost. Higher is better."
        action={
          <Button
            variant="ghost"
            size="sm"
            iconLeft={<Download className="w-3 h-3" />}
            onClick={async () => {
              try {
                const csv = await llmStatsExport('csv');
                downloadCsv(`llm_stats_${Date.now()}.csv`, csv);
                toast.success('Exported');
              } catch (e) {
                toast.error('Export failed', String(e));
              }
            }}
          >
            Export CSV
          </Button>
        }
      >
        {costEff.isLoading ? (
          <Skeleton className="h-20" />
        ) : !costEff.data || costEff.data.length === 0 ? (
          <EmptyState title="No data" />
        ) : (
          <div className="space-y-1.5">
            {costEff.data.map((c) => (
              <div
                key={c.provider_id}
                className="flex items-center gap-2 rounded-md border border-border bg-surface-2 p-2.5"
              >
                <span className="text-[12px] font-mono text-fg flex-1">{c.provider_id}</span>
                <span className="text-[11px] text-muted">cost {fmtCents(c.cost_cents)}</span>
                <span className="text-[11px] text-muted">wins {c.wins}</span>
                <span className={'text-[12px] font-mono font-semibold ' + (c.roi >= 0 ? 'text-bull' : 'text-bear')}>
                  ROI {c.roi >= 0 ? '+' : ''}{c.roi.toFixed(2)}
                </span>
              </div>
            ))}
            <details className="text-[10px] text-muted pt-2">
              <summary className="cursor-pointer hover:text-fg">Raw CSV preview</summary>
              <pre className="mt-1 font-mono whitespace-pre-wrap text-[10px]">
                {toCsv(
                  (costEff.data ?? []).map((c) => ({
                    provider_id: c.provider_id,
                    cost_cents: c.cost_cents,
                    wins: c.wins,
                    roi: c.roi,
                  })),
                )}
              </pre>
            </details>
          </div>
        )}
      </Card>
    </div>
  );
}
