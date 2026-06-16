import { useEffect, type ReactNode } from 'react';
import { X } from 'lucide-react';
import { useToastStore, type Toast } from '@/stores/toast-store';
import { cn } from '@/lib/cn';

const KIND_CLS: Record<Toast['kind'], string> = {
  info: 'border-accent/40 bg-accent/10 text-fg',
  success: 'border-bull/40 bg-bull/10 text-fg',
  warning: 'border-warning/40 bg-warning/10 text-fg',
  error: 'border-bear/40 bg-bear/10 text-fg',
};

export function ToastViewport(): ReactNode {
  const toasts = useToastStore((s) => s.toasts);
  const dismiss = useToastStore((s) => s.dismiss);

  return (
    <div
      className="fixed bottom-4 right-4 z-50 flex flex-col gap-2 w-[340px] pointer-events-none"
      aria-live="polite"
    >
      {toasts.map((t) => (
        <div
          key={t.id}
          className={cn(
            'pointer-events-auto rounded-md border px-3 py-2 shadow-lg flex items-start gap-2',
            KIND_CLS[t.kind],
          )}
        >
          <div className="flex-1 min-w-0">
            <div className="text-[12px] font-medium">{t.title}</div>
            {t.body && (
              <div className="text-[11px] text-fg-secondary mt-0.5 break-words">{t.body}</div>
            )}
          </div>
          <button
            onClick={() => dismiss(t.id)}
            className="text-muted hover:text-fg shrink-0"
            aria-label="dismiss"
          >
            <X className="w-3.5 h-3.5" />
          </button>
        </div>
      ))}
    </div>
  );
}

/** Auto-mount once at the App root. */
export function ToastHost() {
  useEffect(() => {
    // ensure store is initialized
    useToastStore.getState();
  }, []);
  return <ToastViewport />;
}
