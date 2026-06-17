// @vitest-environment happy-dom
import { describe, expect, it, vi, beforeAll, afterAll } from 'vitest';
import { render, screen } from '@testing-library/react';
import { ErrorBoundary } from './ErrorBoundary';
import { QueryError } from './QueryError';
import { classifyError } from '@/lib/invoke-safe';

// Component that always throws — used to test ErrorBoundary.
function Bomb({ message = 'boom' }: { message?: string }) {
  throw new Error(message);
}

// Component that renders fine.
function Safe() {
  return <div data-testid="safe">ok</div>;
}

describe('ErrorBoundary', () => {
  // happy-dom + console.error noise: silence the expected React error logs
  // so the test runner output stays clean.
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
    // Recovery panel copy
    expect(screen.getByText(/Something went wrong/i)).toBeInTheDocument();
    // The error is classified and the kind+message are rendered
    expect(screen.getByText(/db/i)).toBeInTheDocument();
    expect(screen.getByText(/out of space/i)).toBeInTheDocument();
    // The Try again button is present
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
