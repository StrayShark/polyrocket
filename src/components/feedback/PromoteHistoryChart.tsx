/**
 * PromoteHistoryChart — v0.22a.
 *
 * Inline SVG sparkline of the user's model lifecycle:
 *
 *   Brier
 *   0.20 ┤
 *   0.18 ┤    ●─●
 *   0.16 ┤  ●     ╲
 *   0.14 ┤●        ●─●  ← best
 *         └──────────────── time →
 *
 * Each dot is a promotion. Oldest left, newest right.
 * Y axis is Brier (lower = better, so points lower on
 * the chart are better). Lines connect consecutive
 * promotions to make the trend visible.
 *
 * Why inline SVG (no library)?
 * - The chart is small (max 20 points, ~360x80 viewport)
 * - We don't need axes labels, tooltips, or interactivity
 *   (the existing PromoteHistory panel below provides
 *   the per-row detail; the chart is a glance)
 * - Adding recharts/visx/chart.js would ~10x the bundle
 *   for one small sparkline
 *
 * The component reuses the same `useQuery(['promote-history'],
 * listPromoteHistory)` as PromoteHistory so the data is
 * shared (no duplicate fetch). React Query dedupes.
 *
 * Failure modes (all return clean states):
 *  - 0 entries with brier → "No data yet" placeholder
 *  - 1 entry with brier → single dot, no lines
 *  - all entries have null brier → "No brier data" placeholder
 */
import { useQuery } from '@tanstack/react-query';
import { TrendingDown, TrendingUp, Minus } from 'lucide-react';
import { listPromoteHistory, type PromoteHistoryEntry } from '@/ipc';
import { useT } from '@/lib/i18n';
import { Skeleton } from '@/components/feedback/Skeleton';
import { ErrorState } from '@/components/feedback/ErrorState';

interface PromoteHistoryChartProps {
  /** Optional className passthrough (for spacing). */
  className?: string;
  /** SVG viewport width. Default 360. */
  width?: number;
  /** SVG viewport height. Default 80. */
  height?: number;
}

const PADDING = { top: 8, right: 12, bottom: 16, left: 32 };

interface PlottedPoint {
  /** Entry guaranteed to have best_brier: number (filtered upstream). */
  entry: PromoteHistoryEntry & { best_brier: number };
  x: number;
  y: number;
}

function brierColor(brier: number): string {
  // Mirror the LLM traffic lights from PromoteHistory:
  //   bull (green)  < 0.15
  //   warn (yellow) 0.15-0.20
  //   bear (red)    >= 0.20
  if (brier < 0.15) return '#22c55e';   // tailwind green-500
  if (brier < 0.20) return '#eab308';   // tailwind yellow-500
  return '#ef4444';                     // tailwind red-500
}

function plotPoints(
  entries: Array<PromoteHistoryEntry & { best_brier: number }>,
  width: number,
  height: number,
): PlottedPoint[] {
  if (entries.length === 0) return [];
  const xs = entries.map((e) => e.promoted_at_ms);
  const ys = entries.map((e) => e.best_brier);
  const xMin = Math.min(...xs);
  const xMax = Math.max(...xs);
  const xRange = xMax - xMin || 1;
  const yMin = Math.min(...ys);
  const yMax = Math.max(...ys);
  // Pad y range by 10% so dots don't sit on the edge
  const yPad = (yMax - yMin) * 0.1 || 0.01;
  const yLo = yMin - yPad;
  const yHi = yMax + yPad;
  const yRange = yHi - yLo;
  const plotW = width - PADDING.left - PADDING.right;
  const plotH = height - PADDING.top - PADDING.bottom;
  return entries.map((entry) => {
    const x = PADDING.left + ((entry.promoted_at_ms - xMin) / xRange) * plotW;
    const brier = entry.best_brier;
    // Y axis: Brier low = chart bottom (better), Brier high = chart top (worse)
    const y = PADDING.top + ((brier - yLo) / yRange) * plotH;
    return { entry, x, y };
  });
}

