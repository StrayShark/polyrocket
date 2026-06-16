import { create } from 'zustand';
import { sendNotification } from '@/ipc';
import { usePrefsStore } from './prefs-store';

export type ToastKind = 'info' | 'success' | 'warning' | 'error';

export interface Toast {
  id: string;
  kind: ToastKind;
  title: string;
  body?: string;
  /** ms to auto-dismiss; 0 = manual close only. */
  ttl: number;
  /** If true, also send a system notification (respects prefs.notificationsEnabled). */
  systemNotify?: boolean;
}

interface ToastState {
  toasts: Toast[];
  push: (t: Omit<Toast, 'id'>) => string;
  dismiss: (id: string) => void;
  clear: () => void;
}

export const useToastStore = create<ToastState>((set) => ({
  toasts: [],
  push: (t) => {
    const id = `t_${Date.now()}_${Math.random().toString(36).slice(2, 6)}`;
    set((s) => ({ toasts: [...s.toasts, { ...t, id }] }));

    // Best-effort system notification (fire-and-forget).
    if (t.systemNotify) {
      const prefs = usePrefsStore.getState();
      if (prefs.notificationsEnabled) {
        // Map toast kind → notify kind for the OS payload.
        const kind =
          t.kind === 'success' ? 'info' :
          t.kind === 'warning' ? 'info' :
          t.kind === 'error' ? 'keyring_error' :
          'info';
        sendNotification(kind, t.title, t.body ?? '', prefs.notificationsEnabled).catch(() => {
          // Silently ignore — best-effort only
        });
      }
    }

    if (t.ttl > 0) {
      setTimeout(() => {
        set((s) => ({ toasts: s.toasts.filter((x) => x.id !== id) }));
      }, t.ttl);
    }
    return id;
  },
  dismiss: (id) => set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) })),
  clear: () => set({ toasts: [] }),
}));

/** Convenience helpers */
export const toast = {
  info: (title: string, body?: string, systemNotify = false) =>
    useToastStore.getState().push({ kind: 'info', title, body, ttl: 4000, systemNotify }),
  success: (title: string, body?: string, systemNotify = false) =>
    useToastStore.getState().push({ kind: 'success', title, body, ttl: 4000, systemNotify }),
  warning: (title: string, body?: string, systemNotify = true) =>
    useToastStore.getState().push({ kind: 'warning', title, body, ttl: 6000, systemNotify }),
  error: (title: string, body?: string, systemNotify = true) =>
    useToastStore.getState().push({ kind: 'error', title, body, ttl: 0, systemNotify }),
};
