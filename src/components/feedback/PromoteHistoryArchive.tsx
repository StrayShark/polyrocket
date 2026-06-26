/**
 * PromoteHistoryArchive —— v0.34a。
 *
 * 显示**完整** promote history(不仅仅是
 * 内存中最近 20 条)的 modal。通过 v0.33b 的
 * `list_promote_history_archive` IPC,读取
 * Python sidecar 的 `archive.jsonl`。
 *
 * 为什么需要这个组件:
 *   - Python sidecar 的 `promotion_history[]` 在
 *     v0.19a 限制为 20 条。第 21 次 promote 时,
 *     最旧的会被静默丢弃。
 *   - v0.33a 让被丢弃的 entry 在 cap 生效前
 *     写入 `archive.jsonl` 实现持久化。
 *   - v0.33b 新增了 `list_promote_history_archive`
 *     IPC。
 *   - v0.34a(本文件)是 L1 用于读取 archive 的 UI。
 *
 * 本组件是 Modal,由 "View archive" 按钮打开
 * (通常放在 ModelLab 页面现有 PromoteHistory
 * 面板附近)。
 *
 * 渲染状态:
 *  1. loading  → skeleton 行
 *  2. 错误    → ErrorState
 *  3. empty    → "no archive yet" 提示
 *  4. populated → 含 prev/next 分页按钮的 entry 表格
 *
 * 分页:offset/limit,page size = 25(由 Rust IPC
 * 限制上限 1000)。用户可通过 Prev/Next 按钮
 * 翻页,或关闭后重新打开回到第 0 页。
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
  /** modal 是否打开。 */
  open: boolean;
  /** 用户关闭 modal 时调用。 */
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
  // v0.34a —— 分页 state(基于 offset;用户可
  // 通过 Prev/Next 按钮翻页)
  const [offset, setOffset] = useState(0);
  const { data, isLoading, error, refetch } = useQuery({
    queryKey: ['promote-history-archive', offset],
    queryFn: () =>
      listPromoteHistoryArchive({ offset, limit: PAGE_SIZE }),
    enabled: open,
    staleTime: 30_000,
  });

  // modal 打开时重置 offset(让用户始终
  // 从最新 entry 开始)
  // 采用 key trick:父组件每次打开时传新 `key`,
  // 但这取决于父组件。
  // 为简单起见,我们在关闭时重置。
  const handleClose = () => {
    setOffset(0);
    onClose();
  };

  const entries: PromoteHistoryArchiveEntry[] = data?.entries ?? [];
  const total = data?.total ?? 0;
  const noArchive = data?.message?.includes('no archive yet') ?? false;

  // 计算页边界
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
