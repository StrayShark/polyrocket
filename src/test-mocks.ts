// polyrocket — shared test mock fixtures (v0.67f).
//
// Centralizes the `@/ipc` mock pattern that was duplicated
// across 6+ test files. The hard-won lessons from v0.66:
//
//   1. `sendNotification` MUST be in any `@/ipc` mock.
//      toast-store.ts calls it on system-enabled toasts;
//      without it the call resolves to undefined and
//      throws an unhandled rejection.
//   2. `requestNotificationPermission` same story.
//   3. zustand store mocks (e.g. `usePrefsStore`) need
//      BOTH a hook AND a `getState()` method, because
//      toast-store.ts uses `usePrefsStore.getState()`
//      internally (not the hook).
//
// **Usage**:
//
//   import { createIpcMock } from '@/test-mocks';
//   vi.mock('@/ipc', () => createIpcMock({
//     llmProviderList: vi.fn().mockResolvedValue([]),
//     // any other IPC methods you need to override
//   }));
//
// `createIpcMock(overrides)` returns an object that includes
// `sendNotification` and `requestNotificationPermission` as
// `vi.fn().mockResolvedValue(undefined)` by default. Override
// anything by passing it in the `overrides` argument. Anything
// you DON'T override will be `vi.fn()` returning undefined
// (so tests can still assert on calls without the test file
// having to enumerate every IPC the component might use).

import { vi } from 'vitest';

/**
 * Build an `@/ipc` mock object that includes the always-required
 * `sendNotification` and `requestNotificationPermission` plus
 * any test-specific overrides.
 *
 * The result is suitable to be returned from a `vi.mock('@/ipc',
 * () => createIpcMock({ ... }))` factory.
 *
 * @param overrides Map of IPC method name → mock implementation.
 *                  Anything not listed here is set to a default
 *                  `vi.fn()` (returning undefined) so tests can
 *                  still `expect(mock.foo).toHaveBeenCalled()`
 *                  without setting up an explicit return value.
 */
export function createIpcMock(
  overrides: Record<string, ReturnType<typeof vi.fn>> = {},
): Record<string, ReturnType<typeof vi.fn>> {
  return {
    // v0.67f — always-on. toast-store.ts uses both of these
    // when rendering system-enabled toasts. Without these
    // mocks the import resolves to undefined and we get
    // "sendNotification is not a function" unhandled rejections.
    sendNotification: vi.fn().mockResolvedValue(undefined),
    requestNotificationPermission: vi.fn().mockResolvedValue(true),
    // Default stub for every other IPC. The override map
    // wins if a key matches.
    ...overrides,
  };
}

/**
 * Build a `usePrefsStore` mock with both the hook AND the
 * `getState()` method. The zustand-style API is required
 * because toast-store.ts calls `usePrefsStore.getState()`
 * directly (not via the hook) when reading preferences.
 *
 * @param state Initial state. Defaults to a minimal "all off" stub.
 */
export function createPrefsStoreMock(
  state: Record<string, unknown> = {},
): ((selector?: unknown) => unknown) & { getState: () => unknown } {
  const defaults = {
    autoPromoteAfterTrain: false,
    autoPromoteNotify: false,
    notificationsEnabled: true,
    ...state,
  };
  const usePrefsStore: any = () => defaults;
  usePrefsStore.getState = () => defaults;
  return usePrefsStore;
}
