// polyrocket — BarChart(纯 SVG 垂直柱状图,v0.66b 密度)。
//
// 使用场景:
//   - /model-lab:Promote 历史 brier 分数
//   - /pnl:      按 status 拆分的 PnL
//   - /analysis:Signal 置信度分桶
//
// **为何采用纯 SVG**:不依赖图表库,与主题解耦(使用
// CSS 变量:--accent / --bull / --bear / --muted / --border)。
// 3 套主题共用此组件,仅颜色取值不同。
//
// **Y 轴**:始终绘制零线。零线之上的柱为正,以下为负。
// yMin / yMax 基于数据自动缩放,两端各留 10% padding。

import { useMemo } from 'react';
import { cn } from '@/lib/cn';

/** `<BarChart>` 组件的 Props。 */
export interface BarChartProps {
  /** 待渲染的柱,从左到右排列。 */
  data: Array<{ label: string; value: number; color?: string }>;
  /** SVG 宽度(像素)。默认 360。 */
  width?: number;
  /** SVG 高度(像素,仅图表区域,不含标签)。默认 120。 */
  height?: number;
  /**
   * 为 true 时,按数值正负着色:正值 = bull,
   * 负值 = bear。为 false 时,若提供了 `color` 则全部使用,
   * 否则使用 accent。
   */
  signedColor?: boolean;
  /** 根 `<svg>` 额外的 Tailwind class。 */
  className?: string;
}

/**
 * 纯 SVG 实现的垂直柱状图。为每条数据渲染一个 `<rect>`,
 * 并在 y=0 处绘制零线。每根柱的内部 `<title>` 用于 hover
 * 提示(浏览器原生 tooltip)。
 */
export function BarChart({
  data,
  width = 360,
  height = 120,
  signedColor = false,
  className,
}: BarChartProps) {
  // v0.66b — 根据数据自动缩放 yMin/yMax,两端各留 10% padding。
  // 始终包含 0,以保证即使数据全正或全负时零线依然有意义。
  const { yMin, yMax } = useMemo(() => {
    const values = data.map((d) => d.value);
    const min = Math.min(0, ...values);
    const max = Math.max(0, ...values);
    const yMin = min - (max - min) * 0.1;
    const yMax = max + (max - min) * 0.1;
    return { yMin, yMax };
  }, [data, width]);
  const yRange = yMax - yMin;
  // 将数值转换为 SVG 的 y 坐标(y 轴向下增长)。
  const zeroY = height - ((0 - yMin) / yRange) * height;
  const barW = (width - 8) / Math.max(data.length, 1) - 4;
  return (
    <svg width={width} height={height} className={cn('overflow-visible', className)}>
      {/* 零线 */}
      <line x1={0} y1={zeroY} x2={width} y2={zeroY} stroke="var(--border)" strokeWidth={1} />
      {data.map((b, i) => {
        // 柱位置:x 由索引决定,y 由值决定。
        const x = 4 + i * ((width - 8) / data.length);
        const yTop = height - ((b.value - yMin) / yRange) * height;
        // 对负值,柱从零线向下延伸。
        const y = Math.min(yTop, zeroY);
        const h = Math.abs(yTop - zeroY);
        // v0.66b — 颜色解析顺序:
        // 1. 单柱 `color` 属性(覆盖)
        // 2. signedColor 模式 → bull/bear 按符号
        // 3. 回落到 accent
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