export function PromoteHistoryChart({
  className = '',
  width = 360,
  height = 80,
}: PromoteHistoryChartProps) {
  const { t } = useT();
  const { data, isLoading, error, refetch } = useQuery({
    queryKey: ['promote-history'],
    queryFn: () => listPromoteHistory(),
    staleTime: 30_000,
  });

  if (isLoading) {
    return (
      <div className={className} data-testid="promote-history-chart-loading">
        <Skeleton className="h-20" />
      </div>
    );
  }

  if (error) {
    return (
      <div className={className}>
        <ErrorState
          message={String(error)}
          onRetry={() => refetch()}
        />
      </div>
    );
  }

  // Filter to entries with brier data (v0.18 back-compat)
  const entries = (data?.entries ?? []).filter(
    (e): e is PromoteHistoryEntry & { best_brier: number } =>
      e.best_brier !== null,
  );

  if (entries.length === 0) {
    return (
      <div
        className={`text-[11px] text-muted italic ${className}`}
        data-testid="promote-history-chart-empty"
      >
        {t('promote.chart.empty')}
      </div>
    );
  }

  const points = plotPoints(entries, width, height);

  // Build the polyline path connecting consecutive points
  const pathD =
    points.length > 1
      ? points
          .map((p, i) => `${i === 0 ? 'M' : 'L'} ${p.x.toFixed(1)} ${p.y.toFixed(1)}`)
          .join(' ')
      : '';

  // Trend: compare the last point's brier to the first point's brier
  //   last < first → trending up (improving)  ↑ (since low brier is good)
  //   last > first → trending down (worsening)
  //   equal        → flat
  const firstBrier = entries[0].best_brier;
  const lastBrier = entries[entries.length - 1].best_brier;
  const trend: 'up' | 'down' | 'flat' =
    lastBrier < firstBrier - 0.001
      ? 'up'   // improving (lower brier = better)
      : lastBrier > firstBrier + 0.001
      ? 'down' // worsening
      : 'flat';

  return (
    <div className={className} data-testid="promote-history-chart" data-points={entries.length}>
      <div className="flex items-center justify-between mb-1.5">
        <div className="text-[10px] text-muted">
          {t('promote.chart.range', {
            min: firstBrier.toFixed(3),
            max: lastBrier.toFixed(3),
          })}
        </div>
        <div
          className="flex items-center gap-1 text-[10px]"
          data-testid="promote-history-chart-trend"
          data-trend={trend}
        >
          {trend === 'up' && <TrendingDown className="w-3 h-3 text-bull" />}
          {trend === 'down' && <TrendingUp className="w-3 h-3 text-bear" />}
          {trend === 'flat' && <Minus className="w-3 h-3 text-muted" />}
          <span className={trend === 'up' ? 'text-bull' : trend === 'down' ? 'text-bear' : 'text-muted'}>
            {t(`promote.chart.trend.${trend}`)}
          </span>
        </div>
      </div>
      <svg
        width={width}
        height={height}
        viewBox={`0 0 ${width} ${height}`}
        role="img"
        aria-label={t('promote.chart.title')}
        data-testid="promote-history-chart-svg"
      >
        {/* Y axis grid lines + labels at min/mid/max */}
        {(() => {
          const ys = entries.map((e) => e.best_brier);
          const yMin = Math.min(...ys);
          const yMax = Math.max(...ys);
          const yPad = (yMax - yMin) * 0.1 || 0.01;
          const yLo = yMin - yPad;
          const yHi = yMax + yPad;
          const yMid = (yLo + yHi) / 2;
          const plotH = height - PADDING.top - PADDING.bottom;
          const yToScreen = (v: number) =>
            PADDING.top + ((v - yLo) / (yHi - yLo)) * plotH;
          return (
            <g className="text-[8px] fill-current text-muted">
              {/* horizontal grid line at mid */}
              <line
                x1={PADDING.left}
                x2={width - PADDING.right}
                y1={yToScreen(yMid)}
                y2={yToScreen(yMid)}
                stroke="currentColor"
                strokeOpacity={0.15}
                strokeDasharray="2 2"
              />
              {/* Y axis labels: max (top), mid, min (bottom) */}
              <text x={4} y={yToScreen(yMax) + 3} textAnchor="start">
                {yMax.toFixed(3)}
              </text>
              <text x={4} y={yToScreen(yMid) + 3} textAnchor="start">
                {yMid.toFixed(3)}
              </text>
              <text x={4} y={yToScreen(yMin) + 3} textAnchor="start">
                {yMin.toFixed(3)}
              </text>
            </g>
          );
        })()}

        {/* Connecting line (only if > 1 point) */}
        {pathD && (
          <path
            d={pathD}
            fill="none"
            stroke="currentColor"
            strokeOpacity={0.4}
            strokeWidth={1}
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        )}

        {/* Per-entry dots */}
        {points.map((p, i) => (
          <circle
            key={`${p.entry.job_id}-${p.entry.promoted_at_ms}-${i}`}
            cx={p.x}
            cy={p.y}
            r={2.5}
            fill={brierColor(p.entry.best_brier)}
            data-testid="promote-history-chart-dot"
            data-job-id={p.entry.job_id}
            data-brier={p.entry.best_brier}
          />
        ))}
      </svg>
    </div>
  );
}
