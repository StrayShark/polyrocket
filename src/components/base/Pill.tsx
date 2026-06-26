import type { HTMLAttributes, ReactNode } from 'react';
import { cn } from '@/lib/cn';

export type PillKind = 'neutral' | 'bull' | 'bear' | 'warning' | 'accent' | 'muted';
/**
 * v0.119 — Cursor 合并:形状变体。
 *   - "square"(默认):沿用现有 4px 圆角(向后兼容)
 *   - "pill":按 Cursor `badge-pill` 规范使用 pill(9999px)圆角
 */
export type PillShape = 'square' | 'pill';

export interface PillProps extends HTMLAttributes<HTMLSpanElement> {
  kind?: PillKind;
  /** v0.119 — 形状变体。默认 "square" 以保持向后兼容。 */
  shape?: PillShape;
  /** v0.119 — 按 Cursor 规范采用大写 11px caption-uppercase 排版。 */
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
