// v0.62a — toast-store tests.
//
// The toast store is a small zustand store that
// pushes Toast entries to an in-memory list
// (auto-dismissed after ttl). Today 0% coverage.
// This file covers:
//   1. push adds a toast
//   2. dismiss removes by id
//   3. clear empties the list
//   4. toast.* convenience helpers

// @vitest-environment happy-dom

import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { useToastStore, toast } from './toast-store';

// Don't try to send system notifications in tests.
vi.mock('@/ipc', () => ({
  sendNotification: vi.fn().mockResolvedValue(undefined),
}));

let mockNotificationsEnabled = false;
vi.mock('./prefs-store', () => ({
  usePrefsStore: {
    getState: () => ({ notificationsEnabled: mockNotificationsEnabled }),
  },
}));

describe('useToastStore', () => {
  beforeEach(() => {
    useToastStore.getState().clear();
  });

  it('starts empty', () => {
    expect(useToastStore.getState().toasts).toEqual([]);
  });

  it('push adds a toast and returns its id', () => {
    const id = useToastStore.getState().push({ kind: 'info', title: 'hi', ttl: 0 });
    expect(id).toMatch(/^t_/);
    expect(useToastStore.getState().toasts).toHaveLength(1);
    expect(useToastStore.getState().toasts[0].title).toBe('hi');
  });

  it('dismiss removes a toast by id', () => {
    const id = useToastStore.getState().push({ kind: 'info', title: 'x', ttl: 0 });
    useToastStore.getState().dismiss(id);
    expect(useToastStore.getState().toasts).toEqual([]);
  });

  it('clear empties the list', () => {
    useToastStore.getState().push({ kind: 'info', title: 'a', ttl: 0 });
    useToastStore.getState().push({ kind: 'info', title: 'b', ttl: 0 });
    useToastStore.getState().clear();
    expect(useToastStore.getState().toasts).toEqual([]);
  });
});

describe('toast convenience helpers', () => {
  beforeEach(() => {
    useToastStore.getState().clear();
  });

  it('toast.info pushes an info toast', () => {
    toast.info('hello');
    expect(useToastStore.getState().toasts[0].kind).toBe('info');
    expect(useToastStore.getState().toasts[0].title).toBe('hello');
  });

  it('toast.success pushes a success toast', () => {
    toast.success('yay');
    expect(useToastStore.getState().toasts[0].kind).toBe('success');
  });

  it('toast.error pushes an error toast with ttl=0 (manual close)', () => {
    toast.error('boom');
    expect(useToastStore.getState().toasts[0].kind).toBe('error');
    expect(useToastStore.getState().toasts[0].ttl).toBe(0);
  });

  // v0.106 — coverage ramp. Cover toast.warning convenience helper
  // (line 80 in source: `useToastStore.getState().push({ kind: 'warning', ..., systemNotify })`).
  it('toast.warning pushes a warning toast with ttl=6000 (covers line 80)', () => {
    toast.warning('be careful');
    expect(useToastStore.getState().toasts[0].kind).toBe('warning');
    expect(useToastStore.getState().toasts[0].ttl).toBe(6000);
  });

  it('toast.warning with systemNotify=false still works (warning default true)', () => {
    toast.warning('be careful', 'details', false);
    expect(useToastStore.getState().toasts[0].systemNotify).toBe(false);
  });
});

// v0.106 — coverage ramp. Cover the setTimeout auto-dismiss branch
// (line 62-66 in source: `if (t.ttl > 0) { setTimeout(...) }`).
describe('useToastStore TTL auto-dismiss', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    useToastStore.getState().clear();
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it('toast with ttl > 0 is auto-dismissed after ttl (covers line 62-66 branch)', () => {
    const id = useToastStore.getState().push({ kind: 'info', title: 'transient', ttl: 1000 });
    expect(useToastStore.getState().toasts).toHaveLength(1);
    vi.advanceTimersByTime(1000);
    expect(useToastStore.getState().toasts).toHaveLength(0);
    // id is the dismissed toast
    expect(id).toMatch(/^t_/);
  });
});

// v0.106 — coverage ramp. Cover the kind-mapping ternary chain (lines 51-55)
// and the systemNotify branch (line 47). With notificationsEnabled=true,
// every kind ('info' / 'success' / 'warning' / 'error') hits a different
// branch of the ternary.
describe('useToastStore systemNotify kind mapping', () => {
  beforeEach(() => {
    mockNotificationsEnabled = true;
    useToastStore.getState().clear();
  });
  afterEach(() => {
    mockNotificationsEnabled = false;
  });

  it('system notify fires for kind=success (maps to "info" notify)', async () => {
    const { sendNotification } = await import('@/ipc');
    useToastStore.getState().push({ kind: 'success', title: 'win', ttl: 0, systemNotify: true });
    await vi.waitFor(() => {
      expect(sendNotification).toHaveBeenCalledWith('info', 'win', '', true);
    });
  });

  it('system notify fires for kind=warning (maps to "info" notify)', async () => {
    const { sendNotification } = await import('@/ipc');
    useToastStore.getState().push({ kind: 'warning', title: 'careful', ttl: 0, systemNotify: true });
    await vi.waitFor(() => {
      expect(sendNotification).toHaveBeenCalledWith('info', 'careful', '', true);
    });
  });

  it('system notify fires for kind=error (maps to "keyring_error" notify)', async () => {
    const { sendNotification } = await import('@/ipc');
    useToastStore.getState().push({ kind: 'error', title: 'boom', ttl: 0, systemNotify: true });
    await vi.waitFor(() => {
      expect(sendNotification).toHaveBeenCalledWith('keyring_error', 'boom', '', true);
    });
  });

  it('system notify does NOT fire when systemNotify=false', async () => {
    const { sendNotification } = await import('@/ipc');
    vi.mocked(sendNotification).mockClear();
    useToastStore.getState().push({ kind: 'success', title: 'silent', ttl: 0, systemNotify: false });
    expect(sendNotification).not.toHaveBeenCalled();
  });
});
