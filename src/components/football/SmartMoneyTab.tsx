/**
 * SmartMoneyTab —— 展示某市场 YES/NO 两侧的 smart money 分数。
 *
 * 每侧显示 0-100 分数及进度条、holder 统计分解
 * (钱包数、avg PnL、胜率、中位持仓)以及 top contributor
 * 钱包。仅在 football 市场中渲染。
 *
 * @param marketId - 要打分的 Polymarket 市场 ID。
 */
import { useQuery } from '@tanstack/react-query';
import { smartMoneyScore, type SideBreakdown } from '@/ipc';
import { Card } from '@/components/base/Card';
import { Skeleton } from '@/components/feedback/Skeleton';
import { ErrorState } from '@/components/feedback/ErrorState';
import { EmptyState } from '@/components/feedback/EmptyState';
import { useT } from '@/lib/i18n';

interface SmartMoneyTabProps {
  marketId: string;
}

/** 根据分数档位返回 CSS 颜色变量。 */
function scoreColor(score: number): string {
  if (score >= 70) return 'var(--bull, #22c55e)';
  if (score >= 40) return 'var(--accent, #f59e0b)';
  return 'var(--bear, #ef4444)';
}

/** 返回分数档位对应的可读标签。 */
function scoreLabel(score: number, t: (k: string) => string): string {
  if (score >= 70) return t('smart_money.score_smart');
  if (score >= 40) return t('smart_money.score_moderate');
  return t('smart_money.score_weak');
}

/** 渲染某一侧的 breakdown 卡片,含 holder 统计和 top 钱包。 */
function SideBreakdownCard({ title, breakdown, t }: { title: string; breakdown: SideBreakdown; t: (k: string) => string }) {
  return (
    <div className="rounded-lg border p-3" style={{ borderColor: 'var(--border)' }}>
      <h4 className="text-body-sm font-semibold mb-2" style={{ color: 'var(--fg)' }}>{title}</h4>
      <dl className="space-y-1 text-[11px]" style={{ color: 'var(--muted)' }}>
        <div className="flex justify-between">
          <dt>{t('smart_money.wallet_count')}</dt>
          <dd className="font-mono" style={{ color: 'var(--fg)' }}>{breakdown.wallet_count}</dd>
        </div>
        <div className="flex justify-between">
          <dt>{t('smart_money.avg_pnl')}</dt>
          <dd className="font-mono" style={{ color: breakdown.avg_pnl >= 0 ? 'var(--bull)' : 'var(--bear)' }}>
            {breakdown.avg_pnl >= 0 ? '+' : ''}{breakdown.avg_pnl.toFixed(2)} USDC
          </dd>
        </div>
        <div className="flex justify-between">
          <dt>{t('smart_money.win_rate')}</dt>
          <dd className="font-mono" style={{ color: 'var(--fg)' }}>{(breakdown.win_rate * 100).toFixed(1)}%</dd>
        </div>
        <div className="flex justify-between">
          <dt>{t('smart_money.median_position')}</dt>
          <dd className="font-mono" style={{ color: 'var(--fg)' }}>${breakdown.median_position.toFixed(0)}</dd>
        </div>
      </dl>
      {breakdown.top_wallets.length > 0 && (
        <div className="mt-3">
          <p className="text-[10px] mb-1" style={{ color: 'var(--muted)' }}>{t('smart_money.top_wallets')}</p>
          <div className="space-y-0.5">
            {breakdown.top_wallets.slice(0, 3).map((w, i) => (
              <div key={i} className="flex justify-between text-[10px] font-mono" style={{ color: 'var(--muted)' }}>
                <span>{w.address.slice(0, 6)}…{w.address.slice(-3)}</span>
                <span style={{ color: w.pnl >= 0 ? 'var(--bull)' : 'var(--bear)' }}>
                  {w.pnl >= 0 ? '+' : ''}{w.pnl.toFixed(0)} · {(w.win_rate * 100).toFixed(0)}% WR
                </span>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

export function SmartMoneyTab({ marketId }: SmartMoneyTabProps) {
  const { t } = useT();
  const { data, isLoading, error } = useQuery({
    queryKey: ['smart-money', marketId],
    queryFn: () => smartMoneyScore(marketId),
    enabled: !!marketId,
    staleTime: 60_000,
  });

  if (isLoading) return <Skeleton className="h-48" />;
  if (error) return <ErrorState message={String(error)} />;
  if (!data) return <EmptyState title={t('smart_money.score')} />;

  const yesColor = scoreColor(data.yes_score);
  const noColor = scoreColor(data.no_score);

  return (
    <div className="space-y-4">
      <Card title={t('smart_money.score')}>
        <div className="space-y-3">
          {/* YES 一方 */}
          <div>
            <div className="flex items-center justify-between mb-1">
              <span className="text-body-sm font-medium" style={{ color: 'var(--fg)' }}>
                {t('smart_money.yes_side')}
              </span>
              <span className="font-mono text-body-sm" style={{ color: yesColor }}>
                {data.yes_score.toFixed(0)}/100 · {scoreLabel(data.yes_score, t)}
              </span>
            </div>
            <div className="h-2 rounded-full overflow-hidden" style={{ background: 'var(--border)' }}>
              <div className="h-full rounded-full transition-all" style={{ width: `${data.yes_score}%`, background: yesColor }} />
            </div>
          </div>
          {/* NO 一方 */}
          <div>
            <div className="flex items-center justify-between mb-1">
              <span className="text-body-sm font-medium" style={{ color: 'var(--fg)' }}>
                {t('smart_money.no_side')}
              </span>
              <span className="font-mono text-body-sm" style={{ color: noColor }}>
                {data.no_score.toFixed(0)}/100 · {scoreLabel(data.no_score, t)}
              </span>
            </div>
            <div className="h-2 rounded-full overflow-hidden" style={{ background: 'var(--border)' }}>
              <div className="h-full rounded-full transition-all" style={{ width: `${data.no_score}%`, background: noColor }} />
            </div>
          </div>
          {/* 结论 */}
          <p className="text-[11px] mt-2" style={{ color: 'var(--muted)' }}>
            {data.yes_score > data.no_score
              ? `→ YES ${t('smart_money.verdict_smarter')} ${(data.yes_breakdown.win_rate * 100).toFixed(0)}%)`
              : `→ NO ${t('smart_money.verdict_smarter')} ${(data.no_breakdown.win_rate * 100).toFixed(0)}%)`}
          </p>
        </div>
      </Card>

      <div className="grid grid-cols-2 gap-3">
        <SideBreakdownCard title={`YES ${t('smart_money.yes_side')}`} breakdown={data.yes_breakdown} t={t} />
        <SideBreakdownCard title={`NO ${t('smart_money.no_side')}`} breakdown={data.no_breakdown} t={t} />
      </div>

      <Card title={t('smart_money.methodology')} padding="sm">
        <p className="text-[11px] leading-relaxed" style={{ color: 'var(--muted)' }}>
          Score = 40% avg holder win rate (30d) + 30% weighted PnL + 20% trader quality tier + 10% new wallet ratio (lower = more trusted).
        </p>
      </Card>
    </div>
  );
}
