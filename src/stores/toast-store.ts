import { create } from 'zustand';

export type ToastKind = 'info' | 'success' | 'warning' | 'error';

export interface Toast {
  id: string;
  kind: ToastKind;
  title: string;
  body?: string;
  /** ms to auto-dismiss; 0 = manual close only. */
  ttl: number;
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
  info: (title: string, body?: string) =>
    useToastStore.getState().push({ kind: 'info', title, body, ttl: 4000 }),
  success: (title: string, body?: string) =>
    useToastStore.getState().push({ kind: 'success', title, body, ttl: 4000 }),
  warning: (title: string, body?: string) =>
    useToastStore.getState().push({ kind: 'warning', title, body, ttl: 6000 }),
  error: (title: string, body?: string) =>
    useToastStore.getState().push({ kind: 'error', title, body, ttl: 0 }),
};
