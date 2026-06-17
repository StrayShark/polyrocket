/**
 * PromoteHistory — v0.19c, v0.20c.
 *
 * Read-only panel showing the list of past model
 * promotions, oldest first (capped at 20 on the
 * Python side). Each row shows:
 *
 *   - model_version  (e.g. "logistic-train-441c352b")
 *   - promoted_at    (relative: "3h ago" / "yesterday")
 *   - best_brier     (colored badge; green < 0.15,
 *                     yellow 0.15-0.20, red > 0.20)
 *   - Rollback button (v0.20c) — restores this version
 *     as the active model. Disabled while the rollback
 *     is in flight or this row IS the active model.
 *
 * v0.30a — added a trial-type filter at the top of
 * the panel. Three modes:
 *   - "all"   (default) — show every entry
 *   - "best"  — only entries promoted via the best-
 *               trial path (Promote / Promote if better)
 *   - "bulk"  — only entries promoted via the bulk
 *               path (Promote all / Promote trial N)
 * The filter is local component state (not persisted);
 * resets to "all" on remount.
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
 *   - the `rollback_model` mutation succeeding (this
 *     component invalidates itself in onSuccess)
 */
import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { History, RotateCcw, Trophy } from 'lucide-react';
import { listPromoteHistory, rollbackModel, type PromoteHistoryEntry } from '@/ipc';
import { fmtRelativeTime } from '@/lib/format';
import { useT } from '@/lib/i18n';
import { toast } from '@/stores/toast-store';
import { Skeleton } from '@/components/feedback/Skeleton';
import { ErrorState } from '@/components/feedback/ErrorState';
import { EmptyState } from '@/components/feedback/EmptyState';
import { Modal } from '@/components/feedback/Modal';
import { Button } from '@/components/base/Button';

interface PromoteHistoryProps {
  /** Optional className passthrough (for spacing). */
  className?: string;
  /** v0.20c — the currently active model version, so
   * the row can be marked as "active" (and its
   * Rollback button disabled — you can't roll back
   * to the active model). */
  activeModelVersion?: string | null;
  /** v0.40b — set of job_ids currently selected for
   * comparison. If undefined, comparison is
   * disabled (no checkboxes shown). */
  selectedForCompare?: Set<string>;
  /** v0.40b — called when the user toggles a
   * checkbox. Receives the new set. */
  onSelectionChange?: (next: Set<string>) => void;
}

function brierColor(brier: number | null): 'bull' | 'warn' | 'bear' | 'muted' {
  if (brier === null) return 'muted';
  if (brier < 0.15) return 'bull';
  if (brier < 0.2) return 'warn';
  return 'bear';
}

/** v0.30a — classify a history entry as best or bulk
 *  based on its `trial_index`. */
function entryTrialType(entry: PromoteHistoryEntry): 'best' | 'bulk' {
  return entry.trial_index === null || entry.trial_index === undefined
    ? 'best'
    : 'bulk';
}

type HistoryFilter = 'all' | 'best' | 'bulk';

