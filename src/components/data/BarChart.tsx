// polyrocket — BarChart (pure-SVG vertical bar chart, v0.66b density).
//
// Used in:
//   - /model-lab: Promote history brier scores
//   - /pnl:      PnL by status breakdown
//   - /analysis: Signal confidence buckets
//
// **Why pure SVG**: no chart library, theme-agnostic (uses
// CSS variables: --accent / --bull / --bear / --muted / --border).
// The 3 themes share this component; only the color values change.
//
// **Y-axis**: zero line is always rendered. Bars above the
// zero line are positive, below are negative. The yMin / yMax
// are auto-scaled with 10% padding on each end.

import { useMemo } from 'react';
import { cn } from '@/lib/cn';

/** Props for the `<BarChart>` component. */
export interface BarChartProps {
  /** Bars to render, in left-to-right order. */
  data: Array<{ label: string; value: number; color?: string }>;
  /** SVG width in pixels. Default 360. */
  width?: number;
  /** SVG height in pixels (chart area only, not labels). Default 120. */
  height?: number;
  /**
   * When true, color each bar by sign: positive = bull,
   * negative = bear. When false, every bar uses `color` if
   * provided, else the accent color.
   */
  signedColor?: boolean;
  /** Extra Tailwind class names for the root <svg>. */
  className?: string;
}

/**
 * Vertical bar chart in pure SVG. Renders one `<rect>` per
 * datum plus a zero-line at y=0. Each bar carries a
 * `<title>` child for hover tooltips (browsers show
 * native tooltip on hover).
 */
export function BarChart({
  data,
  width = 360,
  height = 120,
  signedColor = false,
  className,
}: BarChartProps) {
  // v0.66b — auto-scale yMin/yMax to the data with 10%
  // padding on each end. Always include 0 so the zero line
  // is meaningful even for all-positive or all-negative data.
  const { yMin, yMax } = useMemo(() => {
    const values = data.map((d) => d.value);
    const min = Math.min(0, ...values);
    const max = Math.max(0, ...values);
    const yMin = min - (max - min) * 0.1;
    const yMax = max + (max - min) * 0.1;
    return { yMin, yMax };
  }, [data, width]);
  const yRange = yMax - yMin;
  // Convert a value to SVG y-coord (inverted: y=0 is top).
  const zeroY = height - ((0 - yMin) / yRange) * height;
  const barW = (width - 8) / Math.max(data.length, 1) - 4;
  return (
    <svg width={width} height={height} className={cn('overflow-visible', className)}>
      {/* zero line */}
      <line x1={0} y1={zeroY} x2={width} y2={zeroY} stroke="var(--border)" strokeWidth={1} />
      {data.map((b, i) => {
        // Bar position: x is fixed by index, y is value.
        const x = 4 + i * ((width - 8) / data.length);
        const yTop = height - ((b.value - yMin) / yRange) * height;
        // For negative values, the bar extends DOWN from zero.
        const y = Math.min(yTop, zeroY);
        const h = Math.abs(yTop - zeroY);
        // v0.66b — color resolution order:
        // 1. per-bar `color` prop (override)
        // 2. signedColor mode → bull/bear by sign
        // 3. fallback to accent
        const color = b.color ?? (signedColor ? (b.value >= 0 ? 'var(--bull)' : 'var(--bear)') : 'var(--accent)');
        return (
          <g key={i}>
            <rect
              x={x}
              y={y}
              width={barW}
              height={Math.max(h, 1)}
              fill={color}
              fillOpacity={0.8}
              rx={1}
            >
              <title>{`${b.label}: ${b.value.toFixed(2)}`}</title>
            </rect>
            <text
              x={x + barW / 2}
              y={height + 12}
              textAnchor="middle"
              fontSize={9}
              fill="var(--muted)"
            >
              {b.label}
            </text>
          </g>
        );
      })}
    </svg>
  );
}
