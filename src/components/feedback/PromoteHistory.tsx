/**
 * PromoteHistory —— v0.19c、v0.20c。
 *
 * 只读面板,显示历史 promote 的 model 列表,
 * 最早在前(Python 端限制为 20)。每行展示:
 *
 *   - model_version(例如 "logistic-train-441c352b")
 *   - promoted_at(相对时间:"3h ago" / "yesterday")
 *   - best_brier(带色徽章;绿 < 0.15,
 *                黄 0.15-0.20,红 > 0.20)
 *   - Rollback 按钮(v0.20c)—— 将该版本恢复
 *     为 active model。在 rollback 进行中,
 *     或该行就是 active model 时,按钮被禁用。
 *
 * v0.30a —— 在面板顶部增加 trial-type 过滤器。
 * 三种模式:
 *   - "all"(默认)—— 显示全部 entry
 *   - "best"—— 只显示走 best-trial 路径
 *              promote 的 entry(Promote / Promote if better)
 *   - "bulk"—— 只显示走 bulk 路径
 *              promote 的 entry(Promote all / Promote trial N)
 * 过滤器是组件本地 state(不持久化);
 * 重新挂载时重置为 "all"。
 *
 * v0.41a —— 每行在 trial 徽章旁有一个小的 "i" 图标。
 * hover(或聚焦)时展示人类可读的 `reason` 字段,
 * 例如 "Promoted as best trial" 或
 * "Promoted as trial 2 of 4"。这与 trial 徽章
 * 重复,但更明确("这是显式选出的 best,
 * 而不只是恰好 Brier 最低")。原生 `<title>`
 * 属性是基线(始终开启,a11y);
 * 小 "i" 图标让其更易发现。
 * v0.41 之前的旧 entry 没有 `reason`;
 * 这种情况下我们展示通用 tooltip。
 *
 * 当前 active model **不在**该列表中 —— 要查看
 * active model,请使用页面顶部的 ModelVersionPill。
 * 此列表用于审计("某时刻哪个 model 是 active 的"),
 * 不用于状态展示。
 *
 * 刷新时机:
 *   - 挂载(初始 query)
 *   - `promote_model` mutation 成功(父组件
 *     传入 `refetchKey` 触发 invalidate)
 *   - `rollback_model` mutation 成功(本组件
 *     在 onSuccess 中自行 invalidate)
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
import { BadgePill } from '@/components/base/BadgePill';

interface PromoteHistoryProps {
  /** 可选的 className 透传(用于间距)。 */
  className?: string;
  /** v0.20c —— 当前 active model 版本,以便
   * 标记行为 "active"(并禁用其
   * Rollback 按钮 —— 不能回滚到 active model)。 */
  activeModelVersion?: string | null;
  /** v0.40b —— 当前已选用于对比的 job_id 集合。
   * 如果为 undefined,对比功能禁用(不显示 checkbox)。 */
  selectedForCompare?: Set<string>;
  /** v0.40b —— 用户切换 checkbox 时回调。
   * 接收新的集合。 */
  onSelectionChange?: (next: Set<string>) => void;
}

function brierColor(brier: number | null): 'bull' | 'warn' | 'bear' | 'muted' {
  if (brier === null) return 'muted';
  if (brier < 0.15) return 'bull';
  if (brier < 0.2) return 'warn';
  return 'bear';
}

