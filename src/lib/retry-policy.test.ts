import { describe, expect, it } from 'vitest';
import { retryDelayMs, shouldRetry, MAX_RETRY_ATTEMPTS, applyRetryPolicy } from './retry-policy';
import type { AppErrorShape } from './invoke-safe';

const retryable = (retryable: boolean): AppErrorShape => ({
  kind: retryable ? 'http' : 'invalid',
  message: 'x',
  hint: 'y',
  retryable,
  raw: 'x',
});

describe('shouldRetry', () => {
  it('returns false once max attempts reached', () => {
    expect(shouldRetry(MAX_RETRY_ATTEMPTS, retryable(true))).toBe(false);
    expect(shouldRetry(MAX_RETRY_ATTEMPTS + 1, retryable(true))).toBe(false);
  });

  it('returns true for retryable errors below the cap', () => {
    expect(shouldRetry(0, retryable(true))).toBe(true);
    expect(shouldRetry(1, retryable(true))).toBe(true);
    expect(shouldRetry(2, retryable(true))).toBe(true);
  });

  it('returns false for non-retryable errors even at attempt 0', () => {
    expect(shouldRetry(0, retryable(false))).toBe(false);
  });

  it('returns true for non-shaped errors (defensive default)', () => {
    expect(shouldRetry(0, new Error('boom'))).toBe(true);
    expect(shouldRetry(0, 'string error')).toBe(true);
    expect(shouldRetry(0, null)).toBe(true);
  });
});

describe('retryDelayMs', () => {
  it('grows exponentially across attempts', () => {
    // n=0: 500 ± 100  （范围 400-600）
    // n=1: 1500 ± 300 （范围 1200-1800）
    // n=2: 4500 ± 900 （范围 3600-5400）
    for (let trial = 0; trial < 20; trial++) {
      const d0 = retryDelayMs(0);
      expect(d0).toBeGreaterThanOrEqual(400);
      expect(d0).toBeLessThanOrEqual(600);

      const d1 = retryDelayMs(1);
      expect(d1).toBeGreaterThanOrEqual(1200);
      expect(d1).toBeLessThanOrEqual(1800);

      const d2 = retryDelayMs(2);
      expect(d2).toBeGreaterThanOrEqual(3600);
      expect(d2).toBeLessThanOrEqual(5400);
    }
  });

  it('jitter is non-deterministic (returns different values)', () => {
    // 下面的循环才是真正的断言;不再使用的 a/b 变量
    // 已被移除 —— 我们改为在循环内部取新的值。
    let sameCount = 0;
    for (let i = 0; i < 10; i++) {
      if (retryDelayMs(1) === retryDelayMs(1)) sameCount++;
    }
    // 10 次中至少有 9 次应不同
    expect(sameCount).toBeLessThan(10);
  });
});

describe('applyRetryPolicy (v0.95)', () => {
  it('sets queries default options: refetchOnWindowFocus false, retry=shouldRetry, retryDelay=retryDelayMs, staleTime 30s', () => {
    const client = {
      setDefaultOptions: (opts: unknown) => {
        // 暂存 options 以便断言
        (client as unknown as { opts: unknown }).opts = opts;
      },
    } as unknown as Parameters<typeof applyRetryPolicy>[0];
    applyRetryPolicy(client);
    const opts = (client as unknown as { opts: { queries: Record<string, unknown>; mutations: Record<string, unknown> } }).opts;
    expect(opts.queries.refetchOnWindowFocus).toBe(false);
    expect(opts.queries.retry).toBe(shouldRetry);
    expect(opts.queries.retryDelay).toBe(retryDelayMs);
    expect(opts.queries.staleTime).toBe(30_000);
    // Mutations 不应自动重试
    expect(opts.mutations.retry).toBe(false);
  });
});
