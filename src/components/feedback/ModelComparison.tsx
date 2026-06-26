/**
 * v0.40a —— 多 model 对比 modal。
 *
 * 让用户并排比较 promote history 中的 2-3 个 model 版本,
 * 展示:
 *   - model_version
 *   - promoted_at(相对时间)
 *   - best_brier(带颜色)
 *   - trial 徽章(best / trial N)
 *   - best_params(lr、reg)—— 来自内存 history
 *   - weights(w0、w1、w2)—— v0.42e-3,来自 archive
 *
 * 用户在 PromoteHistory 面板(通过 checkbox)选择若干 entry,
 * 然后点击 "Compare (N)" 打开本 modal。
 *
 * "Brier 最低者胜出" 的约定:best_brier 最小的行
 * 作为最佳高亮(绿色边框)。这与 PromoteHistory
 * 中实时 Brier 徽章的约定一致
 * (绿色 < 0.15,黄色 0.15-0.20,红色 > 0.20)。
 *
 * v0.42e-3 —— 父组件传入 `weightsByJobId` 映射
 * (来自 sidecar 的 archive.jsonl,通过
 * listPromoteHistoryArchive 获取)。映射以
 * job_id 为 key;未匹配到的 entry(通常是因为
 * 它们还没有从 20-cap 中淘汰,没有 archive 记录)
 * 显示 "(no weights)",archive 查询进行中
 * 时会显示小 spinner。
 */
import { Trophy, X } from 'lucide-react';
import { Modal } from '@/components/feedback/Modal';
import { BadgePill } from '@/components/base/BadgePill';
import { fmtRelativeTime } from '@/lib/format';
import { useT } from '@/lib/i18n';
import type { PromoteHistoryEntry } from '@/ipc';

interface ModelComparisonProps {
  /** modal 是否打开。 */
  open: boolean;
  /** 用户关闭 modal 时的回调。 */
  onClose: () => void;
  /** 待比较的 2-3 个 entry。调用方负责将数量限制在 2-3。 */
  entries: PromoteHistoryEntry[];
  /** v0.42e-3 —— 每个 entry 的权重(w0、w1、w2),来自
   * archive。可选;缺失的 entry 显示 "(no weights)"
   * 或 loading 态。 */
  weightsByJobId?: Map<string, { w0: number; w1: number; w2: number }>;
  /** v0.42e-3 —— archive 查询进行中时为 true。
   * 当为 true 且某 entry 暂未获得权重时,展示小 spinner。 */
  weightsLoading?: boolean;
}

function brierColor(brier: number | null): 'bull' | 'warn' | 'bear' | 'muted' {
  if (brier === null) return 'muted';
  if (brier < 0.15) return 'bull';
  if (brier < 0.2) return 'warn';
  return 'bear';
}

