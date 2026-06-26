// v0.53b — welcome（首次运行落地页）zustand store。
//
// 在 v0.53 规范中从 v0.13 'polyrocket.onboarding' 重命名为
// 'polyrocket.welcome'。首次读取时,
// store 会迁移旧 key 一次然后删除它,
// 因此 onboarding 进行中的用户不会丢失状态。

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
  // 子步骤追踪。每个主步骤都有自己的
  // 跳过 / 部分完成状态。Dashboard
  // 横幅根据这些状态决定要催哪些项。
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
 * 一次性迁移：如果旧的
 * `polyrocket.onboarding` key 存在，则将
 * 相关字段复制过去并删除旧 key。
 * 我们在首次 store 读取时惰性执行，以便已经
 * 拥有 `done=true` 的用户保留该状态。
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
      // 复制字段，并做已知重命名。
      // 旧版：step 是数字 0..3（welcome /
      // theme / wallet / llm）。新版：step 是
      // WelcomeStep 字符串。映射到最接近的一项。
      const stepNum = parsed.state?.step ?? 0;
      const newStep: WelcomeStep =
        stepNum === 0
          ? 'welcome'
          : stepNum === 1
            ? 'theme'
            : stepNum === 2
              ? 'storage' // wallet -> storage（最接近）
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
    // 尽力而为；如果迁移失败，用户
    // 在下次启动时只会获得一个全新的 welcome 流程。
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

/** v0.53b —— L1 用来判断用户是否还有未完成
 * 设置的辅助函数。返回「仍需配置」的
 * 子项列表。Dashboard 横幅会迭代此列表。 */
export function pendingConfigs(s: WelcomeState): string[] {
  const out: string[] = [];
  if (!s.configured.llmAtLeastOne) out.push('llm');
  if (!s.configured.polymarketApi) out.push('polymarket');
  if (!s.configured.walletPk) out.push('wallet');
  return out;
}
