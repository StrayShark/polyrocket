import { create } from 'zustand';
import { sendNotification } from '@/ipc';
import { usePrefsStore } from './prefs-store';

/** Toast 4 种 kind。决定颜色 + 默认 ttl + 是否弹 OS 通知。
 *   - `'info'` — 蓝色 4s
 *   - `'success'` — 绿色 4s
 *   - `'warning'` — 黄色 6s（额外弹 OS 通知）
 *   - `'error'` — 红色 manual close（额外弹 OS 通知）
 */
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

/** Toast zustand store。**不持久化**（toast 是 transient 状态）。
 *
 * **`push()` 业务流程**：
 *   1. 生成 id（`t_${ts}_${rand}`）
 *   2. 推入 toasts 数组
 *   3. 如果 `systemNotify: true` 且 prefs enabled → 调 `sendNotification` IPC
 *   4. 如果 `ttl > 0` → setTimeout 到时自动 dismiss
 *   5. 返回 id（给调用方 dismiss）
 */
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
