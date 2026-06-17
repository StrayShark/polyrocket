/**
 * TrainProgress (v0.17c).
 *
 * Renders the live status of an in-flight `train_job` IPC as
 * a per-trial table:
 *
 *   ⟳ Training… (2/4 trials)
 *   ┌─────┬──────┬───────┐
 *   │ lr  │ reg  │ brier │
 *   ├─────┼──────┼───────┤
 *   │0.05 │ 0.01 │ 0.184 │  ✓ best
 *   │0.10 │ 0.01 │ 0.210 │
 *   │0.05 │ 0.10 │ 0.225 │
 *   │0.10 │ 0.10 │ 0.243 │
 *   └─────┴──────┴───────┘
 *
 *   Best: 0.05 / 0.01 → w0=0.10 w1=0.20 w2=0.30
 *   candidate.json: /home/x/.polyrocket/sidecar/models/candidate.json
 *
 * Hooks into the 2 events emitted from
 * `commands::sidecar::train_job`:
 *
 *   started  → seeds a "training…" header (no trial data yet)
 *   finished → fills the per-trial table + best result
 *
 * Listeners are registered on mount, unregistered on unmount.
 * The component is keyed by `jobId` so multiple concurrent
 * trains stay independent — events for other job_ids are
 * ignored.
 *
 * After `finished`, the component keeps the final state
 * visible (doesn't auto-clear) so the user can see the
 * training result.
 */

import { useEffect, useState, useRef } from 'react';
import { CheckCircle2, XCircle, Loader2, Cpu, Sparkles } from 'lucide-react';
import {
  onTrainStarted,
  onTrainFinished,
  type TrainStartedEvent,
  type TrainFinishedEvent,
  type TrainTrialDto,
} from '@/ipc';
import { Pill } from '@/components/base/Pill';
import { useT } from '@/lib/i18n';

export interface TrainProgressProps {
  /** Server-generated UUID (`train-XXXXXXXX`). When set, the
   * component subscribes to events for this train only.
   * When null/undefined, the component is idle. */
  jobId: string | null | undefined;
  /** When true, force-show the panel even after the train
   * finished (e.g. for showing the last result on mount). */
  defaultExpanded?: boolean;
  className?: string;
}

