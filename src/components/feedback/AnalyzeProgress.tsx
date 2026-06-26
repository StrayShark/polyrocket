/**
 * AnalyzeProgress (v0.15c)。
 *
 * 将一个进行中的 `llm_analyze` IPC 实时状态渲染为按 provider
 * 排列的状态网格:
 *
 *   Anthropic    ✓  1.2s   $0.04
 *   OpenAI       ⏳ running…
 *   Google       ✗  rate_limit
 *   DeepSeek     ·  pending
 *
 * 监听 `commands::llm::llm_analyze` 发出的 4 类事件
 * (见 `domain::llm::progress` 与 `ipc.ts::onAnalyzeStarted` 等):
 *
 *   started        → 初始化 "pending" 网格
 *   provider_done   → 更新对应 provider 的状态
 *   consensus_done  → 更新头部(consensus 预览)
 *   finished        → 标记整体运行结束
 *
 * 监听器在挂载时注册,卸载时注销。网格以 `analysisId`
 * 为 key,保证多个并发分析(例如跨面板)互相独立——
 * 其他 analysis_id 的事件会被忽略。
 *
 * `finished` 之后,组件保留最终状态(不会自动清除),
 * 方便用户在分析完成后也能看到哪个 provider 失败。
 */

import { useEffect, useState, useRef } from 'react';
import { CheckCircle2, XCircle, Loader2, Clock, Sparkles } from 'lucide-react';
import {
  onAnalyzeStarted,
  onProviderDone,
  onConsensusDone,
  onAnalyzeFinished,
  type AnalyzeStartedEvent,
  type ConsensusDoneEvent,
  type AnalyzeFinishedEvent,
} from '@/ipc';
import { BadgePill } from '@/components/base/BadgePill';
import { TimelinePill } from '@/components/feedback/TimelinePill';
import { fmtLatency, fmtCents } from '@/lib/format';
import { useT } from '@/lib/i18n';

type ProviderStatus =
  | { kind: 'pending' }
  | { kind: 'running' }
  | { kind: 'ok'; latency_ms: number; cost_cents: number }
  | { kind: 'failed'; error_kind: string; error_message: string | null; latency_ms: number };

export interface AnalyzeProgressProps {
  /** UUID 字符串。设置后组件将只订阅此 analysis 的事件;
   * 为 null/undefined 时组件处于空闲态(不渲染任何内容)。 */
  analysisId: string | null | undefined;
  /** 可选:为 true 时,即便分析已完成也强制展示网格
   * (例如在挂载时显示上次分析的最终状态)。默认 false。 */
  defaultExpanded?: boolean;
  className?: string;
}

