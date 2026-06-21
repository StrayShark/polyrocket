import { useState, useEffect } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { FlaskConical, GitBranch, Play, CheckCircle2, XCircle, Clock, Sparkles, ArrowUpCircle, Archive, GitCompare } from 'lucide-react';
import {
  llmPerformance,
  sidecarPredict,
  sidecarHealthSnapshot,
  trainJob,
  promoteModel,
  autoPromoteIfBetter,
  promoteAllTrials,
  onTrainStarted,
  onAutoPromoteFinished,
  sendNotification,
  requestNotificationPermission,
  listPromoteHistory,
  listPromoteHistoryArchive,
} from '@/ipc';
import { PromoteHistoryArchive } from '@/components/feedback/PromoteHistoryArchive';
import { ModelComparison } from '@/components/feedback/ModelComparison';
import { BacktestReport } from '@/components/feedback/BacktestReport';
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
import { PromoteHistoryChart } from '@/components/feedback/PromoteHistoryChart';
import { fmtPct } from '@/lib/format';
import { useT } from '@/lib/i18n';
import { toast } from '@/stores/toast-store';
import { usePrefsStore } from '@/stores/prefs-store';
// v0.91 — extracted the train-progress state machine
// out of ModelLab so the deferred v0.83 branch coverage
// gap is now testable in isolation.
import { useTrainProgress } from '@/hooks/useTrainProgress';

/**
 * `/model-lab` 路由 —— model 训练 + 提升 + 实验管理。
 *
 * **5 个 tab**：
 *   1. **Train** —— 选 dataset / params → 启动 train job（async）
 *   2. **Run** —— 实时看 train 进度（`train:progress` 事件流）
 *   3. **Promote** —— 选 archived model → promote 到 active
 *   4. **Backtest** —— 选 model version → 跑回测
 *   5. **Archive** —— 所有 model version 列表（`is_active` 标记）
 *
 * **数据流**：
 *   1. mount 拉 active model / archive / train progress
 *   2. Train 启动 → `trainJob` IPC + 订阅 `train:progress` 事件
 *   3. Promote → `promoteModel` mutation + 失效 archive query
 *
 * **auto-promote 流程**：`trainJob` 完后如果 `autoPromote` prefs 开了 →
 * 后台 `auto_promote_if_better` worker → OS notification。
 */
