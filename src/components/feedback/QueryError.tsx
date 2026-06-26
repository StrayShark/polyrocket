/**
 * Query 错误辅助:把 TanStack Query 的 error 转为统一的
 * ErrorState 面板。在每个路由的 error 分支使用,
 * 让 "出错了" 的 UI 处处一致。
 *
 * 用法:
 *   if (query.error) return <QueryError error={query.error} onRetry={query.refetch} />;
 */

import { AlertTriangle, RefreshCw } from 'lucide-react';
import type { AppErrorShape } from '@/lib/invoke-safe';
import { canRetry } from '@/lib/invoke-safe';

interface QueryErrorProps {
  error: unknown;
  onRetry?: () => void;
}

/** 尽力将任意抛出的值规整为我们的 shape。 */
function shape(e: unknown): AppErrorShape {
  if (e && typeof e === 'object' && 'kind' in e && 'message' in e && 'hint' in e) {
    return e as AppErrorShape;
  }
  const raw = e instanceof Error ? e.message : String(e);
  // 回退到一个合成的 unknown-error shape。
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
