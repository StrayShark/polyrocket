/**
 * BadgePill (v0.119 — Cursor-merge).
 *
 * Small uppercase badge following Cursor's `badge-pill` spec:
 *   - background: surface-strong (or accent/15 / bull/15 / bear/15)
 *   - text: ink (or accent / bull / bear)
 *   - typography: caption-uppercase (11px / 600 / +0.88px tracking / UPPERCASE)
 *   - rounded: pill (9999px)
 *   - padding: 4px 10px
 *
 * Use cases (v0.119):
 *   - Version badge ("v0.119")
 *   - Status badge ("LIVE", "BETA", "NEW")
 *   - Edge badge ("+7.6%", "-3.2%") — use bull/bear variant
 *   - Category tag (rare)
 *
 * v0.119 — replaces ad-hoc uppercase pill patterns scattered across UI.
 * Backward-compatible with existing `Pill` component (different API):
 *   - `Pill` (existing): 4px rounded, 10px font-medium, 6 kinds (bull/bear/warning/accent/neutral/muted)
 *   - `BadgePill` (new): pill (9999px) rounded, 11px caption-uppercase, 5 variants
 */

import type { HTMLAttributes, ReactNode } from 'react';
import { cn } from '@/lib/cn';

export type BadgePillVariant = 'neutral' | 'accent' | 'bull' | 'bear' | 'warning';

export interface BadgePillProps extends HTMLAttributes<HTMLSpanElement> {
  variant?: BadgePillVariant;
  children: ReactNode;
}

const VARIANT: Record<BadgePillVariant, string> = {
  neutral: 'bg-surface-2 text-fg border-border',
  accent: 'bg-accent/15 text-accent border-accent/30',
  bull: 'bg-bull/15 text-bull border-bull/30',
  bear: 'bg-bear/15 text-bear border-bear/30',
  warning: 'bg-warning/15 text-warning border-warning/30',
};

export function BadgePill({
  variant = 'neutral',
  className,
  children,
  ...rest
}: BadgePillProps) {
  return (
    <span
      data-testid="badge-pill"
      data-variant={variant}
      className={cn(
        'inline-flex items-center h-5 px-2.5 rounded-pill text-xs font-semibold tracking-caption-uppercase uppercase whitespace-nowrap border',
        VARIANT[variant],
        className,
      )}
      {...rest}
    >
      {children}
    </span>
  );
}
