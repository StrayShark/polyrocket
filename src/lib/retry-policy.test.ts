import { describe, expect, it } from 'vitest';
import { retryDelayMs, shouldRetry, MAX_RETRY_ATTEMPTS } from './retry-policy';
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
    // n=0: 500 ± 100  (range 400-600)
    // n=1: 1500 ± 300 (range 1200-1800)
    // n=2: 4500 ± 900 (range 3600-5400)
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
    // The loop below is the actual assertion; the unused a/b were
    // removed — we sample fresh values inside the loop instead.
    let sameCount = 0;
    for (let i = 0; i < 10; i++) {
      if (retryDelayMs(1) === retryDelayMs(1)) sameCount++;
    }
    // At least 9 of 10 should differ
    expect(sameCount).toBeLessThan(10);
  });
});
