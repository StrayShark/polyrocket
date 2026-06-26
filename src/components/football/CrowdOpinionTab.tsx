/**
 * CrowdOpinionTab —— 展示某市场的资金加权群体观点。
 *
 * 显示 YES/NO 资金分布、Herfindahl-Hirschman Index(集中度)、
 * 前 3 大钱包占比,以及 smart money divergence 指标。
 * 在 MarketDetail 中以 tab 形式渲染,用于 football 市场。
 *
 * Logic 1(行为金融):资金加权观点衡量的不是
 * 投票人数,而是各方背后的资金量。
 *
 * @param marketId - 获取 crowd opinion 的 Polymarket 市场 ID。
 */
import { useQuery } from '@tanstack/react-query';
import { Users, TrendingUp, AlertTriangle } from 'lucide-react';
import { crowdOpinion, type CrowdOpinion } from '@/ipc';
import { Card } from '@/components/base/Card';
import { Skeleton } from '@/components/feedback/Skeleton';
import { ErrorState } from '@/components/feedback/ErrorState';
import { EmptyState } from '@/components/feedback/EmptyState';
import { useT } from '@/lib/i18n';

/** 翻译函数的类型别名。 */
type TFn = (key: string) => string;

interface CrowdOpinionTabProps {
  marketId: string;
}

/** 返回 HHI 集中度水平的定性标签。 */
function hhiLabel(hhi: number): { label: string; color: string } {
  if (hhi < 0.1) return { label: 'diversified', color: 'var(--bull)' };
  if (hhi < 0.25) return { label: 'moderate', color: 'var(--muted)' };
  if (hhi < 0.5) return { label: 'concentrated', color: 'var(--bear)' };
  return { label: 'monopoly', color: 'var(--bear)' };
}

/** 用合适的单位后缀格式化 USD 金额。 */
function fmtUsd(v: number): string {
  if (v >= 1_000_000) return `$${(v / 1_000_000).toFixed(1)}M`;
  if (v >= 1_000) return `$${(v / 1_000).toFixed(1)}K`;
  return `$${v.toFixed(0)}`;
}

/** 渲染 YES/NO 资金分布条。 */
function CapitalSplitBar({ opinion, t }: { opinion: CrowdOpinion; t: TFn }) {
  const yesPct = opinion.yes_weighted_pct * 100;
  const noPct = opinion.no_weighted_pct * 100;

  return (
    <div data-testid="crowd-capital-split">
      <div className="flex items-center justify-between mb-2 text-sm">
        <span className="font-medium">{t('crowd.capital_split')}</span>
        <span className="text-[var(--muted)]">
          {opinion.n_holders} {t('crowd.holders')}
        </span>
      </div>
      <div className="flex h-8 rounded-lg overflow-hidden" data-testid="crowd-split-bar">
        <div
          className="flex items-center justify-center text-xs font-bold text-white"
          style={{ width: `${yesPct}%`, background: 'var(--bull, #22c55e)' }}
        >
          {yesPct > 10 ? `YES ${yesPct.toFixed(1)}%` : ''}
        </div>
        <div
          className="flex items-center justify-center text-xs font-bold text-white"
          style={{ width: `${noPct}%`, background: 'var(--bear, #ef4444)' }}
        >
          {noPct > 10 ? `NO ${noPct.toFixed(1)}%` : ''}
        </div>
      </div>
      <div className="flex justify-between mt-1 text-xs text-[var(--muted)]">
        <span>{fmtUsd(opinion.yes_capital)}</span>
        <span>{fmtUsd(opinion.no_capital)}</span>
      </div>
    </div>
  );
}

/** 渲染集中度指标(HHI + 前 3 占比)。 */
function ConcentrationMetrics({ opinion, t }: { opinion: CrowdOpinion; t: TFn }) {
  const hhiInfo = hhiLabel(opinion.hhi);

  return (
    <div data-testid="crowd-concentration" className="grid grid-cols-2 gap-3">
      <div className="rounded-lg border border-[var(--border)] p-3">
        <div className="text-xs text-[var(--muted)] mb-1">{t('crowd.hhi')}</div>
        <div className="text-lg font-bold" style={{ color: hhiInfo.color }}>
          {opinion.hhi.toFixed(3)}
        </div>
        <div className="text-xs" style={{ color: hhiInfo.color }}>
          {t(`crowd.hhi_${hhiInfo.label}`)}
        </div>
      </div>
      <div className="rounded-lg border border-[var(--border)] p-3">
        <div className="text-xs text-[var(--muted)] mb-1">{t('crowd.top3_share')}</div>
        <div className="text-lg font-bold">
          {(opinion.top3_share * 100).toFixed(1)}%
        </div>
        <div className="text-xs text-[var(--muted)]">
          {t('crowd.top3_desc')}
        </div>
      </div>
    </div>
  );
}

/** 渲染 smart money divergence 指标。 */
function SmartDivergence({ opinion, t }: { opinion: CrowdOpinion; t: TFn }) {
  const div = opinion.smart_divergence;
  const isSignificant = Math.abs(div) > 0.10;

  return (
    <div
      data-testid="crowd-smart-divergence"
      className="rounded-lg border border-[var(--border)] p-3"
    >
      <div className="flex items-center gap-2 mb-2">
        <TrendingUp size={16} className="text-[var(--accent)]" />
        <span className="text-sm font-medium">{t('crowd.smart_divergence')}</span>
      </div>
      <div className="flex items-center justify-between">
        <div>
          <div className="text-xs text-[var(--muted)]">{t('crowd.crowd_yes')}</div>
          <div className="text-lg font-bold">
            {(opinion.yes_weighted_pct * 100).toFixed(1)}%
          </div>
        </div>
        <div className="text-2xl text-[var(--muted)]">vs</div>
        <div>
          <div className="text-xs text-[var(--muted)]">{t('crowd.smart_yes')}</div>
          <div className="text-lg font-bold" style={{ color: 'var(--accent)' }}>
            {(opinion.smart_yes_pct * 100).toFixed(1)}%
          </div>
        </div>
        <div className="ml-3">
          <div className="text-xs text-[var(--muted)]">{t('crowd.divergence')}</div>
          <div
            className="text-lg font-bold"
            style={{ color: isSignificant ? 'var(--bear)' : 'var(--muted)' }}
          >
            {div > 0 ? '+' : ''}{(div * 100).toFixed(1)}%
          </div>
        </div>
      </div>
      {isSignificant && (
        <div className="flex items-center gap-1 mt-2 text-xs" style={{ color: 'var(--bear)' }}>
          <AlertTriangle size={12} />
          <span>{t('crowd.divergence_alert')}</span>
        </div>
      )}
    </div>
  );
}

export function CrowdOpinionTab({ marketId }: CrowdOpinionTabProps) {
  const { t } = useT();
  const { data, isLoading, error } = useQuery({
    queryKey: ['crowd-opinion', marketId],
    queryFn: () => crowdOpinion(marketId),
    enabled: !!marketId,
  });

  if (isLoading) return <Skeleton className="h-48" />;
  if (error) return <ErrorState message={String(error)} />;
  if (!data || data.n_holders === 0) {
    return <EmptyState icon={<Users className="w-5 h-5" />} title={t('crowd.empty')} description={t('crowd.empty_desc')} />;
  }

  return (
    <Card title={t('crowd.title')} description={t('crowd.description')} data-testid="crowd-opinion-tab">
      <div className="space-y-4">
        <CapitalSplitBar opinion={data} t={t} />
        <ConcentrationMetrics opinion={data} t={t} />
        <SmartDivergence opinion={data} t={t} />
      </div>
    </Card>
  );
}