export function ModelComparison({
  open,
  onClose,
  entries,
  weightsByJobId,
  weightsLoading,
}: ModelComparisonProps) {
  const { t } = useT();
  // 找出 Brier 最低的 entry(best)。
  // 用来高亮"获胜者"。
  const best = entries.reduce<PromoteHistoryEntry | null>((acc, e) => {
    if (e.best_brier === null) return acc;
    if (acc === null) return e;
    if (e.best_brier < acc.best_brier!) return e;
    return acc;
  }, null);

  return (
    <Modal
      open={open}
      onClose={onClose}
      title={
        <div className="flex items-center gap-2">
          <Trophy className="w-4 h-4" />
          <span>{t('compare.title')}</span>
        </div>
      }
      size="lg"
    >
      <div
        className="grid gap-2"
        style={{
          // CSS grid,N 个等宽列,N 即 entry 数量(2 或 3)
          gridTemplateColumns: `repeat(${entries.length}, minmax(0, 1fr))`,
        }}
        data-testid="model-comparison"
        data-count={entries.length}
      >
        {entries.map((e) => {
          const isBest = best !== null && e.job_id === best.job_id;
          const color = brierColor(e.best_brier);
          return (
            <div
              key={e.job_id}
              className={`rounded-md border p-2.5 ${
                isBest ? 'border-bull bg-bull/5' : 'border-border bg-surface-2'
              }`}
              data-testid="model-comparison-col"
              data-job-id={e.job_id}
              data-best={isBest}
            >
              <div className="flex items-center gap-1.5 mb-2">
                <Trophy className={`w-3 h-3 text-${color}`} />
                <span className="text-xs text-muted font-semibold uppercase tracking-caption-uppercase">
                  {e.trial_index !== null && e.trial_index !== undefined
                    ? t('promote.history.trial_n', { n: e.trial_index + 1 })
                    : t('promote.history.trial_best')}
                </span>
                {isBest && (
                  <BadgePill
                    variant="bull"
                    data-testid="model-comparison-best"
                  >
                    ★ {t('compare.best')}
                  </BadgePill>
                )}
              </div>
              <div className="font-mono text-[11px] text-fg truncate mb-1">
                {e.model_version}
              </div>
              <div className="text-[10px] text-muted mb-1">
                {fmtRelativeTime(e.promoted_at_ms)}
              </div>
              <div className="space-y-0.5">
                <div className="text-[10px] text-muted">
                  <span className="inline-block w-12">Brier</span>
                  <span className={`font-mono text-${color}`}>
                    {e.best_brier !== null ? e.best_brier.toFixed(4) : '—'}
                  </span>
                </div>
                {/* v0.40a —— best_params(lr、reg) 在
                    内存 history 中可用;weights(w0、w1、w2)
                    仅在 archive(v0.33)中可用。对比中
                    这里展示 best_params,用户可通过
                    archive modal 查看完整权重。 */}
                {e.best_params ? (
                  <>
                    <div className="text-[10px] text-muted">
                      <span className="inline-block w-12">lr</span>
                      <span className="font-mono text-fg">
                        {String(e.best_params.lr ?? '—')}
                      </span>
                    </div>
                    <div className="text-[10px] text-muted">
                      <span className="inline-block w-12">reg</span>
                      <span className="font-mono text-fg">
                        {String(e.best_params.reg ?? '—')}
                      </span>
                    </div>
                  </>
                ) : (
                  <div className="text-[10px] text-muted italic">
                    (no params; pre-v0.18 entry)
                  </div>
                )}
                {/* v0.42e-3 —— 来自 archive 的
                    weights(w0、w1、w2)。通过带
                    job_ids 白名单的
                    listPromoteHistoryArchive 拉取
                    (结果由父组件传入)。缺失的 entry
                    是尚未从 20-cap 中淘汰的内存记录,
                    没有 archive 行,因此没有权重可展示。 */}
                <div
                  className="text-[10px] text-muted border-t border-border/40 pt-1 mt-1"
                  data-testid="model-comparison-weights"
                  data-job-id={e.job_id}
                >
                  <div className="text-xs text-muted font-semibold uppercase tracking-caption-uppercase mb-0.5">
                    {t('compare.weights_title')}
                  </div>
                  {weightsByJobId?.has(e.job_id) ? (
                    (() => {
                      const w = weightsByJobId.get(e.job_id)!;
                      return (
                        <div className="font-mono text-fg text-[10px]">
                          w0={w.w0.toFixed(3)} · w1={w.w1.toFixed(3)} · w2={w.w2.toFixed(3)}
                        </div>
                      );
                    })()
                  ) : weightsLoading ? (
                    <div className="text-[9px] text-muted italic">
                      {t('compare.weights_loading')}
                    </div>
                  ) : (
                    <div className="text-[9px] text-muted italic">
                      {t('compare.weights_missing')}
                    </div>
                  )}
                </div>
              </div>
            </div>
          );
        })}
      </div>
      {/* 底部的 Best-of 摘要 */}
      {best && (
        <div
          className="mt-3 p-2 rounded bg-bull/10 border border-bull text-[11px]"
          data-testid="model-comparison-summary"
        >
          <span className="text-bull font-semibold">
            {t('compare.lowest_brier')}:{' '}
          </span>
          <span className="font-mono">{best.model_version}</span>
          {' ('}
          <span className="font-mono">{best.best_brier?.toFixed(4)}</span>
          {')'}
        </div>
      )}
      <div className="mt-3 flex items-center justify-between">
        <span className="text-[10px] text-muted">
          {t('compare.hint')}
        </span>
        <button
          type="button"
          onClick={onClose}
          className="text-[10px] text-muted hover:text-fg inline-flex items-center gap-1"
          data-testid="model-comparison-close-btn"
        >
          <X className="w-3 h-3" />
          {t('common.close')}
        </button>
      </div>
    </Modal>
  );
}
