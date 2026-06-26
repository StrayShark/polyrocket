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
// v0.91 — 将 train-progress 状态机从 ModelLab
// 抽离出来，以便此前被推迟的 v0.83 分支覆盖率
// 缺口现在可以独立测试。
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

  // v0.65a — 将 error 提前返回移到后续的 useQuery /
  // useState / useEffect hooks 之后。原本的提前返回
  // 位于第 66 行（紧跟在第一个 useQuery 之后），但后面
  // 还有更多 useQuery / useState 调用，因此 error 后
  // 重新渲染时触发的 hooks 数量更少，React 警告
  // 「Rendered fewer hooks than expected」。对实际用户
  // 的影响很小（下次渲染时页面也会卸载该损坏状态），
  // 但这违反 hooks 规则，会掩盖未来的 bug。

  const total = data?.length ?? 0;
  const best = data && data.length > 0
    ? data.reduce((acc, p) => (p.brier_score < acc.brier_score ? p : acc))
    : null;

  // v0.12d — 「当前活动」model。我们使用一个廉价的 predict
  // （m1/m2）调用 sidecar，以暴露未来 predict 调用将使用的
  // model_version。
  // v0.13b — 同时返回 brier score 用于 tooltip。
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

  // v0.17d — model training 状态。与 v0.15c Analysis 页相同的
  // 「鸡生蛋」模式：Rust 的 `train_job` IPC 在返回前就发出
  // `train_job:started`，因此无法从 IPC 返回值获取 job_id。
  // v0.91 — train progress 状态机抽取到 useTrainProgress hook
  // （src/hooks/useTrainProgress.ts）。该 hook 封装了
  // activeTrainJobId 状态、expectedTrainRef（用于竞态保护），
  // 以及带有 strict-mode 双挂载清理的 train:started
  // 事件监听器订阅。
  //
  // 调用方模式：在 trainMut.mutate() 之前调用 markExpected()；
  // 下一个 train:started 事件将填充 activeTrainJobId。
  const { activeTrainJobId, markExpected, clearActive } = useTrainProgress({
    onTrainStarted,
  });
  // v0.34a — archive modal 打开状态。本地状态，
  // 不持久化。重新挂载时重置为 false。
  const [archiveOpen, setArchiveOpen] = useState(false);
  // v0.40b — comparison modal 打开状态。
  const [compareOpen, setCompareOpen] = useState(false);
  // v0.40b — 用于比较的选中条目。job_id 集合。
  // 最多选 3 个；若用户选第 4 个，则丢弃最早的一个。
  const [selectedForCompare, setSelectedForCompare] = useState<Set<string>>(new Set());
  // v0.43c — backtest 的单选。backtest 按 model 选择：
  // 选一个条目，点击「Backtest」，
  // modal 打开并预填该 model_version。
  const [backtestTarget, setBacktestTarget] = useState<string | null>(null);

  // v0.28c — 监听 `auto_promote:finished` 事件。
  // 当后台 auto-promote 完成时（因为 Settings 中开启了
  // `autoPromoteAfterTrain`），Rust 端会发出该事件。我们：
  //   1. 使展示 active model 和 history 面板的查询失效
  //      （以便重新拉取）。
  //   2. 显示带结果的 toast，让用户知道他们的新 model
  //      是否被自动提升。
  //   3. v0.39b — 如果 `autoPromoteNotify` 首选项开启，
  //      还会发送一条 OS 通知（作为 toast 的补充）。
  //
  // 监听器在挂载时注册一次，并在 ModelLab 页面生命周期内
  // 保持活跃。如果用户离开后再回来，会重新注册。
  // v0.39b — 在挂载时读取一次 auto-promote-notify 标志
  // （监听器也只注册一次；会话中切换标志会在下次挂载时
  // 生效）。
  const autoPromoteNotify = usePrefsStore((s) => s.autoPromoteNotify);
  // v0.42e-2 — 「skipped」分支的可选 OS 通知。
  // 默认 false（大多数用户不希望每次训练都收到
  // 「训练未带来改进」的提示）。开启时，监听器会
  // 在 `promoted === false` 事件上发送 OS 通知，
  // 内容为 sidecar 的「reason」（例如
  // 「candidate not better than active」）。
  const autoPromoteSkippedNotify = usePrefsStore(
    (s) => s.autoPromoteSkippedNotify,
  );
  // v0.40b — comparison modal 需要完整的 history 数据。
  // 我们使用单独的 useQuery，key 与 PromoteHistory 面板
  // 相同，因此 react-query 会去重并共享缓存。
  const historyQuery = useQuery({
    queryKey: ['promote-history'],
    queryFn: () => listPromoteHistory(),
    staleTime: 30_000,
    enabled: compareOpen,
  });
  // v0.42e-3 — 从 archive 中拉取 2-3 个选中条目的
  // weights。内存中的 history 包含 best_params（lr、reg）
  // 但不包含 weights（w0、w1、w2）—— 它们只存储在
  // sidecar 的 archive.jsonl 中。我们只拉取所选的
  // job_ids（在 Rust 端进行白名单过滤），因此即使
  // archive 很大，调用也是有限的。
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
    // v0.39b — 挂载时请求一次通知权限。如果用户授权，
    // 每次 auto-promote 完成都会触发 OS 通知。
    // 如果拒绝，in-app toast 仍然有效。
    requestNotificationPermission().catch(() => {
      // No-op：best-effort。L1 仅回退到 in-app toast。
    });
    const unsub = onAutoPromoteFinished((e) => {
      if (cancelled) return;
      // 刷新所有依赖 active model 的内容：
      // KPI 卡片、history 面板、Brier 图表。
      queryClient.invalidateQueries({ queryKey: ['llm-performance'] });
      queryClient.invalidateQueries({ queryKey: ['sidecar-active-model'] });
      queryClient.invalidateQueries({ queryKey: ['promote-history'] });
      if (e.promoted) {
        toast.success(
          t('auto_promote.toast.auto_promoted'),
          e.model_version ?? undefined,
        );
        // v0.39b — 同时发送真正的 OS 通知，
        // 即使用户在另一个应用中也能知道。
        // 「skipped」事件不会触发（这是正常情况，
        // 用户通常仍在 ModelLab 页面）。
        if (autoPromoteNotify) {
          sendNotification(
            'auto_promote',
            t('auto_promote.toast.auto_promoted'),
            e.model_version ?? t('auto_promote.toast.auto_promoted_body'),
          ).catch(() => {
            // Best-effort：in-app toast 已触发，
            // 因此用户已收到反馈。
          });
        }
      } else {
        // 不要为「skipped」显示 error toast ——
        // 这属于 candidate 未更好的正常情况。
        // 仅显示 info 级别的 toast 以便可见。
        toast.info(t('auto_promote.toast.auto_skipped'), e.message);
        // v0.42e-2 — skipped 分支的可选 OS 通知。
        // 默认关闭（大多数用户不希望每次训练
        // 都收到「无改进」的提示）。开启时，
        // 与 promoted 路径的 OS 通知流程一致。
        if (autoPromoteSkippedNotify) {
          sendNotification(
            'auto_promote',
            t('auto_promote.toast.auto_skipped'),
            e.message || t('auto_promote.toast.auto_skipped_body'),
          ).catch(() => {
            // 尽力而为
          });
        }
      }
    });
    return () => {
      cancelled = true;
      unsub.then((u) => u()).catch(() => { /* 忽略 */ });
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const trainMut = useMutation({
    mutationFn: () => trainJob({}),
    onSuccess: (r) => {
      // IPC 在 `finished` 事件触发之后才返回。
      // TrainProgress 面板已显示最终状态。我们
      // 只需 toast 显示摘要、记录 candidate 以便
      // Promote 按钮使用，并刷新依赖查询。
      if (r.status === 'completed') {
        toast.success(
          t('train.toast.completed'),
          r.best_brier != null
            ? t('train.toast.brier', { value: r.best_brier.toFixed(3) })
            : '',
        );
        // v0.18c — 记录 candidate，使用户无需重新
        // 输入 job_id 即可 promote。
        setLastCandidate({
          jobId: r.job_id,
          candidatePath: r.candidate_path,
          bestBrier: r.best_brier,
        });
      } else {
        toast.error(t('train.toast.failed'), r.message ?? undefined);
        setLastCandidate(null);
      }
      // v0.91 — 使用 hook 的 clearActive() 而非直接 setState。
      clearActive();
      queryClient.invalidateQueries({ queryKey: ['llm-performance'] });
      queryClient.invalidateQueries({ queryKey: ['sidecar-active-model'] });
    },
    onError: (e: Error) => {
      toast.error(t('train.toast.failed'), e.message);
      // v0.91 — 使用 hook 的 clearActive() 而非直接 setState。
      clearActive();
    },
  });

  // v0.18c — 记录最近一次成功的 train，以便 Promote
  // 按钮传递正确的 `job_id`（竞态保护：Python sidecar
  // 拒绝 promote 来自其他 job 的 candidate，因此我们
  // 在 train 时捕获 job_id 并传递给 promote，确保
  // promote 的是我们刚刚训练的 model，而不是某个较早的）。
  const [lastCandidate, setLastCandidate] = useState<{
    jobId: string;
    candidatePath: string | null;
    bestBrier: number | null;
  } | null>(null);

  const promoteMut = useMutation({
    // v0.21c — bulk promote：接受可选 trial_index。
    // variables 参数是 trial_index（或者未定义表示
    // 默认「promote the best」行为）。
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
      // 刷新 active-model 探针，使 ModelVersionPill
      // 更新到新版本。
      queryClient.invalidateQueries({ queryKey: ['sidecar-active-model'] });
      queryClient.invalidateQueries({ queryKey: ['llm-performance'] });
      // v0.19c — 刷新 promote-history 面板，
      // 使新条目出现在顶部。
      queryClient.invalidateQueries({ queryKey: ['promote-history'] });
    },
    onError: (e: Error) => {
      toast.error(t('promote.toast.failed'), e.message);
    },
  });

  // v0.23b — 若更优则自动 promote。一键操作：
  // Python sidecar 比较 Brier score，要么 promote
  // （如果 candidate 至少优于 margin），要么以
  // 明确的「skipped」原因 no-op。
  // v0.23c — margin 现在可通过 Settings 配置（默认 0.005）。
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
        // 刷新 active-model 探针，使 ModelVersionPill
        // 更新到新版本。
        queryClient.invalidateQueries({ queryKey: ['sidecar-active-model'] });
        queryClient.invalidateQueries({ queryKey: ['llm-performance'] });
        queryClient.invalidateQueries({ queryKey: ['promote-history'] });
      } else if (r.skipped) {
        // v0.23b — skipped：candidate 没有显著更优。
        // reason 字段包含对比数字；我们显示一个带
        // delta 和 margin 的简短 toast。
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

  // v0.25b — 批量 promote 全部 4 个 trial。单击即可
  // 将 candidate 中的每个 trial promote 为 history 面板中
  // 的独立版本。便于 A/B 对比：用户可查看全部 4 个 trial
  // 在真实市场的表现，然后通过 v0.20c Rollback 按钮
  // 回滚至胜出者。
  const promoteAllMut = useMutation({
    mutationFn: () => promoteAllTrials(),
    onSuccess: (r) => {
      if (r.ok) {
        // 全部 4 个已 promote
        toast.success(t('promote.toast.all_promoted', { count: r.count }));
        setLastCandidate(null);
      } else {
        // 部分成功：部分 promote、部分失败
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
      // 刷新所有依赖 active model 的内容
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
                  <div className="font-mono text-body-sm text-fg">{p.model_version}</div>
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
                // v0.17d — 与 v0.15c 相同的模式：train_job
                // IPC 在返回前发出 `started`，因此我们监听
                // 下一个 `started` 事件并捕获 id。该标志确保
                // 仅捕获由本次点击触发的事件。
                // v0.91 — 使用 hook 的 markExpected() 而非
                // 直接修改 ref。
                markExpected();
                trainMut.mutate();
              }}
            >
              {trainMut.isPending ? t('modellab.runs.training') : t('modellab.runs.train')}
            </Button>
            {/* v0.18c — Promote 按钮。仅在最近一次 train
                有成功的 candidate（lastCandidate 非 null）
                时启用。按钮传递 candidate 的 job_id，使
                Python sidecar 拒绝 promote 不同 job
                （竞态保护）。 */}
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
            {/* v0.23b — 若更优则自动 promote。一键操作：
                Python sidecar 将 candidate 的 brier 与
                active 的 brier 比较，要么 promote
                （若 candidate 至少优于默认 0.005），
                要么以明确的「skipped」原因 no-op。 */}
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
        {/* v0.17d — 实时进度面板。订阅 2 个 `train_job:*`
            事件。当用户点击 Train 且下一个 `started`
            事件触发时挂载。 */}
        {activeTrainJobId && (
          <TrainProgress
            key={activeTrainJobId}
            jobId={activeTrainJobId}
            className="mt-0"
            // v0.21c — per-trial Promote 按钮。
            // promoteMut 现在接受可选 trial_index；
            // 我们传入该行的 index 以批量 promote
            // 该特定 trial。「best」行上的按钮
            // （组件内部以 `isBest=true` 渲染）
            // 显示「Promote best」以保持清晰。
            onPromote={(trialIndex) => promoteMut.mutate(trialIndex)}
            promotingTrialIndex={
              // promoteMut.variables 是传入 mutate()
              // 的 trial_index；未 pending 时为 null。
              // 类型转换为 number|null，因为 variables
              // 默认为 unknown。
              promoteMut.isPending && typeof promoteMut.variables === 'number'
                ? promoteMut.variables
                : null
            }
            // v0.25b — 批量 promote 全部 4 个。单击即可
            // 将 4 个 trial 作为 history 面板中的独立版本
            // 全部 promote。「Promote all 4」按钮显示在
            // train 进度面板（TrainProgress 组件）底部。
            onPromoteAll={() => promoteAllMut.mutate()}
            promotingAll={promoteAllMut.isPending}
          />
        )}
        {/* v0.18c — Last-candidate 提示。当用户有成功的
            train 尚未 promote 时显示。告知用户即将
            promote 的内容。 */}
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

      {/* v0.19c — Promotion history。展示最近 20 个已 promote 的
          model，最新优先。当前 active model 不在该列表中
          （请使用页面顶部的 ModelVersionPill）。
          v0.20c — 传入 active model version，以便将该行标记
          为「active」，并隐藏 active 行的 Rollback 按钮
          （无法回滚到 active model）。 */}
      {/* v0.22b — Brier 随时间变化的 sparkline。位于
          per-row PromoteHistory 面板上方。两者共享同一个
          react-query key，因此一起加载（无重复拉取）。
          图表为用户提供概览（「下降趋势 = 好」）；
          下方面板提供 per-row 详情。 */}
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
        {/* v0.34a — 「View archive」 + v0.40b 「Compare (N)」
            按钮。当选中 2-3 个条目时 Compare 按钮启用；
            点击打开 ModelComparison modal。 */}
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

      {/* v0.34a — archive modal。仅当 archiveOpen 为 true 时
          渲染；组件本身处理 open prop。Modal 在页面级别
          挂载，因此触发按钮可以在 Card 内而不会出现
          z-index 问题。 */}
      <PromoteHistoryArchive
        open={archiveOpen}
        onClose={() => setArchiveOpen(false)}
      />

      {/* v0.40b — multi-model comparison modal。我们使用
          单独的 useQuery（historyQuery）在 modal 打开时
          拉取数据。将选中的条目（按 job_id 匹配）传给
          modal。 */}
      <ModelComparison
        open={compareOpen}
        onClose={() => setCompareOpen(false)}
        entries={(() => {
          const allEntries = historyQuery.data?.entries ?? [];
          // 匹配用户的选择（job_ids）并保留内存中
          // 的顺序（Python 端为最旧优先；为 modal
          // 反转 —— 最新在顶部，与面板显示一致）。
          const reversed = [...allEntries].reverse();
          return reversed.filter((e) => selectedForCompare.has(e.job_id));
        })()}
        // v0.42e-3 — 传入 archive-weights map，使 modal
        // 能在已展示的 best_params 之外显示 w0/w1/w2。
        // 该 map 以 job_id 为 key；没有 archive 匹配
        // 的条目回退为「(no weights)」—— 这对尚未从
        // 20 上限淘汰的内存中条目很常见，但这些条目
        // 在真正被淘汰之前也不会有 weights。
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

      {/* v0.43c — backtest modal。当用户点击「Backtest」
          按钮（在 PromoteHistory 中恰好选中一个条目时
          可见）时打开。目标 job_id 在 modal 内部与
          内存 history 中查找。 */}
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
      <div className="text-xs text-muted font-semibold uppercase tracking-caption-uppercase">{label}</div>
      <div className={'font-mono text-[12px] ' + (positive ? 'text-bull' : 'text-fg')}>
        {value}
      </div>
    </div>
  );
}
