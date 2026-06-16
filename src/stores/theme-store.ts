import { create } from 'zustand';
import { persist, createJSONStorage } from 'zustand/middleware';

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