import { create } from 'zustand';
import { persist, createJSONStorage } from 'zustand/middleware';

/**
 * L1 — global UI preferences (NOT the same as user_brief_prefs in DB).
 * Persisted to localStorage; pure UI state.
 */
export interface UiPrefs {
  defaultMinEdgePct: number;
  defaultAllocationCapUsdc: number;
  copyTradingEnabled: boolean;
  notificationsEnabled: boolean;
  /** Show advanced stats in PnL/Lab pages. */
  advancedStats: boolean;
  /** v0.23c — auto-promote if better: how much better
   * the candidate must be (lower Brier) to auto-promote.
   * Default 0.005. Set to 1.0 to effectively disable
   * auto-promote (candidate is never 1.0 better). */
  autoPromoteBrierMargin: number;
  /** v0.28c — auto-promote-after-train: when true, the
   * Rust `train_job` handler spawns a background
   * `auto_promote_if_better` worker after a successful
   * train, and the L1 listens for the
   * `auto_promote:finished` event to auto-refresh the
   * history panel. Default false. */
  autoPromoteAfterTrain: boolean;
  /** v0.39b — auto-promote desktop notification: when
   * true, the L1 sends a real OS notification (macOS
   * Notification Center / Windows toast / Linux
   * libnotify) when a background auto-promote
   * completes. The in-app toast still fires
   * regardless. Default true (the user usually
   * wants the OS notification when a background
   * action completes). */
  autoPromoteNotify: boolean;
  /** v0.42c — opt-in lifecycle telemetry. When true,
   * every Rust lifecycle event (train started /
   * completed / failed, promote completed, scheduler
   * tick, etc.) writes one NDJSON line to stderr.
   * Default false. Capture with `polyrocket 2>
   * telemetry.log`. The user can change this at
   * runtime via the Settings telemetry toggle —
   * changes are pushed to Rust via the
   * `setTelemetryEnabled` IPC. */
  telemetryEnabled: boolean;
}

interface PrefsState extends UiPrefs {
  setPref: <K extends keyof UiPrefs>(k: K, v: UiPrefs[K]) => void;
  reset: () => void;
}

const DEFAULT: UiPrefs = {
  defaultMinEdgePct: 5,
  defaultAllocationCapUsdc: 100,
  copyTradingEnabled: false,
  notificationsEnabled: true,
  advancedStats: false,
  autoPromoteBrierMargin: 0.005,
  autoPromoteAfterTrain: false,
  autoPromoteNotify: true,
  telemetryEnabled: false,
};

export const usePrefsStore = create<PrefsState>()(
  persist(
    (set) => ({
      ...DEFAULT,
      setPref: (k, v) => set({ [k]: v } as Partial<UiPrefs>),
      reset: () => set({ ...DEFAULT }),
    }),
    {
      name: 'polyrocket.prefs',
      storage: createJSONStorage(() => localStorage),
    },
  ),
);
