// polyrocket — KpiCard(仪表盘上的 KPI 卡片,v0.66b 密度)。
//
// 所有仪表盘的原子单元。使用于:
//   - /dashboard:        4 张卡片(equity、open PnL、win rate、n signals)
//   - /pnl:              4 张卡片(total bets、won/lost、avg win、avg loss)
//   - /model-lab:        2 张卡片(best brier、total calls)
//   - /analysis:         各种 signal-strength 卡片
//
// **布局**(高 44px,占满卡片宽度):
//   ┌────────────────────────┐
//   │ LABEL              [I] │  ← 大写 muted,可带 icon
//   │ VALUE                  │  ← 22px 等宽,semibold
//   │ delta           hint   │  ← 彩色 delta + muted 提示
//   └────────────────────────┘
//
// **配色**与主题解耦,使用 CSS 变量。
// `delta.positive === true` → bull,`false` → bear,`undefined` → muted。

import type { LucideIcon } from 'lucide-react';
import { cn } from '@/lib/cn';
import { fmtPctInt } from '@/lib/format';

/** `<KpiCard>` 组件的 Props。 */
export interface KpiCardProps {
  /** 显示在 value 上方的大写简短 label。 */
  label: string;
  /** 已格式化好的主值(由调用方负责格式化)。 */
  value: string;
  /** 可选的辅助指标(例如 +2.3% delta 或子项计数)。 */
  delta?: { text: string; positive?: boolean } | null;
  /** 显示在右上角的 Lucide icon。 */
  icon?: LucideIcon;
  /** 显示在 delta 右侧的可选 sub-text。 */
  hint?: string;
  /** 根卡片额外的 Tailwind class。 */
  className?: string;
}

/**
 * 渲染单个 KPI 卡片。value 由调用方**预先格式化**
 * (此处不做任何数值格式化——传入前请使用
 * `fmtUsdc`、`fmtPctInt` 等)。
 */
export function KpiCard({ label, value, delta, icon: Icon, hint, className }: KpiCardProps) {
  return (
    <div className={cn('rounded-card border bg-surface border-border p-4', className)}>
      <div className="flex items-center justify-between mb-1.5">
        <span className="text-xs text-muted font-semibold uppercase tracking-caption-uppercase">
          {label}
        </span>
        {Icon && <Icon className="w-3.5 h-3.5 text-muted" />}
      </div>
      <div className="text-[22px] font-semibold text-fg font-mono leading-tight">
        {value}
      </div>
      <div className="flex items-center justify-between mt-1.5 h-4">
        {delta ? (
          <span
            className={cn(
              'text-[11px] font-mono',
              delta.positive === true ? 'text-bull' : delta.positive === false ? 'text-bear' : 'text-muted',
            )}
          >
            {delta.text}
          </span>
        ) : (
          <span />
        )}
        {hint && <span className="text-[10px] text-muted">{hint}</span>}
      </div>
    </div>
  );
}

/** `<KpiDeltaCard>` 的 Props——会自动计算 delta 的 `<KpiCard>`。 */
export interface KpiDeltaCardProps {
  label: string;
  current: number;
  previous: number;
  /**
   * 格式化函数:接收数字,返回显示字符串。
   * 默认 `fmtPctInt`(0.1234 → "12%")。
   */
  format?: (n: number) => string;
  /**
   * 反转"positive"颜色语义。用于"越低越好"的指标
   * (win rate 取反、Brier score 等)。
   */
  inverse?: boolean;
}

/**
 * 自动计算 delta 的 `<KpiCard>`。delta 为带符号差值
 * `current - previous`,带 `+` / `-` 前缀展示,
 * 并按 `positive` 着色(对"越低越好"的指标会尊重 `inverse`)。
 */
export function KpiDeltaCard({
  label,
  current,
  previous,
  format = (n) => fmtPctInt(n),
  inverse = false,
}: KpiDeltaCardProps) {
  const delta = current - previous;
  const positive = inverse ? delta < 0 : delta > 0;
  return (
    <KpiCard
      label={label}
      value={format(current)}
      delta={{ text: `${delta >= 0 ? '+' : ''}${format(delta)}`, positive }}
    />
  );
}
