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

export function ModelLab() {
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
  const activeModel = useQuery({
    queryKey: ['sidecar-active-model'],
    queryFn: async () => {
      try {
        const snap = await sidecarHealthSnapshot();
        if (snap.success_count === 0) return null;
        const r = await sidecarPredict([['__probe__', 0.5]]);
        return r.model_version;
      } catch {
        return null;
      }
    },
    staleTime: 60_000,
  });

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between gap-3">
        <h2 className="text-[14px] font-medium text-fg">Model performance</h2>
        <ModelVersionPill modelVersion={activeModel.data} variant="verbose" />
      </div>
      <div className="grid grid-cols-2 md:grid-cols-3 gap-3">
        <KpiCard label="Model versions" value={total.toString()} icon={GitBranch} />
        <KpiCard
          label="Best Brier"
          value={best ? best.brier_score.toFixed(3) : '—'}
          icon={FlaskConical}
          hint={best ? best.model_version : ''}
        />
        <KpiCard
          label="Best win rate"
          value={best ? fmtPct(best.win_rate) : '—'}
          hint={best ? best.model_version : ''}
        />
      </div>

      <Card
        title="Model performance"
        description="Per-version stats. Lower Brier is better; higher win rate is better."
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
            title="No model versions"
            description="Train your first model — v0.4.0 will run the Python sidecar in M7 phase 2."
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
                    {p.n_predictions} predictions
                  </div>
                </div>
                <Metric label="Win rate" value={fmtPct(p.win_rate)} positive={p.win_rate >= 0.5} />
                <Metric label="Brier" value={p.brier_score.toFixed(3)} positive={p.brier_score < 0.2} />
                <Metric label="Log loss" value={p.log_loss.toFixed(3)} positive={p.log_loss < 0.5} />
                <Metric label="Avg edge" value={fmtPct(p.avg_edge)} positive={p.avg_edge > 0} />
              </div>
            ))}
          </div>
        )}
      </Card>

      <Card title="Training runs" description="Past and queued model training jobs">
        <EmptyState
          icon={<Play className="w-5 h-5" />}
          title="No training runs yet"
          description="Trigger a training job from Settings → Model Lab in v0.4.1."
        />
      </Card>

      <Card title="Run state machine" description="Reference: legal transitions for training jobs">
        <div className="flex items-center gap-2 text-[12px] flex-wrap">
          <Pill kind="muted">
            <Clock className="w-2.5 h-2.5" /> queued
          </Pill>
          <span className="text-muted">→</span>
          <Pill kind="accent">
            <Play className="w-2.5 h-2.5" /> running
          </Pill>
          <span className="text-muted">→</span>
          <Pill kind="bull">
            <CheckCircle2 className="w-2.5 h-2.5" /> done
          </Pill>
          <span className="text-muted">|</span>
          <Pill kind="bear">
            <XCircle className="w-2.5 h-2.5" /> error
          </Pill>
        </div>
        <div className="text-[11px] text-muted mt-3">
          <code className="font-mono text-fg">queued → running → (done | error)</code> — both done and error are terminal.
          Version promotion uses <code className="font-mono">is_better()</code>: lower Brier wins, tiebreak by win rate, then by n_predictions.
        </div>
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
