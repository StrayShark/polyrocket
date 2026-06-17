/**
 * TanStack Query retry policy (v0.9a).
 *
 * Replaces the default `retry: 1` with a smarter policy that:
 *   1. Inspects the error shape (from `lib/invoke-safe.ts`)
 *   2. Respects the `retryable` flag we already attach
 *   3. Adds exponential backoff + jitter (so 10 queries don't all
 *      retry at the same instant)
 *   4. Caps total attempts to avoid hammering the backend
 *
 * Behaviour:
 *   - `retryable: false` errors (invalid / not_found / internal / serde /
 *     keyring) → fail immediately, no retry
 *   - `retryable: true` errors (db / http / io / network / unknown) →
 *     retry with delays 500ms, 1.5s, 4.5s (×1.5 backoff, ±20% jitter)
 *   - max 3 retries (4 attempts total)
 *   - All retryable:false errors are surfaced to the UI on first failure
 *
 * This is the v0.8b → v0.9a upgrade. The error classifier stays the
 * single source of truth; this file is just the retry policy that
 * reads its `retryable` flag.
 */

import type { QueryClient } from '@tanstack/react-query';
import { canRetry, type AppErrorShape } from './invoke-safe';

const MAX_RETRIES = 3;
const BASE_DELAY_MS = 500;
const BACKOFF_FACTOR = 3;     // 500ms → 1.5s → 4.5s
const JITTER_FRACTION = 0.2;  // ±20%

/**
 * Compute the delay before retry attempt `n` (0-indexed).
 *   n=0 → 500ms ± jitter
 *   n=1 → 1500ms ± jitter
 *   n=2 → 4500ms ± jitter
 */
export function retryDelayMs(attempt: number): number {
  const base = BASE_DELAY_MS * Math.pow(BACKOFF_FACTOR, attempt);
  const jitter = base * JITTER_FRACTION * (Math.random() * 2 - 1);
  return Math.round(base + jitter);
}

/**
 * Predicate: should this error be retried?
 * Returns `false` for non-retryable kinds, `true` (up to MAX_RETRIES)
 * for retryable ones.
 */
export function shouldRetry(failureCount: number, error: unknown): boolean {
  if (failureCount >= MAX_RETRIES) return false;
  // We can't safely classify a non-shaped error; treat as retryable.
  if (!isAppErrorShape(error)) return true;
  return canRetry(error);
}

function isAppErrorShape(e: unknown): e is AppErrorShape {
  return (
    typeof e === 'object' &&
    e !== null &&
    'kind' in e &&
    'message' in e &&
    'retryable' in e
  );
}

/**
 * Apply the v0.9a retry policy to a QueryClient instance.
 * Call this from `main.tsx` after creating the client.
 */
export function applyRetryPolicy(client: QueryClient): void {
  client.setDefaultOptions({
    queries: {
      // Don't hammer on cold start. We already pre-emptively retry
      // in the background; only refetch on window-focus.
      refetchOnWindowFocus: false,
      retry: shouldRetry,
      retryDelay: retryDelayMs,
      // 30s staleness is a reasonable balance between freshness
      // and "don't re-fetch on every render".
      staleTime: 30_000,
    },
    mutations: {
      // Mutations are user-initiated; don't auto-retry (they often
      // have side effects). Users can hit the button again.
      retry: false,
    },
  });
}

/** Max retries (exposed for tests + UI). */
export const MAX_RETRY_ATTEMPTS = MAX_RETRIES;
