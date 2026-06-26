/**
 * ReversionGauge —— 以可视化方式展示市场的均值回归统计。
 *
 * 显示当前价格相对布林带的位置、z-score 仪表,
 * 以及 fade signal 推荐。当 |z-score| > 2.0 时,
 * 市场处于"超调"状态,可能回归均值。
 *
 * Logic 1(行为金融):均值回归用于检测过度反应 —
 * 当散户恐慌买入或恐慌卖出时,价格偏离统计均值,
 * 形成 fade 机会。
 *
 * @param marketId - 要分析的 Polymarket 市场 ID。
 */
import { useQuery } from '@tanstack/react-query';
import { Activity } from 'lucide-react';
import { reversionSignal, type ReversionSignal } from '@/ipc';
import { Card } from '@/components/base/Card';
import { Skeleton } from '@/components/feedback/Skeleton';
import { ErrorState } from '@/components/feedback/ErrorState';
import { EmptyState } from '@/components/feedback/EmptyState';
import { useT } from '@/lib/i18n';

interface ReversionGaugeProps {
  marketId: string;
}

/** 根据 z-score 的大小和方向返回颜色。 */
function zScoreColor(z: number): string {
  if (z > 2.0) return 'var(--bear, #ef4444)';   // 超买 → 红
  if (z < -2.0) return 'var(--bull, #22c55e)';   // 超卖 → 绿
  return 'var(--muted)';                           // 中性
}

/** 翻译函数的类型别名。 */
type TFn = (key: string) => string;

/** 返回 fade signal 方向的标签。 */
function fadeLabel(fade: number, t: TFn): string {
  if (fade < -0.5) return t('reversion.fade_yes');
  if (fade > 0.5) return t('reversion.fade_no');
  return t('reversion.no_signal');
}

/** 渲染布林带可视化。 */
function BollingerBands({ signal, t }: { signal: ReversionSignal; t: TFn }) {
  const { current, bollinger_upper, bollinger_lower, mean } = signal;
  // 缩放:将 [0, 1] 映射到 [0%, 100%] 作为条形宽度。
  const range = Math.max(bollinger_upper - bollinger_lower, 0.01);
  const meanPct = (mean / range) * 100;
  const currentPct = (current / range) * 100;
  const upperPct = (bollinger_upper / range) * 100;
  const lowerPct = (bollinger_lower / range) * 100;

  return (
    <div data-testid="reversion-bollinger" className="space-y-2">
      <div className="text-xs text-[var(--muted)]">{t('reversion.bollinger_bands')}</div>
      <div className="relative h-12 rounded-lg bg-[var(--surface-2)] overflow-hidden">
        {/* 布林带区域 */}
        <div
          className="absolute h-full bg-[var(--accent)] opacity-10"
          style={{ left: `${lowerPct}%`, width: `${upperPct - lowerPct}%` }}
        />
        {/* 中线 */}
        <div
          className="absolute h-full w-0.5 bg-[var(--muted)]"
          style={{ left: `${meanPct}%` }}
        />
        {/* 当前价格标记 */}
        <div
          className="absolute h-full w-1 rounded-full"
          style={{
            left: `${Math.min(Math.max(currentPct, 0), 100)}%`,
            background: zScoreColor(signal.z_score),
          }}
        />
      </div>
      <div className="flex justify-between text-xs text-[var(--muted)]">
        <span>{t('reversion.lower')}: {(bollinger_lower * 100).toFixed(1)}¢</span>
        <span>{t('reversion.mean')}: {(mean * 100).toFixed(1)}¢</span>
        <span>{t('reversion.upper')}: {(bollinger_upper * 100).toFixed(1)}¢</span>
      </div>
    </div>
  );
}

/** 渲染 z-score 仪表和 fade signal。 */
function ZScoreGauge({ signal, t }: { signal: ReversionSignal; t: TFn }) {
  const z = signal.z_score;
  const color = zScoreColor(z);
  // 将 z-score 从 [-4, 4] 映射到 [0%, 100%],作为仪表位置。
  const gaugePct = Math.min(Math.max(((z + 4) / 8) * 100, 0), 100);

  return (
    <div data-testid="reversion-zscore" className="space-y-2">
      <div className="flex items-center justify-between">
        <span className="text-xs text-[var(--muted)]">{t('reversion.z_score')}</span>
        <span className="text-lg font-bold" style={{ color }}>
          {z > 0 ? '+' : ''}{z.toFixed(2)}
        </span>
      </div>
      {/* Z 分数仪表条 */}
      <div className="relative h-4 rounded-full bg-[var(--surface-2)] overflow-hidden">
        {/* 中心线 */}
        <div className="absolute h-full w-0.5 bg-[var(--muted)] left-1/2" />
        {/* Z 分数位置 */}
        <div
          className="absolute h-full w-2 rounded-full"
          style={{ left: `${gaugePct}%`, background: color, transform: 'translateX(-50%)' }}
        />
      </div>
      <div className="flex justify-between text-xs text-[var(--muted)]">
        <span>-4σ ({t('reversion.oversold')})</span>
        <span>0</span>
        <span>+4σ ({t('reversion.overbought')})</span>
      </div>
      {/* 反向信号 */}
      {signal.is_overextended && (
        <div
          data-testid="reversion-fade-signal"
          className="flex items-center justify-between rounded-lg p-3 mt-2"
          style={{ background: 'var(--surface-2)', border: `1px solid ${color}` }}
        >
          <div>
            <div className="text-xs text-[var(--muted)]">{t('reversion.signal')}</div>
            <div className="text-sm font-bold" style={{ color }}>
              {fadeLabel(signal.fade_signal, t)}
            </div>
          </div>
          <div className="text-right">
            <div className="text-xs text-[var(--muted)]">{t('reversion.confidence')}</div>
            <div className="text-sm font-bold">{(signal.confidence * 100).toFixed(0)}%</div>
          </div>
        </div>
      )}
    </div>
  );
}

export function ReversionGauge({ marketId }: ReversionGaugeProps) {
  const { t } = useT();
  const { data, isLoading, error } = useQuery({
    queryKey: ['reversion-signal', marketId],
    queryFn: () => reversionSignal(marketId),
    enabled: !!marketId,
  });

  if (isLoading) return <Skeleton className="h-32" />;
  if (error) return <ErrorState message={String(error)} />;
  if (!data || data.window_size < 2) {
    return (
      <EmptyState
        icon={<Activity className="w-5 h-5" />}
        title={t('reversion.insufficient_data')}
        description={t('reversion.insufficient_desc')}
      />
    );
  }

  return (
    <Card title={t('reversion.title')} description={t('reversion.description')} data-testid="reversion-gauge">
      <div className="space-y-4">
        <BollingerBands signal={data} t={t} />
        <ZScoreGauge signal={data} t={t} />
      </div>
    </Card>
  );
}
