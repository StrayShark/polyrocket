import { AlertCircle } from 'lucide-react';

/** `ErrorState` props。
 *   - `title` — 顶部标题（默认 "Something went wrong"）
 *   - `message` — 错误详情（`font-mono` 等宽字体显示）
 *   - `onRetry` — 「Try again」按钮回调（可空）
 */
export interface ErrorStateProps {
  title?: string;
  message: string;
  onRetry?: () => void;
}

/**
 * `ErrorState` —— 错误状态展示组件。
 *
 * **何时用**：query/mutation 失败时，**不**用 EmptyState（不是「无数据」）。
 *
 * **`message` 用 font-mono**：让 stack trace / JSON 错误对象排版整齐。
 *
 * **`onRetry` 不传** → 不显示「Try again」按钮（错误是终态，比如 404）。
 */
export function ErrorState({
  title = 'Something went wrong',
  message,
  onRetry,
}: ErrorStateProps) {
  return (
    <div className="flex flex-col items-center justify-center text-center py-8 px-4">
      <div className="w-10 h-10 rounded-full bg-bear/10 text-bear grid place-items-center mb-3">
        <AlertCircle className="w-5 h-5" />
      </div>
      <h3 className="text-[13px] font-medium text-fg">{title}</h3>
      <p className="text-[12px] text-muted mt-1 max-w-md font-mono break-words">{message}</p>
      {onRetry && (
        <button
          onClick={onRetry}
          className="mt-3 text-[12px] text-accent hover:underline"
        >
          Try again
        </button>
      )}
    </div>
  );
}