export function ModelLab() {
  const { t } = useT();
  const queryClient = useQueryClient();
  const { data, isLoading, error, refetch } = useQuery({
    queryKey: ['llm-performance'],
    queryFn: () => llmPerformance(),
    staleTime: 60_000,
  });

  // v0.65a — moved error early-return BELOW the
  // subsequent useQuery / useState / useEffect
  // hooks. Previously the early return sat at
  // line 66 (after only the first useQuery) but
  // more useQuery / useState calls followed, so
  // a re-render after error fired fewer hooks
  // and React warned "Rendered fewer hooks than
  // expected". The actual user impact is small
  // (the page would unmount the broken state on
  // next render anyway) but it's a rules-of-hooks
  // violation that hides future bugs.

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
  // v0.91 — train progress state machine extracted into
  // useTrainProgress hook (src/hooks/useTrainProgress.ts).
  // The hook encapsulates the activeTrainJobId state,
  // expectedTrainRef (for race-condition protection), and
  // the train:started event listener subscription with
  // strict-mode double-mount cleanup.
  //
  // Caller pattern: call markExpected() BEFORE
  // trainMut.mutate(); the next train:started event will
  // populate activeTrainJobId.
  const { activeTrainJobId, markExpected, clearActive } = useTrainProgress({
    onTrainStarted,
  });
  // v0.34a — archive modal open state. Local state,
  // not persisted. Resets to false on remount.
  const [archiveOpen, setArchiveOpen] = useState(false);
  // v0.40b — comparison modal open state.
  const [compareOpen, setCompareOpen] = useState(false);
  // v0.40b — selected entries for comparison. Set of
  // job_ids. We limit to 3 selected; if the user
  // selects a 4th, the oldest is dropped.
  const [selectedForCompare, setSelectedForCompare] = useState<Set<string>>(new Set());
  // v0.43c — single-select for backtest. The backtest
  // is per-model: pick one entry, click "Backtest",
  // the modal opens with that model_version pre-filled.
  const [backtestTarget, setBacktestTarget] = useState<string | null>(null);

  // v0.28c — listen for `auto_promote:finished` events.
  // When a background auto-promote completes (because
  // `autoPromoteAfterTrain` is enabled in Settings), the
  // Rust side emits this event. We:
  //   1. Invalidate the queries that show the active model
  //      and the history panel (so they re-fetch).
  //   2. Show a toast with the result so the user knows
  //      whether their new model was auto-promoted.
  //   3. v0.39b — also send an OS notification (in
  //      addition to the toast) if the
  //      `autoPromoteNotify` pref is on.
  //
  // The listener is registered once on mount and stays
  // alive for the lifetime of the ModelLab page. If the
  // user navigates away and comes back, it re-registers.
  // v0.39b — read the auto-promote-notify flag once at
  // mount time (the listener is registered once too;
  // toggling the flag mid-session takes effect on the
  // next mount).
  const autoPromoteNotify = usePrefsStore((s) => s.autoPromoteNotify);
  // v0.42e-2 — opt-in OS notification for the
  // "skipped" branch. Default false (most users
  // don't want "your training didn't improve
  // anything" pings every train). When ON, the
  // listener sends an OS notification on
  // `promoted === false` events with the
  // sidecar's "reason" (e.g. "candidate not
  // better than active").
  const autoPromoteSkippedNotify = usePrefsStore(
    (s) => s.autoPromoteSkippedNotify,
  );
  // v0.40b — the comparison modal needs the full
  // history data. We use a separate useQuery with
  // the same key as the PromoteHistory panel, so
  // react-query dedupes and shares the cache.
  const historyQuery = useQuery({
    queryKey: ['promote-history'],
    queryFn: () => listPromoteHistory(),
    staleTime: 30_000,
    enabled: compareOpen,
  });
  // v0.42e-3 — fetch weights for the 2-3 selected
  // entries from the archive. The in-memory history
  // has best_params (lr, reg) but NOT weights (w0,
  // w1, w2) — those only live in the sidecar's
  // archive.jsonl. We pull just the selected
  // job_ids (whitelist filter on the Rust side)
  // so the call is bounded even with a large
  // archive.
  const weightsQuery = useQuery({
    queryKey: ['promote-history-archive-weights', [...selectedForCompare].sort()],
    queryFn: () =>
      listPromoteHistoryArchive({
        job_ids: [...selectedForCompare],
        limit: 100,
      }),
    staleTime: 30_000,
    enabled: compareOpen && selectedForCompare.size > 0,
  });
  useEffect(() => {
    let cancelled = false;
    // v0.39b — request notification permission once on
    // mount. If the user grants, the OS notification
    // fires for every auto-promote that completes.
    // If denied, the in-app toast still works.
    requestNotificationPermission().catch(() => {
      // No-op: best-effort. The L1 falls back to
      // in-app toast only.
    });
    const unsub = onAutoPromoteFinished((e) => {
      if (cancelled) return;
      // Refresh everything that depends on the active
      // model: KPI cards, history panel, Brier chart.
      queryClient.invalidateQueries({ queryKey: ['llm-performance'] });
      queryClient.invalidateQueries({ queryKey: ['sidecar-active-model'] });
      queryClient.invalidateQueries({ queryKey: ['promote-history'] });
      if (e.promoted) {
        toast.success(
          t('auto_promote.toast.auto_promoted'),
          e.model_version ?? undefined,
        );
        // v0.39b — also send a real OS notification so
        // the user knows even if they're in another app.
        // Skipped for "skipped" events (those are normal,
        // the user is usually still at the ModelLab page).
        if (autoPromoteNotify) {
          sendNotification(
            'auto_promote',
            t('auto_promote.toast.auto_promoted'),
            e.model_version ?? t('auto_promote.toast.auto_promoted_body'),
          ).catch(() => {
            // Best-effort: the in-app toast already
            // fired, so the user has feedback.
          });
        }
      } else {
        // Don't show an error toast for "skipped" — that's
        // the normal case where the candidate wasn't
        // better. Only show info-level toast for visibility.
        toast.info(t('auto_promote.toast.auto_skipped'), e.message);
        // v0.42e-2 — opt-in OS notification for the
        // skipped branch. Off by default (most
        // users don't want a "no improvement"
        // ping every train). When ON, mirror the
        // promoted-path OS notification flow.
        if (autoPromoteSkippedNotify) {
          sendNotification(
            'auto_promote',
            t('auto_promote.toast.auto_skipped'),
            e.message || t('auto_promote.toast.auto_skipped_body'),
          ).catch(() => {
            // best-effort
          });
        }
      }
    });
    return () => {
      cancelled = true;
      unsub.then((u) => u()).catch(() => { /* ignore */ });
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
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
      // v0.91 — use hook's clearActive() instead of direct setState.
      clearActive();
      queryClient.invalidateQueries({ queryKey: ['llm-performance'] });
      queryClient.invalidateQueries({ queryKey: ['sidecar-active-model'] });
    },
    onError: (e: Error) => {
      toast.error(t('train.toast.failed'), e.message);
      // v0.91 — use hook's clearActive() instead of direct setState.
      clearActive();
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
    // v0.21c — bulk promote: accept an optional trial_index.
    // The variables param is the trial_index (or undefined for
    // the default "promote the best" behavior).
    mutationFn: (trialIndex?: number) =>
      promoteModel({
        ...(lastCandidate ? { job_id: lastCandidate.jobId } : {}),
        ...(trialIndex !== undefined ? { trial_index: trialIndex } : {}),
      }),
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

  // v0.23b — auto-promote if better. One-click action:
  // the Python sidecar compares Brier scores and either
  // promotes (if candidate is at least the margin better)
  // or no-ops with a clear "skipped" reason.
  // v0.23c — the margin is now user-configurable via
  // Settings (default 0.005).
  const autoPromoteBrierMargin = usePrefsStore(
    (s) => s.autoPromoteBrierMargin,
  );
  const autoPromoteMut = useMutation({
    mutationFn: () =>
      autoPromoteIfBetter({
        brier_margin: autoPromoteBrierMargin,
      }),
    onSuccess: (r) => {
      if (r.promoted) {
        const delta = r.active_brier != null && r.candidate_brier != null
          ? (r.active_brier - r.candidate_brier).toFixed(3)
          : '?';
        toast.success(
          t('promote.toast.auto_promoted', { delta: `+${delta}` }),
        );
        setLastCandidate(null);
        // Refresh the active-model probe so the
        // ModelVersionPill updates to the new version.
        queryClient.invalidateQueries({ queryKey: ['sidecar-active-model'] });
        queryClient.invalidateQueries({ queryKey: ['llm-performance'] });
        queryClient.invalidateQueries({ queryKey: ['promote-history'] });
      } else if (r.skipped) {
        // v0.23b — skipped: candidate wasn't meaningfully
        // better. The reason field has the comparison
        // numbers; we show a short toast with the delta
        // and margin.
        const delta = r.active_brier != null && r.candidate_brier != null
          ? (r.active_brier - r.candidate_brier).toFixed(3)
          : '?';
        toast.info(
          t('promote.toast.auto_skipped', {
            delta: `+${delta}`,
            margin: r.margin.toFixed(3),
          }),
          r.reason,
        );
      } else {
        toast.error(t('promote.toast.auto_failed'), r.message ?? undefined);
      }
    },
    onError: (e: Error) => {
      toast.error(t('promote.toast.auto_failed'), e.message);
    },
  });

  // v0.25b — bulk promote all 4 trials. One click
  // promotes every trial in the candidate as a
  // separate version in the history panel. Useful
  // for A/B comparison: the user can see how all 4
  // trials perform on real markets, then rollback
  // to the winner via the v0.20c Rollback button.
  const promoteAllMut = useMutation({
    mutationFn: () => promoteAllTrials(),
    onSuccess: (r) => {
      if (r.ok) {
        // All 4 promoted
        toast.success(t('promote.toast.all_promoted', { count: r.count }));
        setLastCandidate(null);
      } else {
        // Partial success: some promoted, some failed
        const ok = r.results.filter((x) => x.promoted).length;
        if (ok > 0) {
          toast.info(
            t('promote.toast.all_partial', { ok, count: r.count }),
            r.message ?? undefined,
          );
        } else {
          toast.error(t('promote.toast.all_failed'), r.message ?? undefined);
        }
        setLastCandidate(null);
      }
      // Refresh everything that depends on the active model
      queryClient.invalidateQueries({ queryKey: ['sidecar-active-model'] });
      queryClient.invalidateQueries({ queryKey: ['llm-performance'] });
      queryClient.invalidateQueries({ queryKey: ['promote-history'] });
    },
    onError: (e: Error) => {
      toast.error(t('promote.toast.all_failed'), e.message);
    },
  });

  if (error) return <ErrorState message={String(error)} onRetry={() => refetch()} />;

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
                // v0.91 — use hook's markExpected() instead of
                // direct ref mutation.
                markExpected();
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
                onClick={() => promoteMut.mutate(undefined)}
              >
                {promoteMut.isPending ? t('promote.btn.promoting') : t('promote.btn.promote')}
              </Button>
            )}
            {/* v0.23b — Auto-promote if better. One-click
                action: the Python sidecar compares the
                candidate's brier to the active's brier
                and either promotes (if candidate is at
                least the default 0.005 better) or no-ops
                with a clear "skipped" reason. */}
            {lastCandidate && (
              <Button
                data-testid="model-auto-promote-btn"
                variant="ghost"
                size="sm"
                iconLeft={<Sparkles className="w-3 h-3" />}
                loading={autoPromoteMut.isPending}
                disabled={
                  autoPromoteMut.isPending ||
                  promoteMut.isPending ||
                  !!activeTrainJobId
                }
                onClick={() => autoPromoteMut.mutate()}
              >
                {autoPromoteMut.isPending
                  ? t('promote.btn.auto_promoting')
                  : t('promote.btn.auto_promote')}
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
            // v0.21c — per-trial Promote buttons. The
            // promoteMut now accepts an optional trial_index;
            // we pass the row's index to bulk-promote that
            // specific trial. The button on the "best" row
            // (rendered with `isBest=true` inside the component)
            // shows "Promote best" for clarity.
            onPromote={(trialIndex) => promoteMut.mutate(trialIndex)}
            promotingTrialIndex={
              // promoteMut.variables is the trial_index
              // passed to mutate(); null when not pending.
              // Type-cast to number|null since variables
              // is unknown by default.
              promoteMut.isPending && typeof promoteMut.variables === 'number'
                ? promoteMut.variables
                : null
            }
            // v0.25b — bulk promote all 4. One click
            // promotes all 4 trials as separate versions
            // in the history panel. The "Promote all 4"
            // button appears at the bottom of the train
            // progress panel (TrainProgress component).
            onPromoteAll={() => promoteAllMut.mutate()}
            promotingAll={promoteAllMut.isPending}
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
          that).
          v0.20c — passes the active model version so
          the row can be marked as "active" and the
          Rollback button can be hidden for the active
          row (you can't roll back to the active
          model). */}
      {/* v0.22b — Brier over time sparkline. Sits above
          the per-row PromoteHistory panel. The two share
          the same react-query key so they load together
          (no duplicate fetch). The chart gives the user
          a glance-level view ("trending down = good");
          the panel below provides per-row detail. */}
      <Card
        title={t('promote.chart.title')}
        description=""
      >
        <PromoteHistoryChart />
      </Card>

      <Card
        title={t('promote.history.title')}
        description={t('promote.history.desc')}
      >
        <PromoteHistory
          activeModelVersion={activeModel.data?.model_version ?? null}
          selectedForCompare={selectedForCompare}
          onSelectionChange={setSelectedForCompare}
        />
        {/* v0.34a — "View archive" + v0.40b "Compare (N)"
            buttons. The Compare button is enabled when
            2-3 entries are selected; clicking opens
            the ModelComparison modal. */}
        <div className="mt-2 flex items-center justify-end gap-2">
          {selectedForCompare.size >= 2 && (
            <Button
              size="sm"
              variant="secondary"
              iconLeft={<GitCompare className="w-3 h-3" />}
              onClick={() => setCompareOpen(true)}
              data-testid="compare-models-btn"
            >
              {t('compare.button', { count: selectedForCompare.size })}
            </Button>
          )}
          <Button
            size="sm"
            variant="ghost"
            iconLeft={<Archive className="w-3 h-3" />}
            onClick={() => setArchiveOpen(true)}
            data-testid="view-archive-btn"
          >
            {t('promote.archive.title')}
          </Button>
        </div>
      </Card>

      {/* v0.34a — archive modal. Renders only when
          archiveOpen is true; the component itself
          handles the open prop. The modal is mounted
          at the page level so the trigger button can
          be inside a Card without z-index issues. */}
      <PromoteHistoryArchive
        open={archiveOpen}
        onClose={() => setArchiveOpen(false)}
      />

      {/* v0.40b — multi-model comparison modal. We
          use a separate useQuery (historyQuery) to
          fetch the data when the modal opens. We
          pass the selected entries (matched by
          job_id) to the modal. */}
      <ModelComparison
        open={compareOpen}
        onClose={() => setCompareOpen(false)}
        entries={(() => {
          const allEntries = historyQuery.data?.entries ?? [];
          // Match the user's selection (job_ids) and
          // preserve the in-memory order (oldest first
          // from the Python side; reverse for the
          // modal — newer at the top, matching the
          // panel display).
          const reversed = [...allEntries].reverse();
          return reversed.filter((e) => selectedForCompare.has(e.job_id));
        })()}
        // v0.42e-3 — pass the archive-weights map so
        // the modal can show w0/w1/w2 alongside the
        // best_params it already shows. The map is
        // keyed by job_id; entries without an archive
        // match fall back to "(no weights)" — typical
        // for in-memory entries that haven't fallen
        // off the 20-cap yet, but those also won't
        // have weights until they DO fall off.
        weightsByJobId={(() => {
          const map = new Map<
            string,
            { w0: number; w1: number; w2: number }
          >();
          for (const e of weightsQuery.data?.entries ?? []) {
            if (e.weights) map.set(e.job_id, e.weights);
          }
          return map;
        })()}
        weightsLoading={weightsQuery.isLoading}
      />

      {/* v0.43c — backtest modal. Opens when the
          user clicks the "Backtest" button (visible
          when exactly one entry is selected in
          PromoteHistory). The target job_id is
          looked up against the in-memory history
          inside the modal. */}
      <BacktestReport
        open={backtestTarget !== null}
        onClose={() => setBacktestTarget(null)}
        targetJobId={backtestTarget}
      />

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
