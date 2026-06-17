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
 * v0.29a — hover tooltips on each dot. Two layers:
 *  1. `<title>` element inside each `<circle>` —
 *     native browser tooltip (works without JS,
 *     screen-reader accessible).
 *  2. A custom positioned `<g>` tooltip on hover —
 *     shows full info: model_version, brier, "Xh ago",
 *     and the trial badge if present (best trial vs
 *     trial #N).
 *
 * The native `<title>` is the a11y path; the custom
 * tooltip is the styled-detail path. Both fire on the
 * same hover event.
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
import { useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { TrendingDown, TrendingUp, Minus } from 'lucide-react';
import { listPromoteHistory, type PromoteHistoryEntry } from '@/ipc';
import { useT } from '@/lib/i18n';
import { fmtRelativeTime } from '@/lib/format';
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

/** v0.29a — derive the trial badge label for a history entry.
 *  Returns the i18n key for the badge (best trial / trial N / none). */
function trialBadgeKey(entry: PromoteHistoryEntry): string | null {
  if (entry.trial_index === null || entry.trial_index === undefined) {
    return 'promote.history.trial_best'; // best trial (no -t{N} suffix)
  }
  return `promote.history.trial_n.${entry.trial_index}`;
}

export function PromoteHistoryChart({
  className = '',
  width = 360,
  height = 80,
}: PromoteHistoryChartProps) {
  const { t } = useT();
  // v0.29a — track which dot is hovered for the custom tooltip.
  // null = no hover (tooltip hidden).
  const [hoveredIndex, setHoveredIndex] = useState<number | null>(null);
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

  // v0.29a — the hovered point (if any), used to render the
  // custom tooltip. Positioned above the dot with a small
  // upward offset so it doesn't overlap the dot.
  const hoveredPoint =
    hoveredIndex !== null && hoveredIndex < points.length
      ? points[hoveredIndex]
      : null;

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
        onMouseLeave={() => setHoveredIndex(null)}
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

        {/* v0.29a — invisible larger hit areas for easier hovering.
           Each dot has a 12px-radius transparent circle on top so
           users don't need pixel-perfect aim. The hit area fires
           the same hover handlers as the visible dot. */}
        {points.map((p, i) => (
          <circle
            key={`hit-${p.entry.job_id}-${p.entry.promoted_at_ms}-${i}`}
            cx={p.x}
            cy={p.y}
            r={6}
            fill="transparent"
            data-testid="promote-history-chart-hit"
            data-hit-index={i}
            style={{ cursor: 'pointer' }}
            onMouseEnter={() => setHoveredIndex(i)}
          />
        ))}

        {/* Per-entry dots with native <title> for a11y. */}
        {points.map((p, i) => (
          <circle
            key={`${p.entry.job_id}-${p.entry.promoted_at_ms}-${i}`}
            cx={p.x}
            cy={p.y}
            r={hoveredIndex === i ? 3.5 : 2.5}
            fill={brierColor(p.entry.best_brier)}
            stroke={hoveredIndex === i ? 'currentColor' : 'none'}
            strokeWidth={hoveredIndex === i ? 1 : 0}
            data-testid="promote-history-chart-dot"
            data-job-id={p.entry.job_id}
            data-brier={p.entry.best_brier}
            data-trial-index={p.entry.trial_index ?? ''}
          >
            {/* Native browser tooltip — works without JS, accessible to
                 screen readers. The custom tooltip below is the styled
                 detail; this is the always-on fallback. */}
            <title>
              {[
                p.entry.model_version,
                `Brier ${p.entry.best_brier.toFixed(3)}`,
                fmtRelativeTime(p.entry.promoted_at_ms),
              ]
                .filter(Boolean)
                .join(' · ')}
            </title>
          </circle>
        ))}

        {/* v0.29a — custom positioned tooltip on hover. Positioned
            above the hovered dot; clamped to the chart viewport so
            it doesn't overflow on the right edge. */}
        {hoveredPoint && (() => {
          const tipW = 180;
          const tipH = 56;
          // Position above the dot; clamp to viewport
          const tipX = Math.max(
            PADDING.left,
            Math.min(width - PADDING.right - tipW, hoveredPoint.x - tipW / 2),
          );
          const tipY = Math.max(2, hoveredPoint.y - tipH - 4);
          const trialKey = trialBadgeKey(hoveredPoint.entry);
          return (
            <g
              data-testid="promote-history-chart-tooltip"
              data-job-id={hoveredPoint.entry.job_id}
              data-brier={hoveredPoint.entry.best_brier}
              data-trial-index={hoveredPoint.entry.trial_index ?? ''}
            >
              {/* Dark background rect with rounded corners */}
              <rect
                x={tipX}
                y={tipY}
                width={tipW}
                height={tipH}
                rx={4}
                fill="var(--bg, #1e293b)"
                fillOpacity={0.95}
                stroke="var(--border, #334155)"
                strokeWidth={1}
              />
              {/* Line 1: model_version (truncated if too long) */}
              <text
                x={tipX + 8}
                y={tipY + 16}
                fontSize="10"
                fontFamily="monospace"
                fill="var(--fg, #e2e8f0)"
              >
                {hoveredPoint.entry.model_version.length > 24
                  ? `${hoveredPoint.entry.model_version.slice(0, 24)}…`
                  : hoveredPoint.entry.model_version}
              </text>
              {/* Line 2: brier value */}
              <text
                x={tipX + 8}
                y={tipY + 30}
                fontSize="10"
                fill="var(--fg-muted, #94a3b8)"
              >
                Brier {hoveredPoint.entry.best_brier.toFixed(3)}
              </text>
              {/* Line 3: "promoted Xh ago" + trial badge */}
              <text
                x={tipX + 8}
                y={tipY + 44}
                fontSize="9"
                fill="var(--fg-muted, #94a3b8)"
              >
                {`promoted ${fmtRelativeTime(hoveredPoint.entry.promoted_at_ms)}`}
                {trialKey ? ` · ${t(trialKey)}` : ''}
              </text>
            </g>
          );
        })()}
      </svg>
    </div>
  );
}
