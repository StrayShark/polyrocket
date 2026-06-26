import { useQuery } from '@tanstack/react-query';
import { Scale } from 'lucide-react';
import { umaDisputeStatus } from '@/ipc';
import { useT } from '@/lib/i18n';

interface UmaDisputeBadgeProps {
  marketId: string;
}

/** 当存在活跃 dispute 时,在 MarketDetail 头部显示 UMA 徽章。 */
export function UmaDisputeBadge({ marketId }: UmaDisputeBadgeProps) {
  const { t } = useT();
  const { data } = useQuery({
    queryKey: ['uma-status', marketId],
    queryFn: () => umaDisputeStatus(marketId),
    enabled: !!marketId,
    staleTime: 10 * 60_000,
  });

  if (!data || data.status === 'clear') return null;

  const isDisputed = data.status === 'disputed';
  const color = isDisputed ? 'var(--bear, #ef4444)' : 'var(--accent, #f59e0b)';
  const label = isDisputed ? t('uma.disputed') : t('uma.resolving');

  return (
    <span
      className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-[10px] font-medium"
      style={{ background: `${color}20`, color, border: `1px solid ${color}40` }}
      title={data.detail ?? label}
    >
      <Scale className="w-3 h-3" />
      {label}
    </span>
  );
}
