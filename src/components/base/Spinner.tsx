import { cn } from '@/lib/cn';

export function Spinner({ className, size = 16 }: { className?: string; size?: number }) {
  return (
    <span
      className={cn(
        'inline-block rounded-full border-2 border-current border-t-transparent animate-spin',
        className,
      )}
      style={{ width: size, height: size }}
      role="status"
      aria-label="loading"
    />
  );
}
