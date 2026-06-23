/**
 * L1 mirror panel — derives pending mirrors from L1 data using
 * the L3 should_mirror() decision rule.
 *
 * No backend IPC for "pending mirrors" yet (M5 phase 2); this view
 * is computed client-side from existing data:
 *  - listCopyTargets()
 *  - recentCopyEvents()
 *  - listActiveSignals()  (to get market edges)
 *
 * For each CopyEvent, find the best active signal for the same
 * market and call should_mirror(target, event.side, event.size, signal.edge).
 */

import { useQuery } from '@tanstack/react-query';
import { useMemo } from 'react';
import { Zap, CheckCircle2, X, Clock, Loader2, AlertCircle } from 'lucide-react';
import { listCopyTargets, recentCopyEvents, listActiveSignals } from '@/ipc';
import { Card } from '@/components/base/Card';
import { Pill } from '@/components/base/Pill';
import { Skeleton } from '@/components/feedback/Skeleton';
import { ErrorState } from '@/components/feedback/ErrorState';
import { EmptyState } from '@/components/feedback/EmptyState';
import { shouldMirror, type CopyTargetInput } from '@/lib/domain/copy';
import { fmtUsdc, fmtRelativeTime } from '@/lib/format';

type MirrorState = 'pending' | 'submitted' | 'filled' | 'rejected' | 'expired';

export interface PendingMirror {
  id: string;
  eventId: number;
  targetLabel: string | null;
  marketId: string;
  whaleSide: string;
  whaleSize: string;
  marketEdge: number;
  mirrorSide: 'YES' | 'NO';
  mirrorSize: string;
  flip: boolean;
  state: MirrorState;
  detectedAt: number;
  reason?: string;
}

/**
 * `MirrorPanel` —— `/copy` 的 mirror 队列面板（M5 phase 2 preview）。
 *
 * **业务流**（L1 客户端算 + L3 `should_mirror`）：
 *   1. 拉 `listCopyTargets()` / `recentCopyEvents()` / `listActiveSignals()`
 *   2. 对每个 `CopyEvent`，找同 market 的「最佳 active signal」
 *   3. 调 `shouldMirror(target, event.side, event.size, signal.edge)` 决定是否 mirror
 *   4. 渲染：pending / submitted / filled / rejected / expired 五种 state
 *
 * **`flip = true` 含义**：model 跟 whale 方向相反但仍 mirror。L1 显示「⚠️ disagree」。
 *
 * **不是 IPC 后端**：当前 L1 客户端 derive，**未来 v0.62+** Rust 端会暴露
 * `list_pending_mirrors` IPC 走 DB `copy_mirror_queue` 表。
 */
export function MirrorPanel() {
  const targets = useQuery({
    queryKey: ['copy-targets'],
    queryFn: () => listCopyTargets(),
    staleTime: 60_000,
  });
  const events = useQuery({
    queryKey: ['copy-events'],
    queryFn: () => recentCopyEvents(undefined, 100),
    refetchInterval: 30_000,
  });
  const signals = useQuery({
    queryKey: ['signals', 'active', { limit: 200 }],
    queryFn: () => listActiveSignals({ limit: 200 }),
  });

  const mirrors = useMemo<PendingMirror[]>(() => {
    if (!targets.data || !events.data || !signals.data) return [];
    const out: PendingMirror[] = [];
    for (const event of events.data) {
      const target = targets.data.find((t) => t.id === event.target_id);
      if (!target) continue;
      const targetInput: CopyTargetInput = {
        id: target.id,
        address: target.address,
        label: target.label,
        enabled: target.enabled,
        allocationCap: target.allocation_cap ?? null,
        minEdge: target.min_edge,
        createdAt: target.created_at,
      };
      // best signal for this market
      const sig = signals.data
        .filter((s) => s.market_id === event.market_id)
        .reduce<typeof signals.data[number] | undefined>(
          (best, s) => (best === undefined || Math.abs(s.edge) > Math.abs(best.edge) ? s : best),
          undefined,
        );
      if (!sig) continue;
      const decision = shouldMirror(targetInput, event.side, event.size, sig.edge);
      if (!decision) {
        out.push({
          id: `no_${event.id}`,
          eventId: event.id,
          targetLabel: target.label,
          marketId: event.market_id,
          whaleSide: event.side,
          whaleSize: event.size,
          marketEdge: sig.edge,
          mirrorSide: 'YES',
          mirrorSize: '0',
          flip: false,
          state: 'rejected',
          detectedAt: event.detected_at,
          reason: edgeReason(targetInput, sig.edge),
        });
        continue;
      }
      // If event already matched a bet → filled, otherwise pending.
      const state: MirrorState = event.matched_bet_id ? 'filled' : 'pending';
      out.push({
        id: `mir_${event.id}`,
        eventId: event.id,
        targetLabel: target.label,
        marketId: event.market_id,
        whaleSide: event.side,
        whaleSize: event.size,
        marketEdge: sig.edge,
        mirrorSide: decision.side,
        mirrorSize: decision.size,
        flip: decision.flip,
        state,
        detectedAt: event.detected_at,
      });
    }
    return out;
  }, [targets.data, events.data, signals.data]);

  if (targets.error) return <ErrorState message={String(targets.error)} />;
  if (events.isLoading || targets.isLoading) {
    return (
      <div className="space-y-2">
        {Array.from({ length: 3 }).map((_, i) => (
          <Skeleton key={i} className="h-16" />
        ))}
      </div>
    );
  }

  const stats = computeStats(mirrors);

  return (
    <div className="space-y-3">
      <div className="grid grid-cols-2 md:grid-cols-5 gap-2">
        <Stat label="Total" value={stats.total.toString()} kind="muted" />
        <Stat label="Pending" value={stats.pending.toString()} kind="accent" icon={Clock} />
        <Stat label="Submitted" value={stats.submitted.toString()} kind="accent" icon={Loader2} />
        <Stat label="Filled" value={stats.filled.toString()} kind="bull" icon={CheckCircle2} />
        <Stat label="Rejected" value={stats.rejected.toString()} kind="bear" icon={X} />
      </div>

      {mirrors.length === 0 ? (
        <EmptyState
          icon={<Zap className="w-5 h-5" />}
          title="No mirror activity"
          description="When watched whales trade, mirror decisions will appear here."
        />
      ) : (
        <div className="space-y-1.5">
          {mirrors.map((m) => (
            <MirrorRow key={m.id} m={m} />
          ))}
        </div>
      )}

      <div className="text-[10px] text-muted px-1 pt-1">
        Mirror rule (from <code className="font-mono">domain::copy::should_mirror</code>):
        enabled target AND <code className="font-mono">|edge| ≥ min_edge</code> AND
        fill size &gt; 0 AND size ≤ allocation_cap.
      </div>
    </div>
  );
}

