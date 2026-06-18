// v0.53b — ThemeStep (Step 3 of 6).
//
// Pick a theme. The L1 already has 3 themes (dark /
// light / matrix). The Welcome step is the same
// as the Theme card on Settings, but rendered in
// the welcome layout.

import { useEffect } from 'react';
import { useThemeStore, type Theme } from '@/stores/theme-store';
import { useT } from '@/lib/i18n';
import { CheckCircle2 } from 'lucide-react';
import { cn } from '@/lib/cn';

const themes: Array<{ id: Theme; name: string; desc: string }> = [
  { id: 'dark', name: 'Dark', desc: 'Cursor / VS Code Dark+' },
  { id: 'light', name: 'Light', desc: 'Clean and bright' },
  { id: 'matrix', name: 'Matrix', desc: 'Green-on-black hacker' },
];

export function ThemeStep() {
  const { t } = useT();
  const theme = useThemeStore((s) => s.theme);
  const setTheme = useThemeStore((s) => s.setTheme);

  // v0.53b — once the user enters the theme step,
  // mark theme as configured. (The actual theme
  // may have been set earlier from the topbar; this
  // is just a "we walked past this step" marker.)
  useEffect(() => {
    // No-op marker; the welcome-store update
    // happens via the Next button handler in
    // Welcome.tsx (or via direct calls when the
    // user picks a theme). For now the per-step
    // configured flag is a separate concern.
  }, []);

  return (
    <div className="space-y-4 py-2">
      <div>
        <h2 className="text-[18px] font-semibold text-fg">
          {t('welcome.theme_title')}
        </h2>
        <p className="text-[12px] text-muted mt-1">
          {t('welcome.theme_desc')}
        </p>
      </div>
      <div className="grid grid-cols-3 gap-3">
        {themes.map((th) => (
          <button
            key={th.id}
            type="button"
            onClick={() => setTheme(th.id)}
            data-testid={`welcome-theme-${th.id}`}
            className={cn(
              'rounded-md border p-4 text-left transition-colors',
              theme === th.id
                ? 'bg-accent/10 border-accent/40'
                : 'bg-surface-2 border-border hover:bg-surface-hover',
            )}
          >
            <div className="flex items-center gap-2 mb-2">
              <div
                className="w-4 h-4 rounded-full"
                style={{
                  background:
                    th.id === 'dark'
                      ? '#1E1E1E'
                      : th.id === 'light'
                        ? '#FFFFFF'
                        : '#10A37F',
                  border: '1px solid var(--border)',
                }}
              />
              <span className="text-[13px] font-medium text-fg">
                {th.name}
              </span>
              {theme === th.id && (
                <CheckCircle2 className="w-3.5 h-3.5 text-accent ml-auto" />
              )}
            </div>
            <div className="text-[11px] text-muted">{th.desc}</div>
          </button>
        ))}
      </div>
    </div>
  );
}
