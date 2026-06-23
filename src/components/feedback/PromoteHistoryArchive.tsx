/**
 * PromoteHistoryArchive — v0.34a.
 *
 * Modal that shows the FULL promote history (not just
 * the in-memory last 20). Reads from the Python
 * sidecar's `archive.jsonl` via the v0.33b
 * `list_promote_history_archive` IPC.
 *
 * Why this exists:
 *   - The Python sidecar's `promotion_history[]` is
 *     capped at 20 entries (v0.19a). When the 21st
 *     promote happens, the oldest is silently dropped.
 *   - v0.33a made the dropped entries durable by
 *     writing them to `archive.jsonl` before the cap
 *     takes effect.
 *   - v0.33b added the `list_promote_history_archive`
 *     IPC.
 *   - v0.34a (this file) is the L1 UI for reading
 *     the archive.
 *
 * The component is a Modal opened by a "View archive"
 * button (typically placed near the existing
 * PromoteHistory panel on the ModelLab page).
 *
 * Render states:
 *  1. loading  → skeleton rows
 *  2. error    → ErrorState
 *  3. empty    → "no archive yet" message
 *  4. populated → table of entries with prev/next
 *     pagination buttons
 *
 * Pagination: offset/limit, page size = 25 (capped at
 * 1000 by the Rust IPC). The user can navigate with
 * Prev/Next buttons or close + reopen to reset to
 * page 0.
 */
import { useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { Archive, ChevronLeft, ChevronRight, Trophy, AlertCircle } from 'lucide-react';
import { listPromoteHistoryArchive, type PromoteHistoryArchiveEntry } from '@/ipc';
import { useT } from '@/lib/i18n';
import { fmtRelativeTime } from '@/lib/format';
import { Modal } from '@/components/feedback/Modal';
import { Button } from '@/components/base/Button';
import { BadgePill } from '@/components/base/BadgePill';
import { Skeleton } from '@/components/feedback/Skeleton';
import { ErrorState } from '@/components/feedback/ErrorState';
import { EmptyState } from '@/components/feedback/EmptyState';

interface PromoteHistoryArchiveProps {
  /** Whether the modal is open. */
  open: boolean;
  /** Called when the user closes the modal. */
  onClose: () => void;
}

const PAGE_SIZE = 25;

function brierColor(brier: number | null): 'bull' | 'warn' | 'bear' | 'muted' {
  if (brier === null) return 'muted';
  if (brier < 0.15) return 'bull';
  if (brier < 0.2) return 'warn';
  return 'bear';
}

export function PromoteHistoryArchive({ open, onClose }: PromoteHistoryArchiveProps) {
  const { t } = useT();
  // v0.34a — pagination state (offset-based; user can
  // navigate with Prev/Next buttons)
  const [offset, setOffset] = useState(0);
  const { data, isLoading, error, refetch } = useQuery({
    queryKey: ['promote-history-archive', offset],
    queryFn: () =>
      listPromoteHistoryArchive({ offset, limit: PAGE_SIZE }),
    enabled: open,
    staleTime: 30_000,
  });

  // Reset offset when the modal opens (so the user
  // always starts at the newest entries)
  // We use a key trick: the parent passes a new `key`
  // each time it opens, but that's the parent's call.
  // For simplicity, we just reset on close.
  const handleClose = () => {
    setOffset(0);
    onClose();
  };

  const entries: PromoteHistoryArchiveEntry[] = data?.entries ?? [];
  const total = data?.total ?? 0;
  const noArchive = data?.message?.includes('no archive yet') ?? false;

  // Compute page bounds
  const startIdx = total === 0 ? 0 : offset + 1;
  const endIdx = Math.min(offset + entries.length, total);
  const hasPrev = offset > 0;
  const hasNext = offset + entries.length < total;

  return (
    <Modal
      open={open}
      onClose={handleClose}
      title={
        <div className="flex items-center gap-2">
          <Archive className="w-4 h-4" />
          <span>{t('promote.archive.title')}</span>
        </div>
      }
      size="lg"
      footer={
        <div className="flex items-center justify-between w-full">
          <div className="text-[11px] text-muted">
            {total === 0
              ? t('promote.archive.no_entries')
              : t('promote.archive.range', {
                  start: startIdx,
                  end: endIdx,
                  total,
                })}
          </div>
          <div className="flex items-center gap-1">
            <Button
              size="sm"
              variant="ghost"
              iconLeft={<ChevronLeft className="w-3 h-3" />}
              onClick={() => setOffset(Math.max(0, offset - PAGE_SIZE))}
              disabled={!hasPrev || isLoading}
              data-testid="promote-history-archive-prev"
            >
              {t('common.prev')}
            </Button>
            <Button
              size="sm"
              variant="ghost"
              iconRight={<ChevronRight className="w-3 h-3" />}
              onClick={() => setOffset(offset + PAGE_SIZE)}
              disabled={!hasNext || isLoading}
              data-testid="promote-history-archive-next"
            >
              {t('common.next')}
            </Button>
          </div>
        </div>
      }
    >
      {isLoading ? (
        <div
          className="space-y-1.5"
          data-testid="promote-history-archive-loading"
        >
          {Array.from({ length: 5 }).map((_, i) => (
            <Skeleton key={i} className="h-7" />
          ))}
        </div>
      ) : error ? (
        <ErrorState
          message={String(error)}
          onRetry={() => refetch()}
        />
      ) : noArchive || total === 0 ? (
        <EmptyState
          icon={<AlertCircle className="w-5 h-5" />}
          title={t('promote.archive.empty_title')}
          description={t('promote.archive.empty_desc')}
        />
      ) : (
        <div
          className="space-y-1 max-h-[400px] overflow-y-auto"
          data-testid="promote-history-archive"
          data-count={entries.length}
          data-total={total}
        >
          {entries.map((e) => (
            <ArchiveRow key={`${e.job_id}-${e.promoted_at_ms}`} entry={e} />
          ))}
        </div>
      )}
    </Modal>
  );
}

function ArchiveRow({ entry }: { entry: PromoteHistoryArchiveEntry }) {
  const { t } = useT();
  const color = brierColor(entry.best_brier);
  return (
    <div
      className="rounded border border-border bg-surface-1 px-2 py-1.5 flex items-center gap-2"
      data-testid="promote-history-archive-row"
      data-job-id={entry.job_id}
      data-brier={entry.best_brier ?? ''}
      data-trial-index={entry.trial_index ?? ''}
    >
      <Trophy className={`w-3 h-3 shrink-0 text-${color}`} />
      <div className="flex-1 min-w-0">
        <div className="font-mono text-[11px] text-fg truncate flex items-center gap-1.5">
          {entry.model_version}
          {entry.trial_index !== null && entry.trial_index !== undefined ? (
            <BadgePill
              variant="accent"
              data-testid="promote-history-archive-trial-badge"
              data-trial-index={entry.trial_index}
            >
              {t('promote.history.trial_n', { n: entry.trial_index + 1 })}
            </BadgePill>
          ) : (
            <BadgePill
              variant="neutral"
              data-testid="promote-history-archive-trial-badge"
              data-trial-index="best"
            >
              {t('promote.history.trial_best')}
            </BadgePill>
          )}
        </div>
        <div className="text-[10px] text-muted">
          {fmtRelativeTime(entry.promoted_at_ms)}
          {entry.best_brier !== null && (
            <>
              {' · '}
              <span className={`text-${color}`}>
                {t('promote.history.brier')} {entry.best_brier.toFixed(3)}
              </span>
            </>
          )}
        </div>
      </div>
    </div>
  );
}
