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

export function Brief() {
  const queryClient = useQueryClient();
  const { data, isLoading, error, refetch, isRefetching } = useQuery({
    queryKey: ['daily-brief'],
    queryFn: () => dailyBriefGet(8),
    refetchInterval: 60_000,
  });

  const refreshMut = useMutation({
    mutationFn: () => dailyBriefRefresh(),
    onSuccess: (r) => {
      toast.success(`Brief refreshed`, `${r.n_items} items at ${fmtDate(r.computed_at)}`);
      queryClient.invalidateQueries({ queryKey: ['daily-brief'] });
    },
    onError: (e: Error) => toast.error('Refresh failed', e.message),
  });

  const dismissMut = useMutation({
    mutationFn: (marketId: string) => dailyBriefDismiss(marketId),
    onSuccess: () => {
      toast.info('Dismissed for 24h');
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
              <h2 className="text-[13px] font-semibold text-fg">Daily Brief</h2>
              <p className="text-[11px] text-muted mt-0.5">
                Top markets worth watching today. Scored by edge × confidence × consensus.
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
              Refresh
            </Button>
            <Button
              variant="primary"
              size="sm"
              iconLeft={<Sparkles className="w-3 h-3" />}
              loading={refreshMut.isPending}
              onClick={() => refreshMut.mutate()}
            >
              Re-score
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
          title="No brief entries"
          description="Run a refresh to score today's market candidates."
          action={
            <Button variant="primary" size="sm" onClick={() => refreshMut.mutate()} loading={refreshMut.isPending}>
              Generate now
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
                      <Pill kind="muted">dismissed</Pill>
                    )}
                  </div>
                  <div className="flex items-center gap-3 mt-0.5 text-[10px] text-muted">
                    <Pill kind="muted">{b.market_category}</Pill>
                    <span>closes {fmtDate(b.market_end_date)}</span>
                    <span>liq ${fmtUsdc(b.market_liquidity)}</span>
                  </div>
                </div>
                <div className="text-right shrink-0">
                  {b.edge != null && (
                    <div className={'text-[13px] font-mono font-semibold ' + (b.edge > 0 ? 'text-bull' : 'text-bear')}>
                      edge {fmtEdge(b.edge)}
                    </div>
                  )}
                  {b.confidence != null && (
                    <div className="text-[10px] text-muted">conf {fmtPct(b.confidence)}</div>
                  )}
                  {b.consensus_side && (
                    <Pill kind={b.consensus_side === 'YES' ? 'bull' : 'bear'}>
                      consensus {b.consensus_side} ({fmtPct(b.consensus_strength ?? 0)})
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
                    Dismiss
                  </Button>
                  <Link to={`/markets/${b.market_id}`}>
                    <Button variant="ghost" size="xs" iconRight={<ChevronRight className="w-3 h-3" />}>
                      Open
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
