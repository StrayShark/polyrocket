import { useMemo } from 'react';
import { cn } from '@/lib/cn';

export interface SparklineProps {
  values: number[];
  width?: number;
  height?: number;
  /** Color of the line and fill. Defaults to var(--accent). */
  color?: string;
  /** Show a small dot at the last point. */
  showLastDot?: boolean;
  className?: string;
  /** Optional reference lines (horizontal) */
  refLines?: number[];
  /** Pad the y-axis with this fraction (default 0.1 = 10%) */
  yPad?: number;
}

/**
 * Pure-SVG sparkline. No external charting dep — small and fast.
 * Auto-scales to data range; positive values get color, negative get bear.
 */
export function Sparkline({
  values,
  width = 120,
  height = 32,
  color,
  showLastDot = true,
  className,
  refLines = [],
  yPad = 0.1,
}: SparklineProps) {
  const path = useMemo(() => buildPath(values, width, height, yPad), [values, width, height, yPad]);
  if (values.length === 0) {
    return (
      <svg width={width} height={height} className={cn('text-muted', className)}>
        <line x1={0} y1={height / 2} x2={width} y2={height / 2} stroke="currentColor" strokeOpacity={0.2} />
      </svg>
    );
  }
  const last = values[values.length - 1];
  const first = values[0];
  const trend = last > first ? 'up' : last < first ? 'down' : 'flat';
  const lineColor = color ?? (trend === 'down' ? 'var(--bear)' : trend === 'up' ? 'var(--bull)' : 'var(--accent)');
  return (
    <svg width={width} height={height} className={cn('overflow-visible', className)}>
      {/* Reference lines */}
      {refLines.map((y, i) => (
        <line
          key={i}
          x1={0}
          y1={y}
          x2={width}
          y2={y}
          stroke="var(--border)"
          strokeDasharray="2 2"
          strokeWidth={0.5}
        />
      ))}
      {/* Fill */}
      <path
        d={`${path} L ${width},${height} L 0,${height} Z`}
        fill={lineColor}
        fillOpacity={0.15}
      />
      {/* Line */}
      <path d={path} fill="none" stroke={lineColor} strokeWidth={1.5} strokeLinejoin="round" strokeLinecap="round" />
      {showLastDot && values.length > 0 && (
        <circle
          cx={pathLast(values, width)}
          cy={pathLastY(values, height, yPad)}
          r={2}
          fill={lineColor}
        />
      )}
    </svg>
  );
}

function buildPath(values: number[], w: number, h: number, yPad: number): string {
  if (values.length === 0) return '';
  const min = Math.min(...values);
  const max = Math.max(...values);
  const range = max - min;
  const pad = range === 0 ? h * 0.1 : range * yPad;
  const yMin = min - pad;
  const yMax = max + pad;
  const yRange = yMax - yMin;
  const step = values.length > 1 ? w / (values.length - 1) : w;
  return values
    .map((v, i) => {
      const x = i * step;
      const y = h - ((v - yMin) / yRange) * h;
      return `${i === 0 ? 'M' : 'L'} ${x.toFixed(1)},${y.toFixed(1)}`;
    })
    .join(' ');
}

function pathLast(values: number[], w: number): number {
  return values.length > 1 ? w : w / 2;
}

function pathLastY(values: number[], h: number, yPad: number): number {
  const min = Math.min(...values);
  const max = Math.max(...values);
  const range = max - min;
  const pad = range === 0 ? h * 0.1 : range * yPad;
  const yMin = min - pad;
  const yMax = max + pad;
  const yRange = yMax - yMin;
  const last = values[values.length - 1];
  return h - ((last - yMin) / yRange) * h;
}

/**
 * Cumulative sum — used for the equity curve.
 */
export function cumulativeSum(values: number[]): number[] {
  const out: number[] = [];
  let sum = 0;
  for (const v of values) {
    sum += v;
    out.push(sum);
  }
  return out;
}