function MirrorRow({ m }: { m: PendingMirror }) {
  const stateKind =
    m.state === 'filled' ? 'bull' :
    m.state === 'rejected' ? 'bear' :
    m.state === 'expired' ? 'muted' :
    'accent';
  const StateIcon =
    m.state === 'filled' ? CheckCircle2 :
    m.state === 'rejected' ? X :
    m.state === 'expired' ? AlertCircle :
    m.state === 'submitted' ? Loader2 : Clock;
  return (
    <div className="rounded-md border border-border bg-surface-2 p-2.5 flex items-center gap-3">
      <Pill kind={stateKind as 'bull' | 'bear' | 'muted' | 'accent'}>
        <StateIcon className="w-2.5 h-2.5" /> {m.state}
      </Pill>
      <div className="flex-1 min-w-0">
        <div className="text-[12px] text-fg">
          <span className="font-mono">{m.targetLabel ?? 'target'}</span>{' '}
          bought <span className="font-mono">${fmtUsdc(m.whaleSize)} {m.whaleSide}</span>{' '}
          on <span className="text-muted">{m.marketId.slice(0, 14)}…</span>
        </div>
        {m.state === 'rejected' ? (
          <div className="text-[10px] text-muted mt-0.5">
            rejected: {m.reason}
          </div>
        ) : (
          <div className="text-[10px] text-muted mt-0.5 flex items-center gap-2">
            <span>
              edge {(m.marketEdge * 100).toFixed(1)}%
            </span>
            <span>·</span>
            <span>
              mirror{' '}
              <span className={m.flip ? 'text-warning' : 'text-fg'}>
                ${fmtUsdc(m.mirrorSize)} {m.mirrorSide}
                {m.flip && ' (flipped)'}
              </span>
            </span>
            <span>·</span>
            <span>{fmtRelativeTime(m.detectedAt)}</span>
          </div>
        )}
      </div>
    </div>
  );
}

function Stat({
  label,
  value,
  kind,
  icon: Icon,
}: {
  label: string;
  value: string;
  kind: 'muted' | 'accent' | 'bull' | 'bear';
  icon?: React.ComponentType<{ className?: string }>;
}) {
  return (
    <Card padding="sm">
      <div className="flex items-center gap-1.5 text-xs text-muted font-semibold uppercase tracking-caption-uppercase">
        {Icon && <Icon className="w-3 h-3" />}
        {label}
      </div>
      <div className="text-title-md font-mono font-semibold text-fg mt-0.5">{value}</div>
      <div className="h-0.5 mt-1.5 rounded-full" style={{ background: pillBar(kind) }} />
    </Card>
  );
}

function pillBar(kind: 'muted' | 'accent' | 'bull' | 'bear'): string {
  switch (kind) {
    case 'muted': return 'var(--muted)';
    case 'accent': return 'var(--accent)';
    case 'bull': return 'var(--bull)';
    case 'bear': return 'var(--bear)';
  }
}

function computeStats(mirrors: PendingMirror[]) {
  return {
    total: mirrors.length,
    pending: mirrors.filter((m) => m.state === 'pending').length,
    submitted: mirrors.filter((m) => m.state === 'submitted').length,
    filled: mirrors.filter((m) => m.state === 'filled').length,
    rejected: mirrors.filter((m) => m.state === 'rejected').length,
  };
}

function edgeReason(target: CopyTargetInput, edge: number): string {
  if (!target.enabled) return 'target disabled';
  if (Math.abs(edge) < target.minEdge) return `|edge| ${(Math.abs(edge) * 100).toFixed(1)}% < min ${(target.minEdge * 100).toFixed(0)}%`;
  return 'no signal for this market';
}
