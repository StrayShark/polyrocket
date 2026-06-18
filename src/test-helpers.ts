// polyrocket — test helpers (v0.66d).
//
// A tiny utility module shared across vitest test files.
// Currently exports:
//
//   `withFakeTimersAndState(fn)`
//     Wrap an async test body that needs vi.useFakeTimers +
//     React state updates. In happy-dom + React 18, the
//     state update from a setTimeout callback does NOT
//     propagate to a test rig until a microtask flushes.
//     This helper advances the fake timers, yields to the
//     microtask queue, then re-asserts.
//
//     Usage:
//       it('foo', async () => {
//         await withFakeTimersAndState(async () => {
//           rig.fireKey('g');
//           vi.advanceTimersByTime(1500);
//         });
//         expect(rig.pendingPrefix).toBeNull();
//       });
//
//   Why this exists (v0.65b ship log): the original
//   keyboard-nav prefix-timeout test couldn't get React
//   setState from a setTimeout callback to reach the
//   test rig. Tried 4 workarounds:
//
//     1. vi.advanceTimersByTime(1500) in act()  — stays 'g'
//     2. vi.runAllTimers() in act()             — stays 'g'
//     3. vi.useFakeTimers({ toFake: [...] })    — stays 'g'
//     4. await act(async () => { advance; await Promise.resolve() })
//                                               — stays 'g'
//
//   The pattern that DOES work in happy-dom: advance
//   timers (sync, fires the callback) THEN yield a real
//   microtask via `await Promise.resolve()` AFTER the
//   advance. This helper encapsulates that. v0.66d
//   retried the keyboard-nav test with this helper and
//   the prefix-timeout test now passes.

import { act } from '@testing-library/react';

export async function withFakeTimersAndState<T>(
  fn: () => T | Promise<T>,
): Promise<T> {
  return act(async () => {
    const result = await fn();
    // Yield to the microtask queue so React's scheduler
    // can flush any pending setState from inside the
    // timer callback. Without this, state updates
    // scheduled by setTimeout are stuck in the React
    // batch and don't reach the test rig.
    await Promise.resolve();
    // Yield once more for the nested scheduler pass.
    // (React 18 sometimes needs 2 microtask flushes
    // to fully propagate a useState update from a
    // setTimeout callback into a test assertion.)
    await Promise.resolve();
    return result;
  });
}
