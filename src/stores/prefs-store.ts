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
