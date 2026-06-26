// @vitest-environment happy-dom
import type { ReactNode } from 'react';
import { describe, expect, it, vi, beforeAll, afterAll } from 'vitest';
import { render, screen } from '@testing-library/react';
import { ErrorBoundary } from './ErrorBoundary';
import { QueryError } from './QueryError';
import { classifyError } from '@/lib/invoke-safe';

// 总是抛出错误的组件——用于测试 ErrorBoundary。
function Bomb({ message = 'boom' }: { message?: string }): ReactNode {
  throw new Error(message);
}

// 正常渲染的组件。
function Safe(): ReactNode {
  return <div data-testid="safe">ok</div>;
}

describe('ErrorBoundary', () => {
  // happy-dom + console.error 噪声:屏蔽预期的 React 错误日志,
  // 让测试运行器的输出保持整洁。
  const origError = console.error;
  beforeAll(() => { console.error = vi.fn(); });
  afterAll(() => { console.error = origError; });

  it('renders children when no error is thrown', () => {
    render(
      <ErrorBoundary>
        <Safe />
      </ErrorBoundary>,
    );
    expect(screen.getByTestId('safe')).toBeInTheDocument();
    expect(screen.getByTestId('safe')).toHaveTextContent('ok');
  });

  it('catches a render error and shows the recovery panel', () => {
    render(
      <ErrorBoundary>
        <Bomb message="database error: out of space" />
      </ErrorBoundary>,
    );
    // 恢复面板文案
    expect(screen.getByText(/Something went wrong/i)).toBeInTheDocument();
    // 错误已被分类,kind+message 已渲染
    expect(screen.getByText(/db/i)).toBeInTheDocument();
    expect(screen.getByText(/out of space/i)).toBeInTheDocument();
    // Try again 按钮存在
    expect(screen.getByRole('button', { name: /try again/i })).toBeInTheDocument();
  });

  it('uses a custom fallback when provided', () => {
    const fallback = (err: ReturnType<typeof classifyError>) => (
      <div data-testid="custom">custom: {err.kind}</div>
    );
    render(
      <ErrorBoundary fallback={fallback}>
        <Bomb message="invalid input: bad" />
      </ErrorBoundary>,
    );
    const node = screen.getByTestId('custom');
    expect(node).toBeInTheDocument();
    expect(node).toHaveTextContent('custom: invalid');
  });
});

describe('QueryError', () => {
  it('renders the error kind and message', () => {
    const err = classifyError('not found: wallet wlt-xxx');
    render(<QueryError error={err} />);
    expect(screen.getByText(/not_found error/i)).toBeInTheDocument();
    expect(screen.getByText(/wlt-xxx/i)).toBeInTheDocument();
  });

  it('hides the retry button for non-retryable errors', () => {
    const err = classifyError('invalid input: bad input');
    render(<QueryError error={err} onRetry={() => {}} />);
    expect(screen.queryByRole('button', { name: /retry/i })).not.toBeInTheDocument();
  });

  it('shows the retry button for retryable errors', () => {
    const err = classifyError('database error: timeout');
    const onRetry = vi.fn();
    render(<QueryError error={err} onRetry={onRetry} />);
    const btn = screen.getByRole('button', { name: /retry/i });
    expect(btn).toBeInTheDocument();
    btn.click();
    expect(onRetry).toHaveBeenCalledOnce();
  });

  it('coerces unknown error shapes', () => {
    render(<QueryError error={new Error('plain old error')} />);
    expect(screen.getByText(/plain old error/i)).toBeInTheDocument();
  });

  it('coerces string errors', () => {
    render(<QueryError error="stringly-typed error" />);
    expect(screen.getByText(/stringly-typed error/i)).toBeInTheDocument();
  });
});
