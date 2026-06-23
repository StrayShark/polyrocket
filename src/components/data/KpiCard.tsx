// polyrocket — KpiCard (KPI tile for dashboards, v0.66b density).
//
// The atomic unit of every dashboard. Used in:
//   - /dashboard:        4 cards (equity, open PnL, win rate, n signals)
//   - /pnl:              4 cards (total bets, won/lost, avg win, avg loss)
//   - /model-lab:        2 cards (best brier, total calls)
//   - /analysis:         various signal-strength cards
//
// **Layout** (44px tall, full card width):
//   ┌────────────────────────┐
//   │ LABEL              [I] │  ← uppercase muted, optional icon
//   │ VALUE                  │  ← 22px monospace, semibold
//   │ delta           hint   │  ← colored delta + muted hint
//   └────────────────────────┘
//
// **Color** is theme-agnostic; uses CSS variables.
// `delta.positive === true` → bull, `false` → bear, `undefined` → muted.

import type { LucideIcon } from 'lucide-react';
import { cn } from '@/lib/cn';
import { fmtPctInt } from '@/lib/format';

/** Props for the `<KpiCard>` component. */
export interface KpiCardProps {
  /** Short uppercase label shown above the value. */
  label: string;
  /** Pre-formatted primary value (caller does formatting). */
  value: string;
  /** Optional secondary metric (e.g. +2.3% delta, or sub-count). */
  delta?: { text: string; positive?: boolean } | null;
  /** Lucide icon shown top-right. */
  icon?: LucideIcon;
  /** Optional sub-text shown right of the delta. */
  hint?: string;
  /** Extra Tailwind class names for the root card. */
  className?: string;
}

/**
 * Render a single KPI tile. The value is **pre-formatted** by
 * the caller (we don't do any number formatting here — use
 * `fmtUsdc`, `fmtPctInt`, etc. before passing in).
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

/** Props for `<KpiDeltaCard>` — a `<KpiCard>` that auto-computes the delta. */
export interface KpiDeltaCardProps {
  label: string;
  current: number;
  previous: number;
  /**
   * Format helper: takes a number and returns a display string.
   * Default is `fmtPctInt` (0.1234 → "12%").
   */
  format?: (n: number) => string;
  /**
   * Invert the "positive" color. Use for metrics where
   * lower is better (win rate inverted, Brier score, etc.).
   */
  inverse?: boolean;
}

/**
 * `<KpiCard>` with auto-computed delta. The delta is the
 * signed difference `current - previous`, displayed with
 * `+` / `-` prefix and colored by `positive` (which respects
 * `inverse` for "lower is better" metrics).
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
