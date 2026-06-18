import { create } from 'zustand';
import { persist, createJSONStorage } from 'zustand/middleware';

/** 3 个主题选项。**仅配色差异**（per `docs/overview.md` 强约束）：
 *   - `'dark'` — Cursor/VS Code Dark+ 风格
 *   - `'light'` — 经典 light 风格
 *   - `'matrix'` — Codex CLI 绿底黑字风格
 *
 * **3 个主题共享 layout** —— 不存在 per-theme 变体。如果未来加主题需要 layout
 * 调整，先更新 `scripts/check-theme-contrast.mjs` 验 4.5:1 对比度。
 */
export type Theme = 'dark' | 'light' | 'matrix';

interface ThemeState {
  theme: Theme;
  setTheme: (t: Theme) => void;
  cycleTheme: () => void;
}

const ORDER: Theme[] = ['dark', 'light', 'matrix'];

function applyToDom(t: Theme) {
  if (typeof document !== 'undefined') {
    document.documentElement.setAttribute('data-theme', t);
  }
}

/** Theme zustand store。**持久化** 到 `localStorage['polyrocket.theme']`。
 *
 * **`onRehydrateStorage`**：SSR 安全 + 自动 apply `data-theme` 到 `<html>`。
 * L1 「Settings → Theme」+ 快捷键切换走 `cycleTheme()`。
 */
export const useThemeStore = create<ThemeState>()(
  persist(
    (set, get) => ({
      theme: 'dark',
      setTheme: (t) => {
        set({ theme: t });
        applyToDom(t);
      },
      cycleTheme: () => {
        const cur = get().theme;
        const next = ORDER[(ORDER.indexOf(cur) + 1) % ORDER.length];
        get().setTheme(next);
      },
    }),
    {
      name: 'polyrocket.theme',
      storage: createJSONStorage(() => localStorage),
      onRehydrateStorage: () => (state) => {
        if (state?.theme) applyToDom(state.theme);
      },
    },
  ),
);