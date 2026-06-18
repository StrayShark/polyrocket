// polyrocket — Toast viewport (v0.66b density).
//
// Renders the global toast queue (info / success / warning / error)
// in a fixed bottom-right stack. Pairs with `useToastStore` from
// `@/stores/toast-store` — toasts are pushed via `toast.info()` /
// `.success()` / `.warning()` / `.error()` and rendered here.
//
// **Why a separate ToastViewport + ToastHost pair**: the host
// is a thin auto-mount component placed at the App root (so
// the listener subscribes once per app load). The viewport is
// the actual UI. Splitting them lets unit tests render the
// viewport without bootstrapping the whole app shell.
//
// **Color tokens** — the four `KIND_CLS` entries are the only
// place a toast's color is decided. If you add a new ToastKind,
// add an entry here AND in `stores/toast-store.ts` (the kind
// union type).

import { useEffect, type ReactNode } from 'react';
import { X } from 'lucide-react';
import { useToastStore, type Toast } from '@/stores/toast-store';
import { cn } from '@/lib/cn';

// v0.66b — color tokens per kind. The 4 themes share this
// mapping; the actual color values are pulled from the active
// theme's CSS variables (bg-accent, bg-bull, etc.).
const KIND_CLS: Record<Toast['kind'], string> = {
  info: 'border-accent/40 bg-accent/10 text-fg',
  success: 'border-bull/40 bg-bull/10 text-fg',
  warning: 'border-warning/40 bg-warning/10 text-fg',
  error: 'border-bear/40 bg-bear/10 text-fg',
};

/**
 * Render the current toast queue as a fixed bottom-right
 * stack. The host element is `aria-live="polite"` so screen
 * readers announce new toasts (without stealing focus).
 *
 * Toasts are stacked vertically (column-flex) and the
 * newest is at the bottom (matching macOS notification
 * ordering). Each toast has a dismiss button (X) that
 * calls `useToastStore.dismiss(id)`.
 */
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

/**
 * Auto-mount once at the App root. The `useEffect` is a
 * no-op apart from touching `getState()` to ensure the
 * zustand store is initialized in the React tree.
 *
 * `ToastHost` wraps `ToastViewport` so callers only need
 * to drop `<ToastHost />` once. Tests that need to render
 * the viewport directly should import `ToastViewport`
 * instead of `ToastHost`.
 */
export function ToastHost() {
  useEffect(() => {
    // ensure store is initialized
    useToastStore.getState();
  }, []);
  return <ToastViewport />;
}
