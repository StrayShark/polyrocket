import { forwardRef, type ButtonHTMLAttributes } from 'react';
import { cn } from '@/lib/cn';

export interface ToggleProps extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, 'onChange'> {
  checked: boolean;
  onChange: (v: boolean) => void;
  label?: string;
}

export const Toggle = forwardRef<HTMLButtonElement, ToggleProps>(function Toggle(
  { checked, onChange, label, className, ...rest },
  ref,
) {
  return (
    <button
      ref={ref}
      type="button"
      role="switch"
      aria-checked={checked}
      onClick={() => onChange(!checked)}
      className={cn(
        'relative inline-flex h-5 w-9 shrink-0 cursor-pointer rounded-full border transition-colors duration-base ease-out-cubic',
        checked ? 'bg-accent border-accent' : 'bg-surface-2 border-border',
        className,
      )}
      {...rest}
    >
      <span
        className={cn(
          'pointer-events-none inline-block h-4 w-4 transform rounded-full bg-white shadow transition-transform duration-base ease-out-cubic',
          checked ? 'translate-x-[18px]' : 'translate-x-[1px] translate-y-[1px]',
        )}
      />
      {label && <span className="sr-only">{label}</span>}
    </button>
  );
});
