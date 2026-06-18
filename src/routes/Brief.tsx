import { useQuery, useQueryClient, useMutation } from '@tanstack/react-query';
import { Calendar, Sparkles, RefreshCw, X, ChevronRight } from 'lucide-react';
import { Link } from 'react-router-dom';
import { dailyBriefGet, dailyBriefRefresh, dailyBriefDismiss } from '@/ipc';
import { Card } from '@/components/base/Card';
import { Pill } from '@/components/base/Pill';
import { Button } from '@/components/base/Button';
import { Skeleton } from '@/components/feedback/Skeleton';
import { ErrorState } from '@/components/feedback/ErrorState';
import { EmptyState } from '@/components/feedback/EmptyState';
import { toast } from '@/stores/toast-store';
import { fmtDate, fmtUsdc, fmtEdge, fmtPct } from '@/lib/format';
import { useT } from '@/lib/i18n';

/**
 * `/brief` 路由 —— Daily Brief 完整列表（v0.3+ spec，跟 Dashboard 上的
 * mini Brief 卡区分）。
 *
 * **数据流**：
 *   1. mount `dailyBriefGet(8)` 拉 top 8 条
 *   2. `refetchInterval: 60_000` 每分钟自动 refetch
 *   3. 「Refresh」按钮 → `dailyBriefRefresh` mutation
 *   4. 单条「Dismiss」→ `dailyBriefDismiss` mutation
 *
 * **状态机**：`loading`（Skeleton）→ `success`（卡片列表） / `error`。
 *
 * **vs Dashboard 上的 brief**：
 *   - Dashboard = 3-4 条 mini 卡（`WelcomeBanner` 旁边）
 *   - `/brief` = 8 条完整卡 + filter（按 match_score / edge）+ 详情展开
 */
export function Brief() {
  const { t } = useT();
  const queryClient = useQueryClient();
  const { data, isLoading, error, refetch, isRefetching } = useQuery({
    queryKey: ['daily-brief'],
    queryFn: () => dailyBriefGet(8),
    refetchInterval: 60_000,
  });

  const refreshMut = useMutation({
    mutationFn: () => dailyBriefRefresh(),
    onSuccess: (r) => {
      toast.success(
        t('brief.toast.refreshed'),
        t('brief.toast.refreshed_body', { n: r.n_items, when: fmtDate(r.computed_at) }),
      );
      queryClient.invalidateQueries({ queryKey: ['daily-brief'] });
    },
    onError: (e: Error) => toast.error(t('brief.toast.refresh_failed'), e.message),
  });

  const dismissMut = useMutation({
    mutationFn: (marketId: string) => dailyBriefDismiss(marketId),
    onSuccess: () => {
      toast.info(t('brief.toast.dismissed'));
      queryClient.invalidateQueries({ queryKey: ['daily-brief'] });
    },
  });

  if (error) return <ErrorState message={String(error)} onRetry={() => refetch()} />;

  return (
    <div className="space-y-4">
      <Card padding="sm">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <Calendar className="w-4 h-4 text-muted" />
            <div>
              <h2 className="text-[13px] font-semibold text-fg">{t('brief.title')}</h2>
              <p className="text-[11px] text-muted mt-0.5">
                {t('brief.subtitle')}
              </p>
            </div>
          </div>
          <div className="flex items-center gap-2">
            <Button
              variant="ghost"
              size="sm"
              iconLeft={<RefreshCw className={'w-3 h-3 ' + (isRefetching ? 'animate-spin' : '')} />}
              onClick={() => refetch()}
              disabled={isRefetching}
            >
              {t('brief.refresh')}
            </Button>
            <Button
              variant="primary"
              size="sm"
              iconLeft={<Sparkles className="w-3 h-3" />}
              loading={refreshMut.isPending}
              onClick={() => refreshMut.mutate()}
            >
              {t('brief.rescore')}
            </Button>
          </div>
        </div>
      </Card>

      {isLoading ? (
        <div className="space-y-2">
          {Array.from({ length: 4 }).map((_, i) => (
            <Skeleton key={i} className="h-20" />
          ))}
        </div>
      ) : !data || data.length === 0 ? (
        <EmptyState
          icon={<Calendar className="w-5 h-5" />}
          title={t('brief.empty.title')}
          description={t('brief.empty.desc')}
          action={
            <Button variant="primary" size="sm" onClick={() => refreshMut.mutate()} loading={refreshMut.isPending}>
              {t('brief.empty.generate')}
            </Button>
          }
        />
      ) : (
        <div className="space-y-2">
          {data.map((b) => (
            <Card key={b.market_id} padding="sm">
              <div className="flex items-center gap-3">
                <div className="flex items-center justify-center w-8 h-8 rounded-full bg-accent/15 text-accent text-[14px] font-mono font-semibold shrink-0">
                  {b.rank}
                </div>
                <div className="flex-1 min-w-0">
                  <div className="flex items-center gap-2">
                    <Link
                      to={`/markets/${b.market_id}`}
                      className="text-[13px] font-medium text-fg hover:text-accent line-clamp-1"
                    >
                      {b.market_question}
                    </Link>
                    {b.dismissed && (
                      <Pill kind="muted">{t('brief.entry.dismissed')}</Pill>
                    )}
                  </div>
                  <div className="flex items-center gap-3 mt-0.5 text-[10px] text-muted">
                    <Pill kind="muted">{b.market_category}</Pill>
                    <span>{t('brief.entry.closes', { when: fmtDate(b.market_end_date) })}</span>
                    <span>{t('brief.entry.liq', { n: fmtUsdc(b.market_liquidity) })}</span>
                  </div>
                </div>
                <div className="text-right shrink-0">
                  {b.edge != null && (
                    <div className={'text-[13px] font-mono font-semibold ' + (b.edge > 0 ? 'text-bull' : 'text-bear')}>
                      {t('brief.entry.edge', { value: fmtEdge(b.edge) })}
                    </div>
                  )}
                  {b.confidence != null && (
                    <div className="text-[10px] text-muted">{t('brief.entry.conf', { value: fmtPct(b.confidence) })}</div>
                  )}
                  {b.consensus_side && (
                    <Pill kind={b.consensus_side === 'YES' ? 'bull' : 'bear'}>
                      {t('brief.entry.consensus', {
                        side: b.consensus_side,
                        strength: fmtPct(b.consensus_strength ?? 0),
                      })}
                    </Pill>
                  )}
                </div>
                <div className="flex flex-col gap-1 shrink-0">
                  <Button
                    variant="ghost"
                    size="xs"
                    iconLeft={<X className="w-3 h-3" />}
                    onClick={() => dismissMut.mutate(b.market_id)}
                  >
                    {t('brief.btn.dismiss')}
                  </Button>
                  <Link to={`/markets/${b.market_id}`}>
                    <Button variant="ghost" size="xs" iconRight={<ChevronRight className="w-3 h-3" />}>
                      {t('brief.btn.open')}
                    </Button>
                  </Link>
                </div>
              </div>
            </Card>
          ))}
        </div>
      )}
    </div>
  );
}
