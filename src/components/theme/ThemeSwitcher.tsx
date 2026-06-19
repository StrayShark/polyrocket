// polyrocket — ThemeSwitcher (v0.67c density).
//
// Three-theme picker (dark / light / matrix) rendered as a
// segmented control. Used in:
//   - sidebar footer (AppShell)
//   - Settings page (per-user preference)
//
// **Why segmented, not dropdown**: the project has only 3
// themes. Segmented control shows all options at a glance —
// 1 click to pick. A dropdown adds a click to expand before
// the user can pick. At 3 options the segmented control
// wins on speed.
//
// **Persistence**: the active theme is stored in
// `useThemeStore` (zustand + persist). Reloading the app
// restores the last-chosen theme. The DOM `data-theme`
// attribute is set by `stores/theme-store.ts` (effect).
//
// **CSS variables**: theme colors come from CSS variables
// in `src/styles/*.css`. Each theme has its own variable
// definitions. The switcher only flips `data-theme` —
// the actual color values are in the stylesheets.

import { Moon, Sun, Circle } from 'lucide-react';
import { useThemeStore, type Theme } from '@/stores/theme-store';
import { cn } from '@/lib/cn';

// Display order. Matters: Dark first (default), Light second
// (most common), Matrix third (niche / power-user).
const ORDER: Theme[] = ['dark', 'light', 'matrix'];

// Lucide icon per theme. Moon = dark, Sun = light, Circle = matrix.
const ICONS: Record<Theme, typeof Moon> = {
  dark: Moon,
  light: Sun,
  matrix: Circle,
};

// Human-readable label per theme. Used for the button text
// AND the aria-label ("Switch to {label} theme").
const LABELS: Record<Theme, string> = {
  dark: 'Dark',
  light: 'Light',
  matrix: 'Matrix',
};

/** Props for `<ThemeSwitcher>`. */
interface ThemeSwitcherProps {
  /**
   * Visual style. `segmented` (default) is a horizontal
   * row of 3 buttons. `dropdown` is reserved for a future
   * shadcn DropdownMenu — currently returns `null`.
   */
  variant?: 'segmented' | 'dropdown';
  /** Extra Tailwind class names for the root container. */
  className?: string;
}

/**
 * Render the 3-theme picker. In `segmented` mode (default),
 * renders 3 buttons in a row with the active one highlighted.
 * In `dropdown` mode, returns `null` (placeholder for a
 * future compact variant).
 */
export function ThemeSwitcher({ variant = 'segmented', className }: ThemeSwitcherProps) {
  // v0.67c — read both theme + setter from the same store
  // via two selectors (zustand pattern). The store is
  // persisted to localStorage; a fresh mount restores the
  // last selection.
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
          // Resolve icon + active state per theme
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
                // Layout: inline-flex icon + label, padding 2 / 1.
                // Active: surface background + fg text + card shadow.
                // Inactive: muted text, hover transitions to surface-hover.
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