export function TrainProgress({
  jobId,
  defaultExpanded = false,
  className = '',
}: TrainProgressProps) {
  const { t } = useT();
  const [started, setStarted] = useState<TrainStartedEvent | null>(null);
  const [finished, setFinished] = useState<TrainFinishedEvent | null>(null);
  const subscribedRef = useRef<string | null>(null);

  useEffect(() => {
    if (!jobId) {
      setStarted(null);
      setFinished(null);
      subscribedRef.current = null;
      return;
    }
    if (subscribedRef.current === jobId) return;
    // Reset for new train
    setStarted(null);
    setFinished(null);

    const unsubs: Array<() => void> = [];
    let cancelled = false;

    onTrainStarted((e) => {
      if (cancelled || e.job_id !== jobId) return;
      setStarted(e);
    }).then((un) => !cancelled && unsubs.push(un));

    onTrainFinished((e) => {
      if (cancelled || e.job_id !== jobId) return;
      setFinished(e);
    }).then((un) => !cancelled && unsubs.push(un));

    subscribedRef.current = jobId;

    return () => {
      cancelled = true;
      for (const u of unsubs) {
        try { u(); } catch { /* ignore */ }
      }
      if (subscribedRef.current === jobId) {
        subscribedRef.current = null;
      }
    };
  }, [jobId]);

  if (!jobId) return null;
  if (!started && !defaultExpanded) return null;
  if (!started && defaultExpanded && !finished) return null;

  const isRunning = !finished;
  const trials: TrainTrialDto[] = finished?.trials ?? [];
  const bestIdx = finished && finished.best_brier != null
    ? trials.reduce(
        (best, t, i) => (best === -1 || t.brier < trials[best].brier ? i : best),
        -1,
      )
    : -1;

  return (
    <div
      data-testid="train-progress"
      data-running={isRunning}
      data-job-id={jobId}
      data-status={finished?.status ?? 'running'}
      className={
        'mt-3 rounded-md border border-border bg-surface-2 p-3 ' + className
      }
    >
      {/* Header */}
      <div className="flex items-center justify-between mb-2">
        <div className="flex items-center gap-2">
          {isRunning ? (
            <Loader2 className="w-3.5 h-3.5 animate-spin text-accent" />
          ) : finished?.status === 'completed' ? (
            <CheckCircle2 className="w-3.5 h-3.5 text-bull" />
          ) : (
            <XCircle className="w-3.5 h-3.5 text-bear" />
          )}
          <span className="text-[12px] font-medium text-fg">
            {isRunning
              ? t('train.progress.running', { trials: started?.n_trials ?? '?' })
              : t('train.progress.done', {
                  status: t(`train.progress.status.${finished?.status ?? 'unknown'}` as 'train.progress.status.completed'),
                })}
          </span>
          {finished && (
            <Pill kind={finished.status === 'completed' ? 'bull' : 'bear'}>
              {t(`train.progress.status.${finished.status}` as 'train.progress.status.completed')}
            </Pill>
          )}
        </div>
        {finished && (
          <div className="text-[10px] text-muted">
            {t('train.progress.duration', { ms: finished.duration_ms })}
          </div>
        )}
      </div>

      {/* Failure message */}
      {finished?.status === 'failed' && finished.message && (
        <div
          data-testid="train-progress-error"
          className="text-[11px] text-bear mb-2 px-2 py-1 rounded bg-bear/10"
        >
          {finished.message}
        </div>
      )}

      {/* Trial table (only when we have at least 1 trial) */}
      {trials.length > 0 && (
        <div
          data-testid="train-progress-table"
          className="rounded border border-border overflow-hidden"
        >
          <table className="w-full text-[11px] font-mono">
            <thead>
              <tr className="bg-surface-3 text-muted">
                <th className="text-left px-2 py-1 font-medium">{t('train.progress.col.trial')}</th>
                <th className="text-right px-2 py-1 font-medium">{t('train.progress.col.lr')}</th>
                <th className="text-right px-2 py-1 font-medium">{t('train.progress.col.reg')}</th>
                <th className="text-right px-2 py-1 font-medium">{t('train.progress.col.brier')}</th>
                <th className="text-right px-2 py-1 font-medium w-12" />
              </tr>
            </thead>
            <tbody>
              {trials.map((t2, i) => {
                const isBest = i === bestIdx;
                return (
                  <tr
                    key={i}
                    data-testid={`train-progress-row-${i}`}
                    data-best={isBest ? 'true' : 'false'}
                    className={isBest ? 'bg-bull/5' : ''}
                  >
                    <td className="px-2 py-1 text-fg">{i + 1}</td>
                    <td className="px-2 py-1 text-right text-fg">{t2.lr.toFixed(2)}</td>
                    <td className="px-2 py-1 text-right text-fg">{t2.reg.toFixed(2)}</td>
                    <td className="px-2 py-1 text-right text-fg">{t2.brier.toFixed(3)}</td>
                    <td className="px-2 py-1 text-right">
                      {isBest && (
                        <span className="text-[9px] uppercase text-bull">best</span>
                      )}
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      )}

      {/* Best result footer (only on success) */}
      {finished?.status === 'completed' && finished.best_brier != null && (
        <div
          data-testid="train-progress-best"
          className="mt-2 text-[11px] text-fg space-y-1"
        >
          <div className="flex items-center gap-1.5">
            <Sparkles className="w-3 h-3 text-bull" />
            <span className="font-mono">
              {t('train.progress.best_brier', { value: finished.best_brier.toFixed(3) })}
            </span>
            {finished.best_params && (
              <span className="text-muted font-mono text-[10px]">
                w0={Number(finished.best_params.w0 ?? 0).toFixed(2)} ·{' '}
                w1={Number(finished.best_params.w1 ?? 0).toFixed(2)} ·{' '}
                w2={Number(finished.best_params.w2 ?? 0).toFixed(2)}
              </span>
            )}
          </div>
          {finished.candidate_path && (
            <div className="flex items-center gap-1.5 text-muted text-[10px]">
              <Cpu className="w-3 h-3" />
              <code className="font-mono">{finished.candidate_path}</code>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
