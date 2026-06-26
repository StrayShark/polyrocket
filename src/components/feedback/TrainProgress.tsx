/**
 * TrainProgress(v0.17c、v0.21c)。
 *
 * 将进行中的 `train_job` IPC 实时状态以
 * per-trial 表格形式渲染:
 *
 *   ⟳ Training… (2/4 trials)
 *   ┌─────┬──────┬───────┬─────────────────┐
 *   │ lr  │ reg  │ brier │                 │
 *   ├─────┼──────┼───────┼─────────────────┤
 *   │0.05 │ 0.01 │ 0.184 │ ✓ best  [Promote]│
 *   │0.10 │ 0.01 │ 0.210 │         [Promote]│
 *   │0.05 │ 0.10 │ 0.225 │         [Promote]│
 *   │0.10 │ 0.10 │ 0.243 │         [Promote]│
 *   └─────┴──────┴───────┴─────────────────┘
 *
 *   Best: 0.05 / 0.01 → w0=0.10 w1=0.20 w2=0.30
 *   candidate.json: /home/x/.polyrocket/sidecar/models/candidate.json
 *
 * v0.21c —— per-trial Promote 按钮。用户可 promote
 * 任意 trial(不仅是 best)。"best" trial 的按钮
 * 文字为 "Promote best" 以区分;其余为
 * "Promote #N"。点击后调 `onPromote` 回调,
 * 传入 trial index。
 *
 * 监听 `commands::sidecar::train_job` 发出的 2 个事件:
 *
 *   started  → 先填充 "training…" 头部(尚无 trial 数据)
 *   finished → 填充 per-trial 表格 + best 结果
 *
 * 监听器在 mount 时注册、unmount 时注销。
 * 组件按 `jobId` 作 key,多个并发训练相互独立
 * —— 其他 job_id 的事件会被忽略。
 *
 * `finished` 之后,组件保留最终状态(不自动清空),
 * 让用户能查看训练结果。
 */

import { useEffect, useState, useRef } from 'react';
import { CheckCircle2, XCircle, Loader2, Cpu, Sparkles, ArrowUpCircle } from 'lucide-react';
import {
  onTrainStarted,
  onTrainFinished,
  type TrainStartedEvent,
  type TrainFinishedEvent,
  type TrainTrialDto,
} from '@/ipc';
import { Pill } from '@/components/base/Pill';
import { Button } from '@/components/base/Button';
import { useT } from '@/lib/i18n';

export interface TrainProgressProps {
  /** 服务端生成的 UUID(`train-XXXXXXXX`)。设置时,
   * 组件只订阅该 train 的事件。
   * null/undefined 时,组件为 idle。 */
  jobId: string | null | undefined;
  /** 为 true 时,即使 train 已结束,也强制显示面板
   * (例如挂载时显示上一次的训练结果)。 */
  defaultExpanded?: boolean;
  className?: string;
  /** v0.21c —— 批量 promote。可选回调,用户点击某
   * trial 行的 "Promote" 按钮时触发。回调接收
   * 0-based 的 trial 编号。若 undefined,Promote
   * 按钮隐藏。 */
  onPromote?: (trialIndex: number) => void;
  /** v0.21c —— 当前正在 promote 的 trial(用于该行
   * 按钮的 loading 态)。 */
  promotingTrialIndex?: number | null;
  /** v0.25b —— 批量 promote 全部 4 个 trial。可选回调,
   * 点击 "Promote all 4" 时触发。undefined 时按钮隐藏。 */
  onPromoteAll?: () => void;
  /** v0.25b —— "Promote all 4" 按钮的 loading 态。 */
  promotingAll?: boolean;
}

export function TrainProgress({
  jobId,
  defaultExpanded = false,
  className = '',
  onPromote,
  promotingTrialIndex = null,
  onPromoteAll,
  promotingAll = false,
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
    // 为新 train 重置
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
        try { u(); } catch { /* 忽略 */ }
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
      {/* 标题 */}
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

      {/* 失败信息 */}
      {finished?.status === 'failed' && finished.message && (
        <div
          data-testid="train-progress-error"
          className="text-[11px] text-bear mb-2 px-2 py-1 rounded bg-bear/10"
        >
          {finished.message}
        </div>
      )}

      {/* 试验表（仅当至少有 1 次试验时）*/}
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
                <th className="text-right px-2 py-1 font-medium w-32" />
              </tr>
            </thead>
            <tbody>
              {trials.map((t2, i) => {
                const isBest = i === bestIdx;
                const isPromoting = promotingTrialIndex === i;
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
                      <div className="flex items-center justify-end gap-1.5">
                        {isBest && (
                          <span className="text-[9px] uppercase text-bull">best</span>
                        )}
                        {onPromote && (
                          <Button
                            data-testid={`train-promote-btn-${i}`}
                            variant="ghost"
                            size="sm"
                            iconLeft={<ArrowUpCircle className="w-3 h-3" />}
                            loading={isPromoting}
                            disabled={promotingTrialIndex !== null}
                            onClick={() => onPromote(i)}
                          >
                            {isBest
                              ? t('train.progress.promote_best')
                              : t('train.progress.promote_trial', { n: i + 1 })}
                          </Button>
                        )}
                      </div>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      )}

      {/* 最佳结果页脚（仅在成功时）*/}
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
          {/* v0.25b —— "Promote all 4" 按钮。位于
              train progress 面板底部,在 "best" footer
              下方。一键将全部 4 个 trial 作为
              独立版本 promote 到 history 面板,
              方便用户 A/B 对比。 */}
          {onPromoteAll && finished.trials.length > 1 && (
            <Button
              data-testid="train-promote-all-btn"
              variant="primary"
              size="sm"
              iconLeft={<ArrowUpCircle className="w-3 h-3" />}
              loading={promotingAll}
              disabled={promotingAll || promotingTrialIndex !== null}
              onClick={() => onPromoteAll()}
              className="mt-1"
            >
              {t('train.progress.promote_all')}
            </Button>
          )}
        </div>
      )}
    </div>
  );
}
