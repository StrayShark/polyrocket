/**
 * PromoteHistoryChart —— v0.22a。
 *
 * 用户 model 生命周期的内联 SVG sparkline:
 *
 *   Brier
 *   0.20 ┤
 *   0.18 ┤    ●─●
 *   0.16 ┤  ●     ╲
 *   0.14 ┤●        ●─●  ← best
 *         └──────────────── time →
 *
 * 每个点是一次 promote。最旧在左,最新在右。
 * Y 轴是 Brier(越低越好,因此图中位置越低越好)。
 * 用线连接相邻 promote,让趋势更直观。
 *
 * v0.29a —— 每个点的 hover tooltip。两层:
 *  1. `<title>` 元素在每个 `<circle>` 内 —— 浏览器
 *     原生 tooltip(无需 JS 可用,屏幕阅读器可读)。
 *  2. hover 时,自定义定位的 `<g>` tooltip —— 显示
 *     完整信息:model_version、brier、"Xh ago",
 *     以及 trial 徽章(若有)(best trial vs trial #N)。
 *
 * 原生 `<title>` 是 a11y 路径;自定义 tooltip 是
 * 带样式的详情路径。两者在同一个 hover 事件触发。
 *
 * 为何用内联 SVG(不引入库)?
 * - 图表很小(最多 20 个点,~360x80 viewport)
 * - 我们不需要坐标轴标签、tooltip、交互(下方
 *   现有的 PromoteHistory 面板提供每行详情;
 *   图表仅用于概览)
 * - 引入 recharts/visx/chart.js 会让 bundle 涨 10 倍
 *   却只为一个 sparkline
 *
 * 组件复用与 PromoteHistory 相同的
 * `useQuery(['promote-history'], listPromoteHistory)`,
 * 共享数据(不会重复拉取)。React Query 会去重。
 *
 * 失败模式(均返回干净状态):
 *  - 0 条带 brier 的 entry → "No data yet" 占位
 *  - 1 条带 brier 的 entry → 单个点,无连线
 *  - 全部 entry 的 brier 为 null → "No brier data" 占位
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
  /** 可选的 className 透传(用于间距)。 */
  className?: string;
  /** SVG viewport 宽度。默认 360。 */
  width?: number;
  /** SVG viewport 高度。默认 80。 */
  height?: number;
}

const PADDING = { top: 8, right: 12, bottom: 16, left: 32 };

interface PlottedPoint {
  /** 已在上游过滤,保证 best_brier 为 number。 */
  entry: PromoteHistoryEntry & { best_brier: number };
  x: number;
  y: number;
}

function brierColor(brier: number): string {
  // 与 PromoteHistory 的 LLM 红绿灯保持一致:
  //   bull(绿)  < 0.15
  //   warn(黄)  0.15-0.20
  //   bear(红)  >= 0.20
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
  // y 轴范围上下各留 10% 边距,避免点贴近边缘
  const yPad = (yMax - yMin) * 0.1 || 0.01;
  const yLo = yMin - yPad;
  const yHi = yMax + yPad;
  const yRange = yHi - yLo;
  const plotW = width - PADDING.left - PADDING.right;
  const plotH = height - PADDING.top - PADDING.bottom;
  return entries.map((entry) => {
    const x = PADDING.left + ((entry.promoted_at_ms - xMin) / xRange) * plotW;
    const brier = entry.best_brier;
    // Y 轴:Brier 低 = 图表底部(更好),Brier 高 = 图表顶部(更差)
    const y = PADDING.top + ((brier - yLo) / yRange) * plotH;
    return { entry, x, y };
  });
}

/** v0.29a —— 为 history entry 派生 trial 徽章 label。
 *  返回徽章的 i18n key(best trial / trial N / 无)。 */
function trialBadgeKey(entry: PromoteHistoryEntry): string | null {
  if (entry.trial_index === null || entry.trial_index === undefined) {
    return 'promote.history.trial_best'; // best trial(无 -t{N} 后缀)
  }
  return `promote.history.trial_n.${entry.trial_index}`;
}

export function PromoteHistoryChart({
  className = '',
  width = 360,
  height = 80,
}: PromoteHistoryChartProps) {
  const { t } = useT();
  // v0.29a —— 记录当前 hover 的点,用于自定义 tooltip。
  // null = 无 hover(隐藏 tooltip)。
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

  // 过滤出含 brier 数据的 entry(v0.18 向后兼容)
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

  // 构建连接相邻点的折线路径
  const pathD =
    points.length > 1
      ? points
          .map((p, i) => `${i === 0 ? 'M' : 'L'} ${p.x.toFixed(1)} ${p.y.toFixed(1)}`)
          .join(' ')
      : '';

  // 趋势:比较最后一点与第一点的 brier
  //   last < first → 趋势向上(改善)↑(因 brier 低是好的)
  //   last > first → 趋势向下(恶化)
  //   equal        → 持平
  const firstBrier = entries[0].best_brier;
  const lastBrier = entries[entries.length - 1].best_brier;
  const trend: 'up' | 'down' | 'flat' =
    lastBrier < firstBrier - 0.001
      ? 'up'   // 改善(更低 brier = 更好)
      : lastBrier > firstBrier + 0.001
      ? 'down' // 恶化
      : 'flat';

  // v0.29a —— 当前 hover 的点(若有),用于渲染
  // 自定义 tooltip。定位在点的上方并略微上移,
  // 避免与点重叠。
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
        {/* Y 轴网格线 + 最小/中/最大 标签 */}
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
              {/* 中点处的水平网格线 */}
              <line
                x1={PADDING.left}
                x2={width - PADDING.right}
                y1={yToScreen(yMid)}
                y2={yToScreen(yMid)}
                stroke="currentColor"
                strokeOpacity={0.15}
                strokeDasharray="2 2"
              />
              {/* Y 轴标签:max(顶部)、mid、min(底部) */}
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

        {/* 连线(仅当 > 1 个点时) */}
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

        {/* v0.29a —— 隐形放大 hit area,方便 hover。
           每个点上方有一个 12px 半径的透明圆,
           用户不需要像素级精准就能 hover 到。hit area
           与可见点使用相同的 hover 处理器。 */}
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

        {/* 每个 entry 的点,带原生 <title> 满足 a11y。 */}
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
            {/* 浏览器原生 tooltip —— 无需 JS 可用,
                 屏幕阅读器可读。下方的自定义 tooltip 是
                 带样式的详情;此处是始终开启的兜底。 */}
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

        {/* v0.29a —— hover 时自定义定位的 tooltip。位于
            hover 点的上方;限制在图表 viewport 内,避免
            在右侧越界。 */}
        {hoveredPoint && (() => {
          const tipW = 180;
          const tipH = 56;
          // 定位在点的上方;clamp 到 viewport
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
              {/* 深色圆角背景矩形 */}
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
              {/* 第 1 行:model_version(过长则截断) */}
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
              {/* 第 2 行:brier 值 */}
              <text
                x={tipX + 8}
                y={tipY + 30}
                fontSize="10"
                fill="var(--fg-muted, #94a3b8)"
              >
                Brier {hoveredPoint.entry.best_brier.toFixed(3)}
              </text>
              {/* 第 3 行:"promoted Xh ago" + trial 徽章 */}
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
