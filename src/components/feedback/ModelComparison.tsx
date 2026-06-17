/**
 * v0.40a — Multi-model comparison modal.
 *
 * Lets the user compare 2-3 model versions from
 * the promote history side-by-side. Shows:
 *   - model_version
 *   - promoted_at (relative)
 *   - best_brier (colored)
 *   - trial badge (best / trial N)
 *   - weights (w0, w1, w2)
 *
 * The user selects entries in the PromoteHistory
 * panel (via checkboxes), then clicks "Compare
 * (N)" which opens this modal.
 *
 * The "lowest Brier wins" convention: the row
 * with the smallest `best_brier` is highlighted
 * as the best (green border). This is the same
 * convention as the live Brier badge in
 * PromoteHistory (green < 0.15, yellow 0.15-0.20,
 * red > 0.20).
 */
import { Trophy, X } from 'lucide-react';
import { Modal } from '@/components/feedback/Modal';
import { fmtRelativeTime } from '@/lib/format';
import { useT } from '@/lib/i18n';
import type { PromoteHistoryEntry } from '@/ipc';

interface ModelComparisonProps {
  /** Whether the modal is open. */
  open: boolean;
  /** Called when the user closes the modal. */
  onClose: () => void;
  /** The 2-3 entries to compare. The caller is
   *  responsible for limiting to 2-3. */
  entries: PromoteHistoryEntry[];
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
}: ModelComparisonProps) {
  const { t } = useT();
  // Find the entry with the lowest Brier (best).
  // We use this to highlight the "winner".
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
          // CSS grid with N equal columns where N is
          // the number of entries (2 or 3)
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
                <span className="text-[10px] uppercase tracking-wide text-muted">
                  {e.trial_index !== null && e.trial_index !== undefined
                    ? t('promote.history.trial_n', { n: e.trial_index + 1 })
                    : t('promote.history.trial_best')}
                </span>
                {isBest && (
                  <span
                    className="text-[9px] uppercase tracking-wide text-bull"
                    data-testid="model-comparison-best"
                  >
                    ★ {t('compare.best')}
                  </span>
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
                {/* v0.40a — best_params (lr, reg) is
                    available on the in-memory history;
                    weights (w0, w1, w2) are only on the
                    archive (v0.33). For the comparison
                    we show best_params here, and the
                    user can use the archive modal to see
                    full weights. */}
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
              </div>
            </div>
          );
        })}
      </div>
      {/* Best-of summary at the bottom */}
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
