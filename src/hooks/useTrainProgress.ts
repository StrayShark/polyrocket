import { useEffect, useRef, useState } from 'react';
import type { TrainStartedEvent } from '@/ipc';

/**
 * Hook signature for a Tauri-style event listener subscription.
 * Returns a Promise resolving to an unlisten function.
 */
type ListenFn = (
  cb: (e: TrainStartedEvent) => void,
) => Promise<() => void>;

/**
 * `useTrainProgress` — encapsulates the train-job progress
 * tracking state machine for `/model-lab` (v0.91).
 *
 * **State machine**:
 *   - `idle` — no train in progress, no event expected
 *   - `expected` — caller called `markExpected()` (typically
 *     before `trainMut.mutate()`). The next `train:started`
 *     event will be captured.
 *   - `running` — `train:started` event captured; `activeTrainJobId`
 *     is set. Resets to `idle` automatically.
 *
 * **Why a custom hook** (v0.83 deferred):
 *   The original inline `useEffect` in ModelLab.tsx was hard to
 *   test because it depended on Tauri's `safeListen` returning a
 *   `Promise<UnlistenFn>` AND on React 18 strict mode's
 *   double-mount behavior (first mount's listener has
 *   `cancelled = true`). Extracting into a hook lets us:
 *     1. Pass a `ListenFn` directly in tests (no Tauri runtime)
 *     2. Test state transitions deterministically
 *     3. Reuse the hook in any page that wants train progress
 *        (e.g. future `/dashboard` widget)
 *
 * **Caller pattern**:
 *   ```ts
 *   const { activeTrainJobId, markExpected, clearActive } = useTrainProgress({
 *     onTrainStarted,
 *   });
 *
 *   const handleClick = () => {
 *     markExpected();
 *     trainMut.mutate();
 *   };
 *   ```
 */
export interface UseTrainProgressResult {
  /** Job ID of the in-flight train, or null when idle. */
  activeTrainJobId: string | null;
  /**
   * Mark that the next `train:started` event should be
   * captured. Call this BEFORE invoking `trainMut.mutate()`.
   * v0.91 — replaces direct ref mutation `expectedTrainRef.current = true`.
   */
  markExpected: () => void;
  /**
   * Manually clear the active job ID (e.g. on cancel button
   * or after a stale event). v0.91 — replaces direct
   * `setActiveTrainJobId(null)` from cancel handlers.
   */
  clearActive: () => void;
}

/**
 * Subscribe to `train:started` events and capture the job ID
 * when one is expected (after `markExpected()`).
 *
 * @param opts.onTrainStarted — the Tauri event subscription
 *                              function (e.g. `onTrainStarted`).
 * @param opts.enabled — set false to detach the listener
 *                        (default true). Used by tests to
 *                        verify cleanup paths.
 */
export function useTrainProgress(opts: {
  onTrainStarted: ListenFn;
  enabled?: boolean;
}): UseTrainProgressResult {
  const { onTrainStarted, enabled = true } = opts;
  const [activeTrainJobId, setActiveTrainJobId] = useState<string | null>(null);
  // Ref avoids the listener closure depending on the
  // markExpected() call site — the listener always reads
  // the latest value.
  const expectedTrainRef = useRef<boolean>(false);

  useEffect(() => {
    if (!enabled) return;
    let cancelled = false;
    const unsubPromise = onTrainStarted((e) => {
      if (cancelled) return;
      if (expectedTrainRef.current) {
        setActiveTrainJobId(e.job_id);
        expectedTrainRef.current = false;
      }
    });
    return () => {
      // Strict-mode double-mount: first mount's listener
      // has cancelled=true; we still await the unlisten
      // Promise to detach it cleanly. The .catch is silent
      // because tests may not await the unlisten.
      cancelled = true;
      unsubPromise.then((u) => u()).catch(() => { /* ignore */ });
    };
  }, [onTrainStarted, enabled]);

  const markExpected = () => {
    expectedTrainRef.current = true;
  };
  const clearActive = () => {
    setActiveTrainJobId(null);
  };

  return { activeTrainJobId, markExpected, clearActive };
}
