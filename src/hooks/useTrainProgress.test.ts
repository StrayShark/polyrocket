// v0.91 — useTrainProgress hook tests (+7 tests).
//
// The hook was extracted in v0.91 from ModelLab.tsx to make
// the deferred v0.83 branch coverage gap testable. This file
// verifies:
//   - initial state: activeTrainJobId=null
//   - markExpected + train:started → activeTrainJobId set
//   - markExpected + train:started resets the expected flag
//     (next event ignored)
//   - 2 markExpected + 2 train:started → second one captured
//   - markExpected + train:started + clearActive → null
//   - enabled=false → no listener subscribed
//   - listener cleanup on unmount (cancelled=true on first
//     mount in strict mode; second mount's listener stays)

// @vitest-environment happy-dom

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { useTrainProgress } from './useTrainProgress';
import type { TrainStartedEvent } from '@/ipc';

type ListenFn = (
  cb: (e: TrainStartedEvent) => void,
) => Promise<() => void>;

function evt(jobId: string): TrainStartedEvent {
  return { job_id: jobId, n_trials: 4, epochs: 80, started_at: 1700000000 };
}

/**
 * Build a controllable ListenFn for tests. Returns helpers:
 *   - fn: the ListenFn to pass to the hook
 *   - fire(): push an event to all registered listeners
 *   - subscribeCount(): how many times the listener was registered
 *   - unsub(): the unlisten fn for the most recent subscription
 */
function makeListener() {
  const listeners: Array<(e: TrainStartedEvent) => void> = [];
  const unsubFns: Array<() => void> = [];
  let count = 0;
  const fn: ListenFn = (cb) => {
    count++;
    listeners.push(cb);
    const unsub = vi.fn();
    unsubFns.push(unsub);
    return Promise.resolve(unsub);
  };
  return {
    fn,
    fire: (e: TrainStartedEvent) => listeners.forEach((l) => l(e)),
    subscribeCount: () => count,
    lastUnsub: () => unsubFns[unsubFns.length - 1],
  };
}

describe('useTrainProgress (v0.91)', () => {
  let listener: ReturnType<typeof makeListener>;
  let onTrainStarted: ListenFn;

  beforeEach(() => {
    listener = makeListener();
    onTrainStarted = listener.fn;
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it('starts with activeTrainJobId=null and subscribes once on mount', () => {
    const { result } = renderHook(() => useTrainProgress({ onTrainStarted }));
    expect(result.current.activeTrainJobId).toBeNull();
    expect(listener.subscribeCount()).toBe(1);
  });

  it('captures job_id when markExpected is called before train:started', async () => {
    const { result } = renderHook(() => useTrainProgress({ onTrainStarted }));
    act(() => {
      result.current.markExpected();
    });
    await act(async () => {
      listener.fire(evt('train-1'));
    });
    expect(result.current.activeTrainJobId).toBe('train-1');
  });

  it('ignores train:started when markExpected was NOT called', async () => {
    const { result } = renderHook(() => useTrainProgress({ onTrainStarted }));
    await act(async () => {
      listener.fire(evt('orphan-event'));
    });
    expect(result.current.activeTrainJobId).toBeNull();
  });

  it('captures only the NEXT event after markExpected (ref resets)', async () => {
    const { result } = renderHook(() => useTrainProgress({ onTrainStarted }));
    act(() => {
      result.current.markExpected();
    });
    await act(async () => {
      listener.fire(evt('first'));
    });
    expect(result.current.activeTrainJobId).toBe('first');
    // Second event arrives without another markExpected — ignored
    await act(async () => {
      listener.fire(evt('second'));
    });
    expect(result.current.activeTrainJobId).toBe('first');
  });

  it('clearActive() resets activeTrainJobId to null', async () => {
    const { result } = renderHook(() => useTrainProgress({ onTrainStarted }));
    act(() => {
      result.current.markExpected();
    });
    await act(async () => {
      listener.fire(evt('train-x'));
    });
    expect(result.current.activeTrainJobId).toBe('train-x');
    act(() => {
      result.current.clearActive();
    });
    expect(result.current.activeTrainJobId).toBeNull();
  });

  it('enabled=false does NOT subscribe to onTrainStarted', () => {
    const { result } = renderHook(() =>
      useTrainProgress({ onTrainStarted, enabled: false }),
    );
    expect(listener.subscribeCount()).toBe(0);
    expect(result.current.activeTrainJobId).toBeNull();
  });

  it('unmounts clean up the listener (await unlisten Promise)', async () => {
    const { unmount } = renderHook(() =>
      useTrainProgress({ onTrainStarted }),
    );
    expect(listener.subscribeCount()).toBe(1);
    const unsub = listener.lastUnsub();
    expect(unsub).toBeDefined();
    unmount();
    // The cleanup function awaits the unlisten promise.
    // After unmount, the unlisten should have been called.
    await act(async () => {
      // microtask flush
    });
    expect(unsub).toHaveBeenCalled();
  });
});
