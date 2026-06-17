/**
 * Query-error helper: turn a TanStack Query error into a consistent
 * ErrorState panel. Use in every route's error branch so the
 * "something failed" UI is identical everywhere.
 *
 * Usage:
 *   if (query.error) return <QueryError error={query.error} onRetry={query.refetch} />;
 */

import { AlertTriangle, RefreshCw } from 'lucide-react';
import type { AppErrorShape } from '@/lib/invoke-safe';
import { canRetry } from '@/lib/invoke-safe';

interface QueryErrorProps {
  error: unknown;
  onRetry?: () => void;
}

/** Best-effort coercion of any thrown value into our shape. */
function shape(e: unknown): AppErrorShape {
  if (e && typeof e === 'object' && 'kind' in e && 'message' in e && 'hint' in e) {
    return e as AppErrorShape;
  }
  const raw = e instanceof Error ? e.message : String(e);
  // Fall back to a synthetic unknown-error shape.
  return {
    kind: 'unknown',
    message: raw,
    hint: 'An unknown error occurred.',
    retryable: true,
    raw,
  };
}

export function QueryError({ error, onRetry }: QueryErrorProps) {
  const e = shape(error);
  return (
    <div role="alert" className="border border-bear/30 bg-bear/5 rounded p-3 my-2">
      <div className="flex items-start gap-2">
        <AlertTriangle className="w-4 h-4 text-bear mt-0.5 shrink-0" />
        <div className="flex-1 min-w-0">
          <div className="text-[12px] font-medium text-bear">
            {e.kind} error
          </div>
          <div className="text-[11px] text-muted font-mono mt-0.5 break-words">
            {e.message}
          </div>
          <div className="text-[11px] text-muted mt-1">
            {e.hint}
          </div>
        </div>
        {onRetry && canRetry(e) && (
          <button
            onClick={onRetry}
            className="text-[11px] text-accent hover:underline inline-flex items-center gap-1 shrink-0"
          >
            <RefreshCw className="w-3 h-3" />
            retry
          </button>
        )}
      </div>
    </div>
  );
}
