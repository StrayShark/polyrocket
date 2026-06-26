import { useQuery } from '@tanstack/react-query';
import { Zap } from 'lucide-react';
import { listSpikeAlerts } from '@/ipc';
import { useT } from '@/lib/i18n';

interface SpikeBadgeProps {
  marketId: string;
}

/**
 * 当该市场近期出现价格 spike 时,在 MarketDetail 头部
 * 显示一个 spike alert 徽章。
 */
export function SpikeBadge({ marketId }: SpikeBadgeProps) {
  const { t } = useT();
  const { data } = useQuery({
    queryKey: ['spike-alerts', marketId],
    queryFn: () => listSpikeAlerts(10),
    staleTime: 30_000,
    refetchInterval: 60_000,
  });

  const recentSpike = data?.find(
    (s) => s.market_id === marketId && Date.now() - s.detected_at < 300_000, // 最近 5 分钟
  );

  if (!recentSpike) return null;

  const change = recentSpike.change_pct;
  const isPositive = change > 0;

  return (
    <span
      className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-[10px] font-medium animate-pulse"
      style={{
        background: isPositive ? 'var(--bull, #22c55e)' : 'var(--bear, #ef4444)',
        color: 'white',
      }}
      title={`${t('spike.price_moved')}: ${recentSpike.old_price.toFixed(1)}% → ${recentSpike.new_price.toFixed(1)}%`}
    >
      <Zap className="w-3 h-3" />
      {t('spike.detected')} {isPositive ? '+' : ''}{change.toFixed(1)}%
    </span>
  );
}
