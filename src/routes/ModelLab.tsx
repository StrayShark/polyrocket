import { useQuery } from '@tanstack/react-query';
import { FlaskConical, GitBranch, Play, CheckCircle2, XCircle, Clock } from 'lucide-react';
import { llmPerformance, sidecarPredict, sidecarHealthSnapshot } from '@/ipc';
import { Card } from '@/components/base/Card';
import { Pill } from '@/components/base/Pill';
import { KpiCard } from '@/components/data/KpiCard';
import { Skeleton } from '@/components/feedback/Skeleton';
import { ErrorState } from '@/components/feedback/ErrorState';
import { EmptyState } from '@/components/feedback/EmptyState';
import { ModelVersionPill } from '@/components/feedback/ModelVersionPill';
import { fmtPct } from '@/lib/format';
import { useT } from '@/lib/i18n';

export function ModelLab() {
  const { t } = useT();
  const { data, isLoading, error, refetch } = useQuery({
    queryKey: ['llm-performance'],
    queryFn: () => llmPerformance(),
    staleTime: 60_000,
  });

  if (error) return <ErrorState message={String(error)} onRetry={() => refetch()} />;

  const total = data?.length ?? 0;
  const best = data && data.length > 0
    ? data.reduce((acc, p) => (p.brier_score < acc.brier_score ? p : acc))
    : null;

  // v0.12d — the "currently active" model. We hit the sidecar
  // with a cheap predict (m1/m2) to surface the model_version
  // that future predict calls will use.
  // v0.13b — also returns the brier score for the tooltip.
  const activeModel = useQuery({
    queryKey: ['sidecar-active-model'],
    queryFn: async () => {
      try {
        const snap = await sidecarHealthSnapshot();
        if (snap.success_count === 0) return null;
        const r = await sidecarPredict([['__probe__', 0.5]]);
        return r;
      } catch {
        return null;
      }
    },
    staleTime: 60_000,
  });

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between gap-3">
        <h2 className="text-[14px] font-medium text-fg">{t('modellab.title')}</h2>
        <ModelVersionPill
          modelVersion={activeModel.data?.model_version}
          brierScore={activeModel.data?.brier_score}
          variant="verbose"
        />
      </div>
      <div className="grid grid-cols-2 md:grid-cols-3 gap-3">
        <KpiCard label={t('modellab.kpi.versions')} value={total.toString()} icon={GitBranch} />
        <KpiCard
          label={t('modellab.kpi.best_brier')}
          value={best ? best.brier_score.toFixed(3) : '—'}
          icon={FlaskConical}
          hint={best ? best.model_version : ''}
        />
        <KpiCard
          label={t('modellab.kpi.best_winrate')}
          value={best ? fmtPct(best.win_rate) : '—'}
          hint={best ? best.model_version : ''}
        />
      </div>

      <Card
        title={t('modellab.perf.title')}
        description={t('modellab.perf.desc')}
      >
        {isLoading ? (
          <div className="space-y-2">
            {Array.from({ length: 4 }).map((_, i) => (
              <Skeleton key={i} className="h-7" />
            ))}
          </div>
        ) : !data || data.length === 0 ? (
          <EmptyState
            icon={<FlaskConical className="w-5 h-5" />}
            title={t('modellab.perf.empty')}
            description={t('modellab.perf.empty_desc')}
          />
        ) : (
          <div className="space-y-2">
            {data.map((p) => (
              <div
                key={p.model_version}
                className="rounded-md border border-border bg-surface-2 p-3 flex items-center gap-4"
              >
                <div className="flex-1 min-w-0">
                  <div className="font-mono text-[13px] text-fg">{p.model_version}</div>
                  <div className="text-[10px] text-muted mt-0.5">
                    {t('modellab.perf.predictions', { n: p.n_predictions })}
                  </div>
                </div>
                <Metric label={t('modellab.perf.metric.winrate')} value={fmtPct(p.win_rate)} positive={p.win_rate >= 0.5} />
                <Metric label={t('modellab.perf.metric.brier')} value={p.brier_score.toFixed(3)} positive={p.brier_score < 0.2} />
                <Metric label={t('modellab.perf.metric.logloss')} value={p.log_loss.toFixed(3)} positive={p.log_loss < 0.5} />
                <Metric label={t('modellab.perf.metric.avgedge')} value={fmtPct(p.avg_edge)} positive={p.avg_edge > 0} />
              </div>
            ))}
          </div>
        )}
      </Card>

      <Card title={t('modellab.runs.title')} description={t('modellab.runs.desc')}>
        <EmptyState
          icon={<Play className="w-5 h-5" />}
          title={t('modellab.runs.empty')}
          description={t('modellab.runs.empty_desc')}
        />
      </Card>

      <Card title={t('modellab.sm.title')} description={t('modellab.sm.desc')}>
        <div className="flex items-center gap-2 text-[12px] flex-wrap">
          <Pill kind="muted">
            <Clock className="w-2.5 h-2.5" /> {t('modellab.sm.queued')}
          </Pill>
          <span className="text-muted">→</span>
          <Pill kind="accent">
            <Play className="w-2.5 h-2.5" /> {t('modellab.sm.running')}
          </Pill>
          <span className="text-muted">→</span>
          <Pill kind="bull">
            <CheckCircle2 className="w-2.5 h-2.5" /> {t('modellab.sm.done')}
          </Pill>
          <span className="text-muted">|</span>
          <Pill kind="bear">
            <XCircle className="w-2.5 h-2.5" /> {t('modellab.sm.error')}
          </Pill>
        </div>
        <div
          className="text-[11px] text-muted mt-3"
          dangerouslySetInnerHTML={{ __html: t('modellab.sm.note') }}
        />
      </Card>
    </div>
  );
}

function Metric({ label, value, positive }: { label: string; value: string; positive: boolean }) {
  return (
    <div className="text-right shrink-0">
      <div className="text-[10px] text-muted uppercase tracking-wide">{label}</div>
      <div className={'font-mono text-[12px] ' + (positive ? 'text-bull' : 'text-fg')}>
        {value}
      </div>
    </div>
  );
}
