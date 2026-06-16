import { useMemo } from 'react';
import { cn } from '@/lib/cn';

export interface BarChartProps {
  data: Array<{ label: string; value: number; color?: string }>;
  width?: number;
  height?: number;
  /** Color the bar by sign: positive = bull, negative = bear. */
  signedColor?: boolean;
  className?: string;
}

/** Vertical bar chart in pure SVG. */
export function BarChart({
  data,
  width = 360,
  height = 120,
  signedColor = false,
  className,
}: BarChartProps) {
  const { yMin, yMax } = useMemo(() => {
    const values = data.map((d) => d.value);
    const min = Math.min(0, ...values);
    const max = Math.max(0, ...values);
    const yMin = min - (max - min) * 0.1;
    const yMax = max + (max - min) * 0.1;
    return { yMin, yMax };
  }, [data, width]);
  const yRange = yMax - yMin;
  const zeroY = height - ((0 - yMin) / yRange) * height;
  const barW = (width - 8) / Math.max(data.length, 1) - 4;
  return (
    <svg width={width} height={height} className={cn('overflow-visible', className)}>
      {/* zero line */}
      <line x1={0} y1={zeroY} x2={width} y2={zeroY} stroke="var(--border)" strokeWidth={1} />
      {data.map((b, i) => {
        const x = 4 + i * ((width - 8) / data.length);
        const yTop = height - ((b.value - yMin) / yRange) * height;
        const y = Math.min(yTop, zeroY);
        const h = Math.abs(yTop - zeroY);
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
