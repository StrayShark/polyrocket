import type { LucideIcon } from 'lucide-react';
import { cn } from '@/lib/cn';
import { fmtPctInt } from '@/lib/format';

export interface KpiCardProps {
  label: string;
  value: string;
  /** Optional secondary metric (e.g. +2.3% delta, or sub-count) */
  delta?: { text: string; positive?: boolean } | null;
  icon?: LucideIcon;
  hint?: string;
  className?: string;
}

export function KpiCard({ label, value, delta, icon: Icon, hint, className }: KpiCardProps) {
  return (
    <div className={cn('rounded-lg border bg-surface border-border p-4', className)}>
      <div className="flex items-center justify-between mb-1.5">
        <span className="text-[11px] text-muted font-medium uppercase tracking-wide">
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

export interface KpiDeltaCardProps {
  label: string;
  current: number;
  previous: number;
  /** Format helper: 0.1234 → 12.34 */
  format?: (n: number) => string;
  inverse?: boolean; // true for win_rate/brier (lower=better)
}

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
