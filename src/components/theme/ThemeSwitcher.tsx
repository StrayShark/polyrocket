import { Moon, Sun, Circle } from 'lucide-react';
import { useThemeStore, type Theme } from '@/stores/theme-store';
import { cn } from '@/lib/cn';

const ORDER: Theme[] = ['dark', 'light', 'matrix'];

const ICONS: Record<Theme, typeof Moon> = {
  dark: Moon,
  light: Sun,
  matrix: Circle,
};

const LABELS: Record<Theme, string> = {
  dark: 'Dark',
  light: 'Light',
  matrix: 'Matrix',
};

interface ThemeSwitcherProps {
  variant?: 'segmented' | 'dropdown';
  className?: string;
}

export function ThemeSwitcher({ variant = 'segmented', className }: ThemeSwitcherProps) {
  const theme = useThemeStore((s) => s.theme);
  const setTheme = useThemeStore((s) => s.setTheme);

  if (variant === 'segmented') {
    return (
      <div
        className={cn(
          'inline-flex items-center gap-0.5 p-0.5 rounded-md border bg-surface-2',
          'border-border',
          className,
        )}
      >
        {ORDER.map((t) => {
          const Icon = ICONS[t];
          const active = t === theme;
          return (
            <button
              key={t}
              type="button"
              onClick={() => setTheme(t)}
              aria-pressed={active}
              aria-label={`Switch to ${LABELS[t]} theme`}
              className={cn(
                'inline-flex items-center gap-1 px-2 py-1 text-xs rounded transition-colors',
                active
                  ? 'bg-surface text-fg shadow-card'
                  : 'text-muted hover:text-fg hover:bg-surface-hover',
              )}
            >
              <Icon className="w-3 h-3" />
              <span>{LABELS[t]}</span>
            </button>
          );
        })}
      </div>
    );
  }

  // dropdown placeholder — to be filled with shadcn DropdownMenu later
  return null;
}