export function AnalyzeProgress({
  analysisId,
  defaultExpanded = false,
  className = '',
}: AnalyzeProgressProps) {
  const { t } = useT();
  const [started, setStarted] = useState<AnalyzeStartedEvent | null>(null);
  const [providers, setProviders] = useState<Record<string, ProviderStatus>>({});
  const [consensus, setConsensus] = useState<ConsensusDoneEvent | null>(null);
  const [finished, setFinished] = useState<AnalyzeFinishedEvent | null>(null);
  const subscribedRef = useRef<string | null>(null);

  useEffect(() => {
    if (!analysisId) {
      // 空闲态——清空
      setStarted(null);
      setProviders({});
      setConsensus(null);
      setFinished(null);
      subscribedRef.current = null;
      return;
    }
    if (subscribedRef.current === analysisId) {
      // 已订阅此 analysis
      return;
    }
    // 为新 analysis 重置
    setStarted(null);
    setProviders({});
    setConsensus(null);
    setFinished(null);

    const unsubs: Array<() => void> = [];
    let cancelled = false;

    onAnalyzeStarted((e) => {
      if (cancelled || e.analysis_id !== analysisId) return;
      setStarted(e);
      // 初始化 pending 网格
      const seed: Record<string, ProviderStatus> = {};
      for (const id of e.providers) seed[id] = { kind: 'pending' };
      setProviders(seed);
    }).then((un) => !cancelled && unsubs.push(un));

    onProviderDone((e) => {
      if (cancelled || e.analysis_id !== analysisId) return;
      setProviders((prev) => ({
        ...prev,
        [e.provider_id]: e.ok
          ? { kind: 'ok', latency_ms: e.latency_ms, cost_cents: e.cost_cents }
          : {
              kind: 'failed',
              error_kind: e.error_kind,
              error_message: e.error_message,
              latency_ms: e.latency_ms,
            },
      }));
    }).then((un) => !cancelled && unsubs.push(un));

    onConsensusDone((e) => {
      if (cancelled || e.analysis_id !== analysisId) return;
      setConsensus(e);
    }).then((un) => !cancelled && unsubs.push(un));

    onAnalyzeFinished((e) => {
      if (cancelled || e.analysis_id !== analysisId) return;
      setFinished(e);
    }).then((un) => !cancelled && unsubs.push(un));

    subscribedRef.current = analysisId;

    return () => {
      cancelled = true;
      for (const u of unsubs) {
        try { u(); } catch { /* 忽略 */ }
      }
      if (subscribedRef.current === analysisId) {
        subscribedRef.current = null;
      }
    };
  }, [analysisId]);

  // 在收到本 analysis 的事件(或带有 finished 结果且显式展开)之前不渲染任何内容。
  if (!analysisId) return null;
  if (!started && !defaultExpanded) return null;
  if (!started && defaultExpanded && !finished) return null;

  const providerList = Object.entries(providers);
  const isRunning = !finished;
  const totalLatency = finished?.total_latency_ms ?? 0;
  const totalCost = finished?.total_cost_cents ?? 0;
  const nDone = providerList.filter(([, s]) => s.kind === 'ok' || s.kind === 'failed').length;
  const nOk = providerList.filter(([, s]) => s.kind === 'ok').length;
  const nFailed = providerList.filter(([, s]) => s.kind === 'failed').length;

  // v0.119 — 分析头部的 Cursor TimelinePill 映射。
  // 将整个 LLM 分析流程映射到 5 种 Cursor pastel 阶段之一,
  // 让用户感知到 agent timeline 当前所处的阶段。
  //   - 未开始 / 全部 pending → "Thinking"(规划 query)
  //   - 任一 provider running 或进行中 → "Reading"(LLM 正在生成)
  //   - 所有 provider 结束,等待 consensus → "Editing"
  //   - 收到 finished 事件 → "Done"
  const nRunning = providerList.filter(([, s]) => s.kind === 'running').length;
  const allPending = providerList.length > 0 && nDone === 0 && nRunning === 0;
  const timelineStage = finished
    ? 'done'
    : !started || (isRunning && allPending)
      ? 'thinking'
      : isRunning && nRunning > 0
        ? 'read'
        : isRunning && nDone < providerList.length
          ? 'read'
          : isRunning
            ? 'edit'
            : 'done';

  return (
    <div
      data-testid="analyze-progress"
      data-running={isRunning}
      data-analysis-id={analysisId}
      className={
        'mt-3 rounded-md border border-border bg-surface-2 p-3 ' + className
      }
    >
      <div className="flex items-center justify-between mb-2">
        <div className="flex items-center gap-2">
          {isRunning ? (
            <Loader2 className="w-3.5 h-3.5 animate-spin text-accent" />
          ) : (
            <CheckCircle2 className="w-3.5 h-3.5 text-bull" />
          )}
          <span className="text-body-sm font-medium text-fg">
            {isRunning
              ? t('analysis.progress.running', { done: nDone, total: providerList.length })
              : t('analysis.progress.done', {
                  ok: nOk,
                  failed: nFailed,
                  total: providerList.length,
                })}
          </span>
          {/* v0.119 —— Cursor TimelinePill,展示整体 LLM 阶段。
              按 Cursor 规范,pastel 配色仅用于 timeline UI。 */}
          <TimelinePill stage={timelineStage} />
          {finished && (
            <BadgePill
              variant={
                finished.status === 'completed'
                  ? 'bull'
                  : finished.status === 'partial'
                    ? 'warning'
                    : 'bear'
              }
              data-testid="analyze-progress-status-pill"
            >
              {t(`analysis.progress.status.${finished.status}` as 'analysis.progress.status.completed')}
            </BadgePill>
          )}
        </div>
        <div className="text-[11px] text-muted">
          {isRunning ? '' : (
            <>
              {t('analysis.progress.total_latency', { ms: totalLatency })} ·
              {' '}{t('analysis.progress.total_cost', { cost: fmtCents(totalCost) })}
            </>
          )}
        </div>
      </div>

      {consensus && (
        <div className="mb-2 text-[11px] text-muted flex items-center gap-2">
          <Sparkles className="w-3 h-3" />
          <span>
            {t('analysis.progress.consensus', {
              pred: consensus.consensus_pred != null ? (consensus.consensus_pred * 100).toFixed(1) + '%' : '—',
              side: consensus.consensus_side ?? '—',
            })}
          </span>
        </div>
      )}

      <div className="space-y-1">
        {providerList.map(([id, s]) => (
          <div
            key={id}
            data-testid={`analyze-progress-row-${id}`}
            data-status={s.kind}
            className="flex items-center gap-2 text-body-sm"
          >
            {s.kind === 'pending' && <Clock className="w-3 h-3 text-muted shrink-0" />}
            {s.kind === 'running' && <Loader2 className="w-3 h-3 animate-spin text-accent shrink-0" />}
            {s.kind === 'ok' && <CheckCircle2 className="w-3 h-3 text-bull shrink-0" />}
            {s.kind === 'failed' && <XCircle className="w-3 h-3 text-bear shrink-0" />}
            <span className="font-mono text-fg flex-1 truncate">{id}</span>
            <span className="text-muted text-[11px] shrink-0">
              {s.kind === 'pending' && t('analysis.progress.row.pending')}
              {s.kind === 'running' && t('analysis.progress.row.running')}
              {s.kind === 'ok' && fmtLatency(s.latency_ms)}
              {s.kind === 'failed' && (
                <span data-testid={`analyze-progress-error-${id}`} title={s.error_message ?? s.error_kind}>
                  {s.error_kind}
                </span>
              )}
            </span>
          </div>
        ))}
      </div>
    </div>
  );
}
