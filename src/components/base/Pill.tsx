import type { HTMLAttributes, ReactNode } from 'react';
import { cn } from '@/lib/cn';

export type PillKind = 'neutral' | 'bull' | 'bear' | 'warning' | 'accent' | 'muted';

export interface PillProps extends HTMLAttributes<HTMLSpanElement> {
  kind?: PillKind;
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

export function Pill({ kind = 'neutral', className, children, ...rest }: PillProps) {
  return (
    <span
      className={cn(
        'inline-flex items-center gap-1 h-5 px-1.5 rounded text-[10px] font-medium border',
        KIND[kind],
        className,
      )}
      {...rest}
    >
      {children}
    </span>
  );
}
