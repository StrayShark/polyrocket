import type { HTMLAttributes, ReactNode } from 'react';
import { cn } from '@/lib/cn';

export type PillKind = 'neutral' | 'bull' | 'bear' | 'warning' | 'accent' | 'muted';
/**
 * v0.119 — Cursor-merge: shape variants.
 *   - "square" (default): existing 4px rounded (backward-compat)
 *   - "pill": pill (9999px) rounded per Cursor `badge-pill` spec
 */
export type PillShape = 'square' | 'pill';

export interface PillProps extends HTMLAttributes<HTMLSpanElement> {
  kind?: PillKind;
  /** v0.119 — shape variant. Default "square" keeps backward compat. */
  shape?: PillShape;
  /** v0.119 — uppercase 11px caption-uppercase typography per Cursor. */
  uppercase?: boolean;
  children: ReactNode;
}

const KIND: Record<PillKind, string> = {
  neutral: 'bg-surface-2 text-fg border-border',
  bull: 'bg-bull/15 text-bull border-bull/30',
  bear: 'bg-bear/15 text-bear border-bear/30',
  warning: 'bg-warning/15 text-warning border-warning/30',
  accent: 'bg-accent/15 text-accent border-accent/30',
  muted: 'bg-transparent text-muted border-transparent',
};

const SHAPE: Record<PillShape, string> = {
  square: 'rounded',
  pill: 'rounded-pill',
};

export function Pill({
  kind = 'neutral',
  shape = 'square',
  uppercase = false,
  className,
  children,
  ...rest
}: PillProps) {
  return (
    <span
      className={cn(
        'inline-flex items-center gap-1 h-5 px-1.5 border',
        uppercase
          ? 'text-xs font-semibold tracking-caption-uppercase uppercase px-2.5'
          : 'text-[10px] font-medium',
        SHAPE[shape],
        KIND[kind],
        className,
      )}
      {...rest}
    >
      {children}
    </span>
  );
}
