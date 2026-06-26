/**
 * 语言切换下拉框(v0.9d)。
 * 放在顶栏,点击后可选 en 或 zh。
 */

import { useState } from 'react';
import { Globe } from 'lucide-react';
import {
  LOCALE_LABEL,
  SUPPORTED_LOCALES,
  useLocaleStore,
  type Locale,
} from '@/lib/i18n';

export function LocaleSwitcher() {
  const locale = useLocaleStore((s) => s.locale);
  const setLocale = useLocaleStore((s) => s.setLocale);
  const [open, setOpen] = useState(false);

  return (
    <div className="relative">
      <button
        type="button"
        onClick={() => setOpen((o) => !o)}
        className="w-7 h-7 grid place-items-center rounded-md hover:bg-surface-hover"
        style={{ color: 'var(--fg-secondary)' }}
        title={`Language · ${LOCALE_LABEL[locale]}`}
        aria-label="Switch language"
        data-testid="locale-switcher"
      >
        <Globe className="w-3.5 h-3.5" />
      </button>
      {open && (
        <div
          className="absolute right-0 top-9 z-40 w-32 rounded-md border bg-surface border-border shadow-lg py-1"
          role="menu"
        >
          {SUPPORTED_LOCALES.map((l: Locale) => (
            <button
              key={l}
              type="button"
              onClick={() => { setLocale(l); setOpen(false); }}
              className={`w-full text-left px-3 py-1.5 text-[12px] ${
                l === locale ? 'text-fg font-medium bg-surface-hover' : 'text-muted'
              } hover:bg-surface-hover`}
              data-testid={`locale-option-${l}`}
            >
              {LOCALE_LABEL[l]}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
