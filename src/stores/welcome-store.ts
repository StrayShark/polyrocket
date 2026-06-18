// v0.53b — welcome (first-run landing) zustand store.
//
// Renamed from v0.13 'polyrocket.onboarding' to
// 'polyrocket.welcome' (v0.53 spec). On first read
// the store migrates the old key once and deletes
// it, so users mid-onboarding don't lose state.

import { create } from 'zustand';
import { persist, createJSONStorage } from 'zustand/middleware';

const STORAGE_KEY = 'polyrocket.welcome';
const LEGACY_KEY = 'polyrocket.onboarding';

/** Welcome flow 6 步。**顺序固定**（WELCOME_STEPS 数组）—— L1 「/welcome」路由按
 * 这个顺序渲染 StepProgress 进度条。
 *
 * **v0.53a spec**：「storage」替代了 v0.13 的「wallet」（wallet 现在是 polymarket
 * 步骤里的 sub-section）。
 */
export type WelcomeStep =
  | 'welcome'
  | 'storage'
  | 'theme'
  | 'llm'
  | 'polymarket'
  | 'finish';

export const WELCOME_STEPS: WelcomeStep[] = [
  'welcome',
  'storage',
  'theme',
  'llm',
  'polymarket',
  'finish',
];

export interface WelcomeState {
  done: boolean;
  step: WelcomeStep;
  locale: string;
  // Sub-step tracking. Each main step has its own
  // skip / partial-completion state. The Dashboard
  // banner uses these to decide what to nag about.
  configured: {
    storagePath: boolean;
    theme: boolean;
    llmAtLeastOne: boolean;
    polymarketApi: boolean;
    walletPk: boolean;
  };
  setStep: (s: WelcomeStep) => void;
  setDone: (b: boolean) => void;
  setLocale: (l: string) => void;
  setConfigured: (k: keyof WelcomeState['configured'], v: boolean) => void;
  reset: () => void;
}

const initial = {
  done: false,
  step: 'welcome' as WelcomeStep,
  locale: '',
  configured: {
    storagePath: false,
    theme: false,
    llmAtLeastOne: false,
    polymarketApi: false,
    walletPk: false,
  },
};

/**
 * One-time migration: if the old
 * `polyrocket.onboarding` key exists, copy the
 * relevant fields over and delete the old key.
 * We do this lazily on first store read so a user
 * who already has `done=true` keeps that state.
 */
function migrateLegacy(): void {
  try {
    if (typeof window === 'undefined') return;
    const raw = window.localStorage.getItem(LEGACY_KEY);
    if (!raw) return;
    const parsed = JSON.parse(raw) as {
      state?: { done?: boolean; step?: number };
    };
    const newRaw = window.localStorage.getItem(STORAGE_KEY);
    if (!newRaw) {
      // Copy with the field renames we know about.
      // Old: step was a number 0..3 (welcome /
      // theme / wallet / llm). New: step is a
      // WelcomeStep string. We map the closest.
      const stepNum = parsed.state?.step ?? 0;
      const newStep: WelcomeStep =
        stepNum === 0
          ? 'welcome'
          : stepNum === 1
            ? 'theme'
            : stepNum === 2
              ? 'storage' // wallet -> storage (closest)
              : 'llm';
      window.localStorage.setItem(
        STORAGE_KEY,
        JSON.stringify({
          state: {
            ...initial,
            done: parsed.state?.done ?? false,
            step: newStep,
          },
          version: 0,
        }),
      );
    }
    window.localStorage.removeItem(LEGACY_KEY);
  } catch {
    // best-effort; if migration fails, the user
    // just gets a fresh welcome on next launch.
  }
}

/** Welcome zustand store。**持久化** 到 `localStorage['polyrocket.welcome']`。
 *
 * **`migrateLegacy()`**：第一次 store 初始化时把旧的 `polyrocket.onboarding`
 * 数据迁移过来。**lazy on first read** —— 不在 app boot 时强跑，避免阻塞。
 *
 * **5 个 sub-configured flags**：Dashboard banner 用这些判断「用户是否还没配
 * 完」，决定 nag 强度。
 */
export const useWelcomeStore = create<WelcomeState>()(
  persist(
    (set) => ({
      ...initial,
      setStep: (s) => set({ step: s }),
      setDone: (b) =>
        set({
          done: b,
          step: b ? 'finish' : 'welcome',
        }),
      setLocale: (l) => set({ locale: l }),
      setConfigured: (k, v) =>
        set((state) => ({
          configured: { ...state.configured, [k]: v },
        })),
      reset: () => set({ ...initial, done: false }),
    }),
    {
      name: STORAGE_KEY,
      storage: createJSONStorage(() => {
        if (typeof window === 'undefined') {
          return {
            getItem: () => null,
            setItem: () => undefined,
            removeItem: () => undefined,
          };
        }
        migrateLegacy();
        return window.localStorage;
      }),
    },
  ),
);

/** v0.53b — the helper used by the L1 to decide
 * whether the user has unfinished setup. Returns
 * the list of "still needed" sub-configs. The
 * Dashboard banner iterates over this. */
export function pendingConfigs(s: WelcomeState): string[] {
  const out: string[] = [];
  if (!s.configured.llmAtLeastOne) out.push('llm');
  if (!s.configured.polymarketApi) out.push('polymarket');
  if (!s.configured.walletPk) out.push('wallet');
  return out;
}