/** v0.30a —— 根据 `trial_index` 将 history entry 归类为 best 或 bulk。 */
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
  // v0.30a —— trial-type 过滤器(本地 state,重挂时重置)
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
        // v0.20c —— 同时刷新 active-model probe(让
        // ModelVersionPill 更新)和 history 面板
        // (让新出现的 "rollback" 标记可见)。
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
  // v0.30a —— 在反转之前应用 trial-type 过滤器。
  //   - "all"  → 不过滤
  //   - "best" → 只保留走 best 路径 promote 的 entry
  //   - "bulk" → 只保留走 bulk 路径 promote 的 entry
  const filtered =
    filter === 'all'
      ? entries
      : entries.filter((e) => entryTrialType(e) === filter);
  // 反转以使最新在前(数组在 Python 端是最早在前,
  // 这是 append-only 日志的自然顺序;UI 反转为 "recent first")。
  const reversed = [...filtered].reverse();
  const filteredEmpty = filtered.length === 0;
  // v0.40a —— 计算已选 entry 集合(用于多 model 对比功能)。
  // 使用 job_id 作为唯一 key(model_version 在同一 id
  // 重新跑训练时可能冲突,但这很罕见;job_id 是规范 key)。
  // 最多 3 个;若用户选更多,modal 中只显示最近 3 个。

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
      {/* v0.30a —— trial-type 过滤 chips。三个按钮
          (All / Best / Bulk),当前激活的高亮。
          本地 state,无 IPC。重挂时重置。 */}
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
            className={`px-2 py-0.5 rounded border transition-colors duration-base ease-out-cubic ${
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
                    // 最多选 3 个。如果用户
                    // 添加第 4 个,则丢掉最早那个。
                    const next = new Set(selectedForCompare ?? new Set());
                    if (next.has(jobId)) {
                      next.delete(jobId);
                    } else {
                      if (next.size >= 3) {
                        // 丢掉最早的(第一个
                        // 插入的 —— Set 保留
                        // 插入顺序)
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
      {/* v0.40b —— 对比选择现在在 ModelLab 层面管理
          (这样 "Compare" 按钮可以与 Card 底部的
          "View archive" 按钮并排)。
          PromoteHistory 组件本身不渲染 checkbox —— ModelLab
          通过 entry 级别的现有 props,或通过单独的行注入。
          集成方式见 v0.40b。 */}
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
  // v0.20c —— 回滚前确认(不可逆)。
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
      {/* v0.40b —— 多 model 对比用的 checkbox。
          仅当 `onSelectToggle` 提供了才会显示
          (即父组件启用对比功能)。 */}
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
            <BadgePill
              variant="bull"
              data-testid="promote-history-active-badge"
            >
              {t('promote.history.active')}
            </BadgePill>
          )}
          {/* v0.24a —— per-trial 徽章。model version
              对 bulk-promoted trial 已经有 `-t{N}`
              后缀,但用户得仔细看才能注意到。
              一个小的显式徽章让 trial 来源一眼可见。
              "best" → 自动选出的 best trial
              "trial N" → bulk 路径下 promote 的某个 trial
              缺失 trial_index → 视为 best(v0.18 向后兼容) */}
          {entry.trial_index != null ? (
            <BadgePill
              variant="accent"
              data-testid="promote-history-trial-badge"
              data-trial-index={entry.trial_index}
            >
              {t('promote.history.trial_n', { n: entry.trial_index + 1 })}
            </BadgePill>
          ) : (
            <BadgePill
              variant="neutral"
              data-testid="promote-history-trial-badge"
              data-trial-index="best"
            >
              {t('promote.history.trial_best')}
            </BadgePill>
          )}
          {/* v0.41a —— 小 "i" 图标,tooltip 显示
              per-promotion 原因。该图标是显眼的可发现元素;
              原生 `title` 属性是 a11y 基线。
              旧 entry(无 `reason`)展示通用 "Promoted" tooltip。 */}
          <span
            className="text-[10px] text-muted cursor-help"
            data-testid="promote-history-reason-icon"
            data-reason={entry.reason ?? ''}
            title={entry.reason ?? t('promote.history.reason_fallback')}
            aria-label={entry.reason ?? t('promote.history.reason_fallback')}
          >
            ⓘ
          </span>
        </div>
        <div className="text-[10px] text-muted mt-0.5">
          {fmtRelativeTime(entry.promoted_at_ms)}
        </div>
      </div>
      <div className="text-right shrink-0">
        <div className="text-xs text-muted font-semibold uppercase tracking-caption-uppercase">
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
      {/* v0.20c —— Rollback 按钮。如果该行
          就是 active model,则禁用(回滚到当前
          model 没意义)。点击后弹确认框,
          确认后才真正执行 rollback。 */}
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
