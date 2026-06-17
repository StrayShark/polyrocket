import { useState, useRef, useEffect } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { FlaskConical, GitBranch, Play, CheckCircle2, XCircle, Clock, Sparkles, ArrowUpCircle } from 'lucide-react';
import {
  llmPerformance,
  sidecarPredict,
  sidecarHealthSnapshot,
  trainJob,
  promoteModel,
  onTrainStarted,
  type TrainStartedEvent,
} from '@/ipc';
import { Card } from '@/components/base/Card';
import { Pill } from '@/components/base/Pill';
import { KpiCard } from '@/components/data/KpiCard';
import { Button } from '@/components/base/Button';
import { Skeleton } from '@/components/feedback/Skeleton';
import { ErrorState } from '@/components/feedback/ErrorState';
import { EmptyState } from '@/components/feedback/EmptyState';
import { ModelVersionPill } from '@/components/feedback/ModelVersionPill';
import { TrainProgress } from '@/components/feedback/TrainProgress';
import { PromoteHistory } from '@/components/feedback/PromoteHistory';
import { fmtPct } from '@/lib/format';
import { useT } from '@/lib/i18n';
import { toast } from '@/stores/toast-store';

export function ModelLab() {
  const { t } = useT();
  const queryClient = useQueryClient();
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

  // v0.17d — model training state. Same chicken-and-egg
  // pattern as v0.15c Analysis page: the Rust `train_job`
  // IPC emits `train_job:started` BEFORE returning, so we
  // can't get the job_id from the IPC return value. We
  // listen for the next `started` event after the user
  // clicks Train, capture the id, and pass it to
  // `TrainProgress`. On `finished`, we toast the result
  // and invalidate the per-version query (a successful
  // train writes a new candidate.json; once promoted
  // the per-version list updates).
  const [activeTrainJobId, setActiveTrainJobId] = useState<string | null>(null);
  const expectedTrainRef = useRef<boolean>(false);
  useEffect(() => {
    let cancelled = false;
    const unsubStarted = onTrainStarted((e: TrainStartedEvent) => {
      if (cancelled) return;
      if (expectedTrainRef.current) {
        setActiveTrainJobId(e.job_id);
        expectedTrainRef.current = false;
      }
    });
    return () => {
      cancelled = true;
      unsubStarted.then((u) => u()).catch(() => { /* ignore */ });
    };
  }, []);

  const trainMut = useMutation({
    mutationFn: () => trainJob({}),
    onSuccess: (r) => {
      // IPC returned AFTER the `finished` event fired.
      // The TrainProgress panel already shows the final
      // state. We just toast the summary, remember the
      // candidate for the Promote button, and refresh
      // any dependent queries.
      if (r.status === 'completed') {
        toast.success(
          t('train.toast.completed'),
          r.best_brier != null
            ? t('train.toast.brier', { value: r.best_brier.toFixed(3) })
            : '',
        );
        // v0.18c — remember the candidate so the user
        // can promote it without re-entering the job_id.
        setLastCandidate({
          jobId: r.job_id,
          candidatePath: r.candidate_path,
          bestBrier: r.best_brier,
        });
      } else {
        toast.error(t('train.toast.failed'), r.message ?? undefined);
        setLastCandidate(null);
      }
      setActiveTrainJobId(null);
      queryClient.invalidateQueries({ queryKey: ['llm-performance'] });
      queryClient.invalidateQueries({ queryKey: ['sidecar-active-model'] });
    },
    onError: (e: Error) => {
      toast.error(t('train.toast.failed'), e.message);
      setActiveTrainJobId(null);
    },
  });

  // v0.18c — remember the last successful train so the
  // Promote button can pass the right `job_id` (race-
  // condition protection: the Python sidecar refuses
  // to promote a candidate from a different job, so
  // we capture the job_id at train time and pass it
  // to promote to ensure we're promoting the model
  // we just trained, not some older one).
  const [lastCandidate, setLastCandidate] = useState<{
    jobId: string;
    candidatePath: string | null;
    bestBrier: number | null;
  } | null>(null);

  const promoteMut = useMutation({
    mutationFn: () =>
      promoteModel(lastCandidate ? { job_id: lastCandidate.jobId } : {}),
    onSuccess: (r) => {
      if (r.promoted) {
        toast.success(
          t('promote.toast.promoted'),
          t('promote.toast.version', { version: r.model_version }),
        );
        setLastCandidate(null);
      } else {
        toast.error(t('promote.toast.failed'), r.message ?? undefined);
      }
      // Refresh the active-model probe so the
      // ModelVersionPill updates to the new version.
      queryClient.invalidateQueries({ queryKey: ['sidecar-active-model'] });
      queryClient.invalidateQueries({ queryKey: ['llm-performance'] });
      // v0.19c — refresh the promote-history panel so the
      // new entry appears at the top.
      queryClient.invalidateQueries({ queryKey: ['promote-history'] });
    },
    onError: (e: Error) => {
      toast.error(t('promote.toast.failed'), e.message);
    },
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

      <Card
        title={t('modellab.runs.title')}
        description={t('modellab.runs.desc')}
        action={
          <div className="flex items-center gap-2">
            <Button
              data-testid="model-train-btn"
              variant="primary"
              size="sm"
              iconLeft={<Sparkles className="w-3 h-3" />}
              loading={trainMut.isPending}
              disabled={trainMut.isPending}
              onClick={() => {
                // v0.17d — same pattern as v0.15c: the train_job
                // IPC emits `started` BEFORE returning, so we
                // listen for the next `started` event and
                // capture the id. The flag ensures we only
                // capture events triggered by THIS click.
                expectedTrainRef.current = true;
                trainMut.mutate();
              }}
            >
              {trainMut.isPending ? t('modellab.runs.training') : t('modellab.runs.train')}
            </Button>
            {/* v0.18c — Promote button. Only enabled when
                there's a successful candidate from a
                recent train (lastCandidate is non-null). The
                button passes the candidate's job_id so the
                Python sidecar refuses to promote a different
                job (race-condition protection). */}
            {lastCandidate && (
              <Button
                data-testid="model-promote-btn"
                variant="secondary"
                size="sm"
                iconLeft={<ArrowUpCircle className="w-3 h-3" />}
                loading={promoteMut.isPending}
                disabled={promoteMut.isPending || !!activeTrainJobId}
                onClick={() => promoteMut.mutate()}
              >
                {promoteMut.isPending ? t('promote.btn.promoting') : t('promote.btn.promote')}
              </Button>
            )}
          </div>
        }
      >
        {/* v0.17d — live progress panel. Subscribes to the 2
            `train_job:*` events. Mounts when the user clicks
            Train and the next `started` event fires. */}
        {activeTrainJobId && (
          <TrainProgress
            key={activeTrainJobId}
            jobId={activeTrainJobId}
            className="mt-0"
          />
        )}
        {/* v0.18c — Last-candidate hint. Shown when the user
            has a successful train that hasn't been promoted
            yet. Tells the user what they're about to promote. */}
        {!activeTrainJobId && lastCandidate && (
          <div
            data-testid="model-last-candidate"
            className="flex items-center gap-2 text-[11px] text-muted px-2 py-1.5 rounded border border-dashed border-border"
          >
            <ArrowUpCircle className="w-3.5 h-3.5 text-accent" />
            <span className="flex-1 font-mono text-fg">
              {lastCandidate.jobId}
              {lastCandidate.bestBrier != null && (
                <span className="text-muted ml-2">
                  Brier: {lastCandidate.bestBrier.toFixed(3)}
                </span>
              )}
            </span>
            <span className="text-[10px]">
              {t('promote.last_candidate.hint')}
            </span>
          </div>
        )}
        {!activeTrainJobId && !trainMut.isPending && !lastCandidate && (
          <EmptyState
            icon={<Play className="w-5 h-5" />}
            title={t('modellab.runs.empty')}
            description={t('modellab.runs.empty_desc')}
          />
        )}
      </Card>

      {/* v0.19c — Promotion history. Shows the last 20
          promoted models, newest first. The currently
          active model is NOT in this list (use the
          ModelVersionPill at the top of the page for
          that). */}
      <Card
        title={t('promote.history.title')}
        description={t('promote.history.desc')}
      >
        <PromoteHistory />
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
