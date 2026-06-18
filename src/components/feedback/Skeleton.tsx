import { cn } from '@/lib/cn';

/**
 * `Skeleton` —— 占位骨架（loading 态）。
 *
 * **用法**：
 *   `<Skeleton className="h-8 w-1/2" />` —— 8 行高 + 半宽的灰色块
 *   `<Skeleton className="h-32" />` —— 32 行高的卡片占位
 *
 * **`aria-hidden`**：screen reader 跳过（避免读出"loading"扰乱）。
 *
 * **`animate-pulse` 来自 Tailwind**：pulse 是 Tailwind 内建动画，**不**额外
 * 定义 keyframes（轻量）。
 */
export function Skeleton({ className }: { className?: string }) {
  return (
    <div
      className={cn('animate-pulse rounded bg-surface-2', className)}
      aria-hidden
    />
  );
}
