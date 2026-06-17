/**
 * React error boundary.
 *
 * Catches render-time exceptions (componentDidCatch in v0.8b; not
 * concurrent-safe yet) and shows a recovery UI instead of a blank
 * page. Wrap the App in `<ErrorBoundary>` so a single broken route
 * doesn't white-screen the whole Tauri window.
 *
 * Uses class component (the only React-supported way to do this in
 * v18; the upcoming `use()` hook will replace it in v0.9+).
 */

import { Component, type ReactNode } from 'react';
import { AlertCircle, RotateCcw } from 'lucide-react';
import { classifyError, type AppErrorShape } from '@/lib/invoke-safe';

interface ErrorBoundaryProps {
  children: ReactNode;
  /** Optional fallback; defaults to the built-in recovery panel. */
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
    // Log to the dev console; in production the tauri-plugin-log will
    // pick this up via the global error handler.
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
        <p className="text-[13px] text-muted mb-3">
          {error.hint}
        </p>
        <pre className="text-[11px] text-muted font-mono text-left bg-bg p-3 rounded mb-4 overflow-x-auto">
          {error.kind}: {error.message}
        </pre>
        <button
          onClick={onReset}
          className="inline-flex items-center gap-1.5 text-[13px] text-accent hover:underline"
        >
          <RotateCcw className="w-3.5 h-3.5" />
          Try again
        </button>
      </div>
    </div>
  );
}
