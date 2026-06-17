/**
 * PromoteHistory — v0.19c.
 *
 * Read-only panel showing the list of past model
 * promotions, oldest first (capped at 20 on the
 * Python side). Each row shows:
 *
 *   - model_version  (e.g. "logistic-train-441c352b")
 *   - promoted_at    (relative: "3h ago" / "yesterday")
 *   - best_brier     (colored badge; green < 0.15,
 *                     yellow 0.15-0.20, red > 0.20)
 *
 * The currently active model is NOT in this list — to
 * see the active model, use the ModelVersionPill at
 * the top of the page. The list is for audit
 * ("which model was active at which time") not for
 * status display.
 *
 * Refreshes on:
 *   - mount (initial query)
 *   - the `promote_model` mutation succeeding (parent
 *     passes `refetchKey` to invalidate)
 */
import { useQuery } from '@tanstack/react-query';
import { History, Trophy } from 'lucide-react';
import { listPromoteHistory, type PromoteHistoryEntry } from '@/ipc';
import { fmtRelativeTime } from '@/lib/format';
import { useT } from '@/lib/i18n';
import { Skeleton } from '@/components/feedback/Skeleton';
import { ErrorState } from '@/components/feedback/ErrorState';
import { EmptyState } from '@/components/feedback/EmptyState';

interface PromoteHistoryProps {
  /** Optional className passthrough (for spacing). */
  className?: string;
}

function brierColor(brier: number | null): 'bull' | 'warn' | 'bear' | 'muted' {
  if (brier === null) return 'muted';
  if (brier < 0.15) return 'bull';
  if (brier < 0.2) return 'warn';
  return 'bear';
}

export function PromoteHistory({ className = '' }: PromoteHistoryProps) {
  const { t } = useT();
  const { data, isLoading, error, refetch } = useQuery({
    queryKey: ['promote-history'],
    queryFn: () => listPromoteHistory(),
    staleTime: 30_000,
  });

  if (isLoading) {
    return (
      <div className={`space-y-2 ${className}`} data-testid="promote-history-loading">
        {Array.from({ length: 3 }).map((_, i) => (
          <Skeleton key={i} className="h-9" />
        ))}
      </div>
    );
  }

  if (error) {
    return (
      <div className={className}>
        <ErrorState
          message={String(error)}
          onRetry={() => refetch()}
        />
      </div>
    );
  }

  const entries = data?.entries ?? [];
  const empty = entries.length === 0;
  // Reverse to show newest first (the array is oldest-first
  // from the Python side, which is the natural order for
  // an append-only log; the UI reverses for "recent first").
  const reversed = [...entries].reverse();

  if (empty) {
    return (
      <div className={className}>
        <EmptyState
          icon={<History className="w-5 h-5" />}
          title={t('promote.history.empty')}
          description={t('promote.history.empty_desc')}
        />
      </div>
    );
  }

  return (
    <div
      className={`space-y-2 ${className}`}
      data-testid="promote-history"
      data-count={entries.length}
    >
      {reversed.map((e) => (
        <HistoryRow key={`${e.job_id}-${e.promoted_at_ms}`} entry={e} />
      ))}
    </div>
  );
}

function HistoryRow({ entry }: { entry: PromoteHistoryEntry }) {
  const { t } = useT();
  const color = brierColor(entry.best_brier);
  return (
    <div
      className="rounded-md border border-border bg-surface-2 p-2.5 flex items-center gap-3"
      data-testid="promote-history-row"
      data-job-id={entry.job_id}
    >
      <Trophy className={`w-3.5 h-3.5 shrink-0 text-${color}`} />
      <div className="flex-1 min-w-0">
        <div className="font-mono text-[12px] text-fg truncate">
          {entry.model_version}
        </div>
        <div className="text-[10px] text-muted mt-0.5">
          {fmtRelativeTime(entry.promoted_at_ms)}
        </div>
      </div>
      <div className="text-right shrink-0">
        <div className="text-[10px] text-muted uppercase tracking-wide">
          {t('promote.history.brier')}
        </div>
        <div
          className={`font-mono text-[12px] text-${color}`}
          data-testid="promote-history-brier"
          data-brier={entry.best_brier ?? ''}
        >
          {entry.best_brier !== null ? entry.best_brier.toFixed(3) : '—'}
        </div>
      </div>
    </div>
  );
}
