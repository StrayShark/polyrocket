/**
 * React 错误边界。
 *
 * 捕获渲染期异常(v0.8b 的 componentDidCatch;目前尚不支持并发模式),
 * 并显示恢复 UI,而不是空白页。用 `<ErrorBoundary>` 包裹 App,
 * 防止单个路由崩溃导致整个 Tauri 窗口白屏。
 *
 * 使用 class 组件(v18 中这是 React 唯一支持的方式;
 * 即将推出的 `use()` hook 将在 v0.9+ 替换它)。
 */

import { Component, type ReactNode } from 'react';
import { AlertCircle, RotateCcw } from 'lucide-react';
import { classifyError, type AppErrorShape } from '@/lib/invoke-safe';

interface ErrorBoundaryProps {
  children: ReactNode;
  /** 可选 fallback;默认为内置的恢复面板。 */
  fallback?: (err: AppErrorShape, reset: () => void) => ReactNode;
}

interface ErrorBoundaryState {
  error: AppErrorShape | null;
}

export class ErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
  state: ErrorBoundaryState = { error: null };

  static getDerivedStateFromError(error: unknown): ErrorBoundaryState {
    const raw = error instanceof Error ? error.message : String(error);
    return { error: classifyError(raw) };
  }

  componentDidCatch(error: unknown, info: { componentStack?: string }) {
    // 输出到开发控制台;生产环境由 tauri-plugin-log
    // 通过全局错误处理接管。
    // eslint-disable-next-line no-console
    console.error('[ErrorBoundary]', error, info.componentStack);
  }

  reset = () => this.setState({ error: null });

  render() {
    if (this.state.error) {
      if (this.props.fallback) {
        return this.props.fallback(this.state.error, this.reset);
      }
      return <DefaultErrorPanel error={this.state.error} onReset={this.reset} />;
    }
    return this.props.children;
  }
}

function DefaultErrorPanel({ error, onReset }: { error: AppErrorShape; onReset: () => void }) {
  return (
    <div
      role="alert"
      className="min-h-screen flex items-center justify-center bg-bg text-fg p-6"
    >
      <div className="max-w-md w-full bg-surface border border-border rounded-lg p-6 text-center">
        <div className="w-12 h-12 rounded-full bg-bear/10 text-bear grid place-items-center mx-auto mb-3">
          <AlertCircle className="w-6 h-6" />
        </div>
        <h1 className="text-base font-semibold mb-1">Something went wrong</h1>
        <p className="text-body-sm text-muted mb-3">
          {error.hint}
        </p>
        <pre className="text-[11px] text-muted font-mono text-left bg-bg p-3 rounded mb-4 overflow-x-auto">
          {error.kind}: {error.message}
        </pre>
        <button
          onClick={onReset}
          className="inline-flex items-center gap-1.5 text-body-sm text-accent hover:underline"
        >
          <RotateCcw className="w-3.5 h-3.5" />
          Try again
        </button>
      </div>
    </div>
  );
}
