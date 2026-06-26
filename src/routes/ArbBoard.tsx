/**
 * ArbBoard —— 套利机会扫描器，包含平台内（intra-platform）
 * 和跨平台（cross-platform）两个 tab。
 *
 * Tab 1（intra）：扫描所有活动的 football market，查找
 * YES+NO 总成本 < $1.00 的情况。
 * Tab 2（cross）：扫描 Polymarket ↔ Kalshi 间价差 ≥ 3%
 * 的情况。
 * 每个机会卡片展示 market 问题、成本明细和利润率。
 * 手动「Scan」按钮触发重新扫描。
 *
 * 路由：`/arb-board`
 */
import { useState } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { Link } from 'react-router-dom';
import { RefreshCw, TrendingUp, ArrowRight } from 'lucide-react';
import { arbScan, crossPlatformArbScan, type ArbOpportunity, type CrossPlatformArb } from '@/ipc';
import { Card } from '@/components/base/Card';
import { Button } from '@/components/base/Button';
import { Skeleton } from '@/components/feedback/Skeleton';
import { ErrorState } from '@/components/feedback/ErrorState';
import { EmptyState } from '@/components/feedback/EmptyState';
import { useT } from '@/lib/i18n';
import { toast } from '@/stores/toast-store';

/** 活动 tab 标识：'intra' 表示同平台，'cross' 表示 Polymarket↔Kalshi。 */
type TabId = 'intra' | 'cross';

/**
 * ArbBoard —— 主路由组件。
 * 管理 tab 状态并渲染当前 tab 的套利列表。
 */
export function ArbBoard() {
  const { t } = useT();
  const queryClient = useQueryClient();
  const [tab, setTab] = useState<TabId>('intra');

  const intraQuery = useQuery({
    queryKey: ['arb-scan'],
    queryFn: () => arbScan(),
    staleTime: 60_000,
  });

  const crossQuery = useQuery({
    queryKey: ['cross-arb-scan'],
    queryFn: () => crossPlatformArbScan(),
    staleTime: 60_000,
  });

  const handleScan = async () => {
    try {
      if (tab === 'intra') {
        await arbScan();
      } else {
        await crossPlatformArbScan();
      }
      queryClient.invalidateQueries({ queryKey: tab === 'intra' ? ['arb-scan'] : ['cross-arb-scan'] });
      toast.success('Scan complete', `Scanned for ${tab === 'intra' ? 'intra-platform' : 'cross-platform'} arbitrage`);
    } catch (e) {
      toast.error('Scan failed', String(e));
    }
  };

  return (
    <div className="p-4 space-y-4 max-w-5xl">
      <div className="flex items-center justify-between">
        <h1 className="text-title-lg font-semibold" style={{ color: 'var(--fg)' }}>
          {t('arb.title')}
        </h1>
        <Button size="sm" variant="secondary" onClick={handleScan} loading={intraQuery.isFetching || crossQuery.isFetching}>
          <RefreshCw className="w-3.5 h-3.5" />
          {t('arb.scan')}
        </Button>
      </div>

      {/* Tab 切换器 */}
      <div className="flex gap-1 border-b" style={{ borderColor: 'var(--border)' }}>
        {([
          { id: 'intra' as const, label: t('arb.title') },
          { id: 'cross' as const, label: t('arb.cross_platform') },
        ]).map((tb) => (
          <button
            key={tb.id}
            onClick={() => setTab(tb.id)}
            className="px-3 py-1.5 text-body-sm font-medium border-b-2 transition-colors"
            style={{
              borderColor: tab === tb.id ? 'var(--accent)' : 'transparent',
              color: tab === tb.id ? 'var(--accent)' : 'var(--muted)',
            }}
          >
            {tb.label}
          </button>
        ))}
      </div>

      {/* 平台内 tab */}
      {tab === 'intra' && (
        <IntraArbList query={intraQuery} t={t} />
      )}

      {/* 跨平台 tab */}
      {tab === 'cross' && (
        <CrossArbList query={crossQuery} t={t} />
      )}
    </div>
  );
}

