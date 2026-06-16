import { forwardRef, type InputHTMLAttributes } from 'react';
import { cn } from '@/lib/cn';

export interface InputProps extends InputHTMLAttributes<HTMLInputElement> {
  invalid?: boolean;
}

export const Input = forwardRef<HTMLInputElement, InputProps>(function Input(
  { className, invalid, ...rest },
  ref,
) {
  return (
    <input
      ref={ref}
      className={cn(
        'w-full h-8 px-2.5 rounded-md text-[13px] bg-surface text-fg border transition-colors',
        'placeholder:text-muted focus:outline-none focus:ring-2 focus:ring-accent',
        invalid ? 'border-bear' : 'border-border',
        className,
      )}
      {...rest}
    />
  );
});
