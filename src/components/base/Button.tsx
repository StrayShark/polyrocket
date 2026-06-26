import { forwardRef, type ButtonHTMLAttributes, type ReactNode } from 'react';
import { cn } from '@/lib/cn';

export type ButtonVariant = 'primary' | 'secondary' | 'ghost' | 'danger' | 'success';
export type ButtonSize = 'xs' | 'sm' | 'md' | 'lg' | 'xl';

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
  size?: ButtonSize;
  loading?: boolean;
  iconLeft?: ReactNode;
  iconRight?: ReactNode;
}

const VARIANT: Record<ButtonVariant, string> = {
  primary: 'bg-accent text-white hover:bg-accent-hover border-transparent',
  secondary: 'bg-surface-2 text-fg hover:bg-surface-hover border-border',
  ghost: 'bg-transparent text-fg hover:bg-surface-hover border-transparent',
  danger: 'bg-bear text-white hover:opacity-90 border-transparent',
  success: 'bg-bull text-white hover:opacity-90 border-transparent',
};

const SIZE: Record<ButtonSize, string> = {
  // xs:24px — 仅用于行内标签样式的按钮(徽章、面包屑)。
  xs: 'h-6 px-2 text-[11px] gap-1',
  // sm:32px — Cursor 最小触摸目标。
  sm: 'h-8 px-2.5 text-[12px] gap-1.5',
  // md:36px — polyrocket 默认尺寸(桌面端紧凑布局)。
  md: 'h-9 px-3 text-body-sm gap-1.5',
  // lg:40px — 对齐 Cursor button-primary。
  lg: 'h-10 px-4 text-[14px] gap-2',
  // xl:44px — 对齐 Cursor button-download(大型 CTA / 焦点位)。
  xl: 'h-11 px-5 text-[15px] gap-2.5',
};

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(function Button(
  { className, variant = 'secondary', size = 'md', loading, iconLeft, iconRight, children, disabled, ...rest },
  ref,
) {
  return (
    <button
      ref={ref}
      disabled={disabled || loading}
      className={cn(
        'inline-flex items-center justify-center rounded-md border font-medium transition-colors duration-base ease-out-cubic',
        'active:scale-[0.98] transition-transform duration-fast ease-out-cubic',
        'disabled:opacity-50 disabled:cursor-not-allowed disabled:active:scale-100 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent',
        VARIANT[variant],
        SIZE[size],
        className,
      )}
      {...rest}
    >
      {loading ? <Spinner /> : iconLeft}
      {children}
      {iconRight}
    </button>
  );
});

function Spinner() {
  return (
    <span
      className="inline-block w-3 h-3 rounded-full border-2 border-current border-t-transparent animate-spin"
      aria-label="loading"
    />
  );
}
