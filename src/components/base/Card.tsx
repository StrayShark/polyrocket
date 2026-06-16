import type { HTMLAttributes, ReactNode } from 'react';
import { cn } from '@/lib/cn';

export interface CardProps extends Omit<HTMLAttributes<HTMLDivElement>, 'title'> {
  title?: ReactNode;
  description?: ReactNode;
  action?: ReactNode;
  padding?: 'none' | 'sm' | 'md' | 'lg';
}

const PAD = { none: 'p-0', sm: 'p-3', md: 'p-4', lg: 'p-5' } as const;

export function Card({
  title,
  description,
  action,
  padding = 'md',
  className,
  children,
  ...rest
}: CardProps) {
  return (
    <div
      className={cn('rounded-lg border bg-surface border-border', className)}
      {...rest}
    >
      {(title || action) && (
        <div className="flex items-start justify-between gap-2 px-4 pt-3">
          <div>
            {title && (
              <h3 className="text-[13px] font-semibold text-fg">{title}</h3>
            )}
            {description && (
              <p className="text-[11px] text-muted mt-0.5">{description}</p>
            )}
          </div>
          {action && <div className="shrink-0">{action}</div>}
        </div>
      )}
      <div className={PAD[padding]}>{children}</div>
    </div>
  );
}
