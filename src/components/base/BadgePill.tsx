/**
 * BadgePill (v0.119 — Cursor 合并)。
 *
 * 按照 Cursor 的 `badge-pill` 规范实现的小号大写徽章:
 *   - 背景:surface-strong(或 accent/15 / bull/15 / bear/15)
 *   - 文字:ink(或 accent / bull / bear)
 *   - 排版:caption-uppercase(11px / 600 / +0.88px 字距 / UPPERCASE)
 *   - 圆角:pill(9999px)
 *   - 内边距:4px 10px
 *
 * 使用场景(v0.119):
 *   - 版本徽章("v0.119")
 *   - 状态徽章("LIVE"、"BETA"、"NEW")
 *   - 边际徽章("+7.6%"、"-3.2%")— 使用 bull/bear 变体
 *   - 分类标签(较少使用)
 *
 * v0.119 — 替换散落在 UI 中各处零散的大写 pill 样式。
 * 与现有 `Pill` 组件保持向后兼容(API 不同):
 *   - `Pill`(现有):4px 圆角,10px font-medium,6 种(bull/bear/warning/accent/neutral/muted)
 *   - `BadgePill`(新增):pill(9999px)圆角,11px caption-uppercase,5 种变体
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