function IntraArbList({ query, t }: { query: ReturnType<typeof useQuery<ArbOpportunity[]>>; t: (k: string) => string }) {
  if (query.isLoading) return <Skeleton className="h-40" />;
  if (query.error) return <ErrorState message={String(query.error)} onRetry={() => query.refetch()} />;
  if (!query.data || query.data.length === 0) {
    return <EmptyState title={t('arb.no_opportunities')} icon={<TrendingUp className="w-5 h-5" />} />;
  }

  return (
    <div className="space-y-2">
      {query.data.map((arb, i) => (
        <Card key={i} padding="sm">
          <div className="flex items-center justify-between gap-4">
            <div className="flex-1 min-w-0">
              <Link to={`/markets/${arb.market_id}`} className="text-body-sm font-medium hover:underline" style={{ color: 'var(--fg)' }}>
                {arb.question}
              </Link>
              <div className="flex items-center gap-3 mt-1 text-[11px] font-mono" style={{ color: 'var(--muted)' }}>
                <span>{t('arb.yes_cost')}: ${arb.yes_cost.toFixed(2)}</span>
                <span>+</span>
                <span>{t('arb.no_cost')}: ${arb.no_cost.toFixed(2)}</span>
                <span>=</span>
                <span style={{ color: 'var(--bear)' }}>${arb.total_cost.toFixed(2)}</span>
              </div>
            </div>
            <div className="text-right">
              <div className="text-body-sm font-mono font-semibold" style={{ color: 'var(--bull)' }}>
                +{(arb.profit_margin).toFixed(1)}%
              </div>
              <div className="text-[10px]" style={{ color: 'var(--muted)' }}>
                {t('arb.profit_margin')}
              </div>
            </div>
            <Link to={`/markets/${arb.market_id}`}>
              <ArrowRight className="w-4 h-4" style={{ color: 'var(--muted)' }} />
            </Link>
          </div>
        </Card>
      ))}
    </div>
  );
}

function CrossArbList({ query, t }: { query: ReturnType<typeof useQuery<CrossPlatformArb[]>>; t: (k: string) => string }) {
  if (query.isLoading) return <Skeleton className="h-40" />;
  if (query.error) return <ErrorState message={String(query.error)} onRetry={() => query.refetch()} />;
  if (!query.data || query.data.length === 0) {
    return <EmptyState title={t('arb.no_opportunities')} icon={<TrendingUp className="w-5 h-5" />} />;
  }

  return (
    <div className="space-y-2">
      {query.data.map((arb, i) => (
        <Card key={i} padding="sm">
          <div className="flex items-center justify-between gap-4">
            <div className="flex-1 min-w-0">
              <p className="text-body-sm font-medium" style={{ color: 'var(--fg)' }}>
                {arb.match_name}
              </p>
              <p className="text-[11px] truncate" style={{ color: 'var(--muted)' }}>
                {arb.market_question}
              </p>
              <div className="flex items-center gap-3 mt-1 text-[11px] font-mono" style={{ color: 'var(--muted)' }}>
                <span>PM: ${(arb.pm_price / 100).toFixed(3)}</span>
                <span>Kalshi: ${(arb.kalshi_price / 100).toFixed(3)}</span>
                <span style={{ color: 'var(--accent)' }}>{arb.direction}</span>
              </div>
            </div>
            <div className="text-right">
              <div className="text-body-sm font-mono font-semibold" style={{ color: 'var(--bull)' }}>
                {arb.spread.toFixed(1)}%
              </div>
              <div className="text-[10px]" style={{ color: 'var(--muted)' }}>
                ~${arb.est_profit_per_1000.toFixed(0)}/$1k
              </div>
            </div>
          </div>
        </Card>
      ))}
    </div>
  );
}