export function PromoteHistory({
  className = '',
  activeModelVersion = null,
  selectedForCompare,
  onSelectionChange,
}: PromoteHistoryProps) {
  const { t } = useT();
  const queryClient = useQueryClient();
  // v0.30a — trial-type filter (local state, resets on remount)
  const [filter, setFilter] = useState<HistoryFilter>('all');
  const { data, isLoading, error, refetch } = useQuery({
    queryKey: ['promote-history'],
    queryFn: () => listPromoteHistory(),
    staleTime: 30_000,
  });

  const rollbackMut = useMutation({
    mutationFn: (model_version: string) => rollbackModel({ model_version }),
    onSuccess: (r) => {
      if (r.rolled_back) {
        toast.success(
          t('rollback.toast.rolled_back'),
          t('rollback.toast.version', { version: r.model_version }),
        );
        // v0.20c — refresh BOTH the active-model probe (so
        // the ModelVersionPill updates) and the history
        // panel (so the new "rollback" marker appears).
        queryClient.invalidateQueries({ queryKey: ['sidecar-active-model'] });
        queryClient.invalidateQueries({ queryKey: ['promote-history'] });
        queryClient.invalidateQueries({ queryKey: ['llm-performance'] });
      } else {
        toast.error(t('rollback.toast.failed'), r.message ?? undefined);
      }
    },
    onError: (e: Error) => {
      toast.error(t('rollback.toast.failed'), e.message);
    },
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
  // v0.30a — apply the trial-type filter before reversing.
  //   - "all"  → no filter
  //   - "best" → only entries promoted via the best path
  //   - "bulk" → only entries promoted via the bulk path
  const filtered =
    filter === 'all'
      ? entries
      : entries.filter((e) => entryTrialType(e) === filter);
  // Reverse to show newest first (the array is oldest-first
  // from the Python side, which is the natural order for
  // an append-only log; the UI reverses for "recent first").
  const reversed = [...filtered].reverse();
  const filteredEmpty = filtered.length === 0;
  // v0.40a — compute the set of selected entries (for
  // the multi-model comparison feature). We use job_id
  // as the unique key (model_version can collide if
  // a train was re-run with the same id, but that's
  // rare; job_id is the canonical key).
  // Limit to 3 selected; if the user selects more, we
  // only show 3 in the modal (the most recent 3).

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
      data-count={filtered.length}
    >
      {/* v0.30a — trial-type filter chips. Three buttons
          (All / Best / Bulk); the active one is highlighted.
          Local state, no IPC. Resets on remount. */}
      <div
        className="flex items-center gap-1 text-[10px]"
        data-testid="promote-history-filter"
        data-active={filter}
      >
        {(['all', 'best', 'bulk'] as const).map((f) => (
          <button
            key={f}
            type="button"
            data-testid={`promote-history-filter-${f}`}
            onClick={() => setFilter(f)}
            className={`px-2 py-0.5 rounded border transition-colors ${
              filter === f
                ? 'border-accent text-accent bg-accent/10'
                : 'border-border text-muted hover:text-fg'
            }`}
          >
            {t(`promote.history.filter.${f}`)}
          </button>
        ))}
      </div>
      {filteredEmpty ? (
        <div
          className="text-[11px] text-muted italic"
          data-testid="promote-history-filtered-empty"
        >
          {t('promote.history.filter_empty', { filter: t(`promote.history.filter.${filter}`) })}
        </div>
      ) : (
        reversed.map((e) => (
          <HistoryRow
            key={`${e.job_id}-${e.promoted_at_ms}`}
            entry={e}
            isActive={e.model_version === activeModelVersion}
            isPending={rollbackMut.isPending}
            pendingVersion={rollbackMut.variables}
            onRollback={(mv) => rollbackMut.mutate(mv)}
            isSelected={selectedForCompare?.has(e.job_id) ?? false}
            onSelectToggle={
              onSelectionChange
                ? (jobId: string) => {
                    // Limit to 3 selected. If the user
                    // adds a 4th, drop the oldest.
                    const next = new Set(selectedForCompare ?? new Set());
                    if (next.has(jobId)) {
                      next.delete(jobId);
                    } else {
                      if (next.size >= 3) {
                        // Drop the oldest (the first
                        // inserted — Set preserves
                        // insertion order)
                        const first = next.values().next().value;
                        if (first !== undefined) next.delete(first);
                      }
                      next.add(jobId);
                    }
                    onSelectionChange(next as Set<string>);
                  }
                : undefined
            }
          />
        ))
      )}
      {/* v0.40b — the comparison selection is now
          managed at the ModelLab level (so the
          "Compare" button can sit next to the
          "View archive" button in the Card footer).
          The PromoteHistory component itself doesn't
          render the checkbox — ModelLab injects it
          via the existing entry-level props, or via
          a separate row. See v0.40b for the integration. */}
    </div>
  );
}

function HistoryRow({
  entry,
  isActive,
  isPending,
  pendingVersion,
  onRollback,
  isSelected = false,
  onSelectToggle,
}: {
  entry: PromoteHistoryEntry;
  isActive: boolean;
  isPending: boolean;
  pendingVersion: string | undefined;
  onRollback: (model_version: string) => void;
  isSelected?: boolean;
  onSelectToggle?: (job_id: string) => void;
}) {
  const { t } = useT();
  const color = brierColor(entry.best_brier);
  // v0.20c — confirm before rollback (irreversible).
  const [confirming, setConfirming] = useState(false);
  const isThisRowPending = isPending && pendingVersion === entry.model_version;

  return (
    <div
      className="rounded-md border border-border bg-surface-2 p-2.5 flex items-center gap-3"
      data-testid="promote-history-row"
      data-job-id={entry.job_id}
      data-active={isActive}
      data-selected={isSelected}
    >
      {/* v0.40b — checkbox for multi-model comparison.
          Only shown if `onSelectToggle` is provided
          (i.e. the parent enables the comparison
          feature). */}
      {onSelectToggle && (
        <input
          type="checkbox"
          checked={isSelected}
          onChange={() => onSelectToggle(entry.job_id)}
          className="shrink-0"
          data-testid="promote-history-compare-checkbox"
          data-job-id={entry.job_id}
        />
      )}
      <Trophy className={`w-3.5 h-3.5 shrink-0 text-${color}`} />
      <div className="flex-1 min-w-0">
        <div className="font-mono text-[12px] text-fg truncate flex items-center gap-1.5">
          {entry.model_version}
          {isActive && (
            <span
              className="text-[9px] uppercase tracking-wide text-bull"
              data-testid="promote-history-active-badge"
            >
              {t('promote.history.active')}
            </span>
          )}
          {/* v0.24a — per-trial badge. The model version
              already has a `-t{N}` suffix for bulk-promoted
              trials, but the user has to look carefully to
              see it. A small explicit badge makes the trial
              source immediately visible.
              "best" → the auto-picked best trial
              "trial N" → a bulk-promoted specific trial
              missing trial_index → treat as best (v0.18 back-compat) */}
          {entry.trial_index != null ? (
            <span
              className="text-[9px] uppercase tracking-wide text-accent"
              data-testid="promote-history-trial-badge"
              data-trial-index={entry.trial_index}
            >
              {t('promote.history.trial_n', { n: entry.trial_index + 1 })}
            </span>
          ) : (
            <span
              className="text-[9px] uppercase tracking-wide text-muted"
              data-testid="promote-history-trial-badge"
              data-trial-index="best"
            >
              {t('promote.history.trial_best')}
            </span>
          )}
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
      {/* v0.20c — Rollback button. Disabled if this row
          IS the active model (no point rolling back to
          the current model). Shows a confirmation
          dialog before doing the actual rollback. */}
      {!isActive && (
        <Button
          data-testid="promote-history-rollback"
          variant="ghost"
          size="sm"
          iconLeft={<RotateCcw className="w-3 h-3" />}
          loading={isThisRowPending}
          disabled={isPending}
          onClick={() => setConfirming(true)}
        >
          {t('rollback.btn.rollback')}
        </Button>
      )}
      <Modal
        open={confirming}
        onClose={() => setConfirming(false)}
        title={t('rollback.confirm.title')}
        footer={
          <div className="flex justify-end gap-2">
            <Button
              variant="ghost"
              size="sm"
              onClick={() => setConfirming(false)}
              disabled={isThisRowPending}
            >
              {t('rollback.confirm.cancel')}
            </Button>
            <Button
              data-testid="rollback-confirm-btn"
              variant="primary"
              size="sm"
              loading={isThisRowPending}
              disabled={isThisRowPending}
              onClick={() => {
                onRollback(entry.model_version);
                setConfirming(false);
              }}
            >
              {t('rollback.confirm.confirm')}
            </Button>
          </div>
        }
      >
        <div className="space-y-2 text-[12px]">
          <p className="text-fg">
            {t('rollback.confirm.body', { version: entry.model_version })}
          </p>
          <p className="text-muted text-[11px]">
            {t('rollback.confirm.warning')}
          </p>
        </div>
      </Modal>
    </div>
  );
}
