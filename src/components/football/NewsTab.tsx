/**
 * NewsTab —— 展示与某市场相关的 catalyst 新闻条目。
 *
 * 显示新闻列表,包含相关度分数、影响方向
 * (up / down / neutral)、来源、相对时间。
 * 含一个 refresh 按钮以重新拉取新闻数据。
 * 在 MarketDetail 中以 tab 形式渲染,用于 football 市场。
 *
 * @param marketId - 要获取新闻的 Polymarket 市场 ID。
 */
import { useQuery } from '@tanstack/react-query';
import { ExternalLink, RefreshCw, Newspaper } from 'lucide-react';
import { marketNews, type NewsItem } from '@/ipc';
import { Card } from '@/components/base/Card';
import { Button } from '@/components/base/Button';
import { Skeleton } from '@/components/feedback/Skeleton';
import { ErrorState } from '@/components/feedback/ErrorState';
import { EmptyState } from '@/components/feedback/EmptyState';
import { fmtRelativeTime } from '@/lib/format';
import { useT } from '@/lib/i18n';

interface NewsTabProps {
  marketId: string;
}

/** 根据 impact 方向返回 CSS 颜色变量。 */
function impactColor(direction: string | null): string {
  if (direction === 'up') return 'var(--bull, #22c55e)';
  if (direction === 'down') return 'var(--bear, #ef4444)';
  return 'var(--muted)';
}

/** 返回 impact 方向对应的箭头符号(↑ / ↓ / →)。 */
function impactArrow(direction: string | null): string {
  if (direction === 'up') return '↑';
  if (direction === 'down') return '↓';
  return '→';
}

/** 渲染单条新闻卡片,含元数据和影响指示。 */
function NewsCard({ item, t }: { item: NewsItem; t: (k: string) => string }) {
  return (
    <div className="rounded-lg border p-3 mb-2" style={{ borderColor: 'var(--border)' }}>
      <div className="flex items-start gap-2">
        <Newspaper className="w-4 h-4 shrink-0 mt-0.5" style={{ color: 'var(--muted)' }} />
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2 mb-1">
            <span className="text-[10px]" style={{ color: 'var(--muted)' }}>
              {fmtRelativeTime(item.published_at)} · {item.source}
            </span>
            {item.relevance_score != null && (
              <span className="text-[10px] font-mono px-1.5 py-0.5 rounded" style={{ background: 'var(--surface-2)', color: 'var(--muted)' }}>
                {t('news.relevance')}: {(item.relevance_score * 100).toFixed(0)}%
              </span>
            )}
            {item.impact_direction && (
              <span className="text-[10px] font-mono" style={{ color: impactColor(item.impact_direction) }}>
                {impactArrow(item.impact_direction)} {t('news.impact')}
              </span>
            )}
          </div>
          <p className="text-body-sm font-medium mb-1" style={{ color: 'var(--fg)' }}>
            {item.title}
          </p>
          {item.summary && (
            <p className="text-[11px]" style={{ color: 'var(--muted)' }}>{item.summary}</p>
          )}
          <a
            href={item.url}
            target="_blank"
            rel="noopener noreferrer"
            className="inline-flex items-center gap-1 text-[10px] mt-1 hover:underline"
            style={{ color: 'var(--accent)' }}
          >
            <ExternalLink className="w-3 h-3" />
            Open source
          </a>
        </div>
      </div>
    </div>
  );
}

export function NewsTab({ marketId }: NewsTabProps) {
  const { t } = useT();
  const { data, isLoading, error, refetch, isFetching } = useQuery({
    queryKey: ['market-news', marketId],
    queryFn: () => marketNews(marketId),
    enabled: !!marketId,
    staleTime: 5 * 60_000,
  });

  if (isLoading) return <Skeleton className="h-32" />;
  if (error) return <ErrorState message={String(error)} onRetry={() => refetch()} />;

  return (
    <div className="space-y-3">
      <Card
        title={t('news.catalyst')}
        action={
          <Button size="sm" variant="ghost" onClick={() => refetch()} disabled={isFetching}>
            <RefreshCw className={`w-3.5 h-3.5 ${isFetching ? 'animate-spin' : ''}`} />
          </Button>
        }
      >
        {(!data || data.length === 0) ? (
          <EmptyState title={t('news.no_news')} />
        ) : (
          <div>
            {data.map((item) => (
              <NewsCard key={item.id} item={item} t={t} />
            ))}
          </div>
        )}
      </Card>
    </div>
  );
}
