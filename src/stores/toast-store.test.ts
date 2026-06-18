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

import { describe, it, expect, beforeEach, vi } from 'vitest';
import { useToastStore, toast } from './toast-store';

// Don't try to send system notifications in tests.
vi.mock('@/ipc', () => ({
  sendNotification: vi.fn().mockResolvedValue(undefined),
}));

vi.mock('./prefs-store', () => ({
  usePrefsStore: {
    getState: () => ({ notificationsEnabled: false }),
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
});
