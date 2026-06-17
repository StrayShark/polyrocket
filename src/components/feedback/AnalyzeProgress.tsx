/**
 * AnalyzeProgress (v0.15c).
 *
 * Renders the live status of an in-flight `llm_analyze` IPC as a
 * per-provider status grid:
 *
 *   Anthropic    ✓  1.2s   $0.04
 *   OpenAI       ⏳ running…
 *   Google       ✗  rate_limit
 *   DeepSeek     ·  pending
 *
 * Hooks into the 4 events emitted from `commands::llm::llm_analyze`
 * (see `domain::llm::progress` + `ipc.ts::onAnalyzeStarted` etc.):
 *
 *   started        → seeds the initial "pending" grid
 *   provider_done   → updates the matching provider's status
 *   consensus_done  → updates the header (consensus preview)
 *   finished        → marks the overall run as done
 *
 * Listeners are registered on mount, unregistered on unmount. The
 * grid is keyed by `analysisId` so multiple concurrent analyzes
 * (e.g. across panels) stay independent — events for other
 * analysis_ids are ignored.
 *
 * After `finished`, the component keeps the final state visible
 * (doesn't auto-clear) so the user can see which provider failed
 * even after the analyze is done.
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
import { Pill } from '@/components/base/Pill';
import { fmtLatency, fmtCents } from '@/lib/format';
import { useT } from '@/lib/i18n';

type ProviderStatus =
  | { kind: 'pending' }
  | { kind: 'running' }
  | { kind: 'ok'; latency_ms: number; cost_cents: number }
  | { kind: 'failed'; error_kind: string; error_message: string | null; latency_ms: number };

export interface AnalyzeProgressProps {
  /** UUID string. When set, the component will subscribe to events
   * for this analysis only. When null/undefined, the component
   * is idle (renders nothing). */
  analysisId: string | null | undefined;
  /** Optional: when true, force-show the grid even after the
   * analyze finished (e.g. for showing the final state of the
   * last analyze on mount). Default false. */
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
      // Idle state — clear
      setStarted(null);
      setProviders({});
      setConsensus(null);
      setFinished(null);
      subscribedRef.current = null;
      return;
    }
    if (subscribedRef.current === analysisId) {
      // Already subscribed for this analysis
      return;
    }
    // Reset for new analysis
    setStarted(null);
    setProviders({});
    setConsensus(null);
    setFinished(null);

    const unsubs: Array<() => void> = [];
    let cancelled = false;

    onAnalyzeStarted((e) => {
      if (cancelled || e.analysis_id !== analysisId) return;
      setStarted(e);
      // Seed pending grid
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
        try { u(); } catch { /* ignore */ }
      }
      if (subscribedRef.current === analysisId) {
        subscribedRef.current = null;
      }
    };
  }, [analysisId]);

  // Don't render anything until we have an event for this analysis,
  // or if explicitly expanded with a finished result.
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
          <span className="text-[12px] font-medium text-fg">
            {isRunning
              ? t('analysis.progress.running', { done: nDone, total: providerList.length })
              : t('analysis.progress.done', {
                  ok: nOk,
                  failed: nFailed,
                  total: providerList.length,
                })}
          </span>
          {finished && (
            <Pill kind={finished.status === 'completed' ? 'bull' : finished.status === 'partial' ? 'warning' : 'bear'}>
              {t(`analysis.progress.status.${finished.status}` as 'analysis.progress.status.completed')}
            </Pill>
          )}
        </div>
        <div className="text-[10px] text-muted">
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
            className="flex items-center gap-2 text-[12px]"
          >
            {s.kind === 'pending' && <Clock className="w-3 h-3 text-muted shrink-0" />}
            {s.kind === 'running' && <Loader2 className="w-3 h-3 animate-spin text-accent shrink-0" />}
            {s.kind === 'ok' && <CheckCircle2 className="w-3 h-3 text-bull shrink-0" />}
            {s.kind === 'failed' && <XCircle className="w-3 h-3 text-bear shrink-0" />}
            <span className="font-mono text-fg flex-1 truncate">{id}</span>
            <span className="text-muted text-[10px] shrink-0">
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
