/**
 * L1 — i18n foundation (v0.9d).
 *
 * Minimal, zero-dependency translation system. Just enough to:
 *   - register a string dictionary per locale
 *   - look up by key with `{{var}}` interpolation
 *   - expose a React context for components
 *   - persist the user's locale choice (zustand)
 *
 * NOT in scope for v0.9d:
 *   - Pluralization rules (`{n, plural, one {...} other {...}}`)
 *   - Date/number formatting (use Intl directly)
 *   - Lazy-loaded locale files (everything is in one bundle)
 *   - ICU MessageFormat
 *
 * Adding a string:
 *   1. Add a key to the `en` dictionary (the source of truth)
 *   2. Add the same key to the other locales (Chinese for v0.9d)
 *   3. Use `t('key.name', { var: 'value' })` in components
 *
 * If a key is missing in the active locale, the `en` version is
 * used as a fallback and a console.warn fires in dev. We never
 * throw — broken translations are a UX bug, not a crash.
 */

import { create } from 'zustand';
import { persist } from 'zustand/middleware';

export type Locale = 'en' | 'zh';

export const SUPPORTED_LOCALES: Locale[] = ['en', 'zh'];
export const DEFAULT_LOCALE: Locale = 'en';

/** Human-readable label for each locale (used in the UI). */
export const LOCALE_LABEL: Record<Locale, string> = {
  en: 'English',
  zh: '中文',
};

/** Type-safe dictionary shape. Every locale must match it. */
export type Dictionary = Record<string, string>;

const en: Dictionary = {
  'app.name': 'polyrocket',
  'app.tagline': 'Local-first Polymarket analysis',
  'nav.dashboard': 'Dashboard',
  'nav.markets': 'Markets',
  'nav.signals': 'Signals',
  'nav.copy': 'Copy',
  'nav.pnl': 'P&L',
  'nav.lab': 'Model Lab',
  'nav.settings': 'Settings',
  'common.search': 'Search…',
  'common.cancel': 'Cancel',
  'common.save': 'Save',
  'common.retry': 'Retry',
  'common.loading': 'Loading…',
  'common.error': 'Something went wrong',
  'common.close': 'Close',
  'kbd.title': 'Keyboard shortcuts',
  'kbd.tip': 'Press a key, then the route key. Esc to cancel.',
  'palette.placeholder': 'Type a command…',
  'palette.empty': 'No matches',
  'palette.category.navigate': 'Navigate',
  'palette.category.actions': 'Actions',
  'palette.cmd.sync': 'Sync markets',
  'palette.cmd.recompute': 'Recompute signals',
  'palette.cmd.reset': 'Reset demo data',
  'palette.cmd.purge': 'Purge audit log',
  'palette.cmd.help': 'Show keyboard shortcuts',
  'audit.purged': 'Purged {{n}} audit rows',
  'seed.populated': 'First-run seeder populated {{n}} rows',
  'error.db': 'A local database operation failed.',
  'error.http': 'A network call to an external service failed.',
  'error.keyring': 'OS keyring is unavailable.',
  'error.invalid': 'The request was rejected by validation.',
  'error.not_found': 'The requested item no longer exists.',
  'error.internal': 'An internal error occurred.',
  'error.unknown': 'An unknown error occurred.',
};

const zh: Dictionary = {
  'app.name': 'polyrocket',
  'app.tagline': '本地优先的 Polymarket 分析',
  'nav.dashboard': '仪表盘',
  'nav.markets': '市场',
  'nav.signals': '信号',
  'nav.copy': '跟单',
  'nav.pnl': '盈亏',
  'nav.lab': '模型实验室',
  'nav.settings': '设置',
  'common.search': '搜索…',
  'common.cancel': '取消',
  'common.save': '保存',
  'common.retry': '重试',
  'common.loading': '加载中…',
  'common.error': '出错了',
  'common.close': '关闭',
  'kbd.title': '键盘快捷键',
  'kbd.tip': '先按前缀键，再按路由键。Esc 取消。',
  'palette.placeholder': '输入命令…',
  'palette.empty': '无匹配',
  'palette.category.navigate': '导航',
  'palette.category.actions': '操作',
  'palette.cmd.sync': '同步市场',
  'palette.cmd.recompute': '重算信号',
  'palette.cmd.reset': '重置示例数据',
  'palette.cmd.purge': '清理审计日志',
  'palette.cmd.help': '显示键盘快捷键',
  'audit.purged': '已清理 {{n}} 条审计记录',
  'seed.populated': '首次启动注入器已写入 {{n}} 行',
  'error.db': '本地数据库操作失败。',
  'error.http': '外部网络请求失败。',
  'error.keyring': '系统钥匙串不可用。',
  'error.invalid': '请求被校验拒绝。',
  'error.not_found': '请求的项目已不存在。',
  'error.internal': '内部错误。',
  'error.unknown': '未知错误。',
};

const dictionaries: Record<Locale, Dictionary> = { en, zh };

/**
 * Look up a key in a dictionary with `{{var}}` interpolation.
 * Returns the raw key (with `?` prefix) if not found, so the
 * missing translation is visible in the UI.
 */
export function translate(locale: Locale, key: string, vars?: Record<string, string | number>): string {
  const dict = dictionaries[locale] ?? dictionaries[DEFAULT_LOCALE];
  let template = dict[key];
  if (template == null) {
    // Fallback: en, then the key itself
    template = dictionaries[DEFAULT_LOCALE][key] ?? `?${key}?`;
    if (typeof import.meta !== 'undefined' && (import.meta as any).env?.DEV) {
      // eslint-disable-next-line no-console
      console.warn(`[i18n] missing key: ${key} (locale: ${locale})`);
    }
  }
  if (!vars) return template;
  return template.replace(/\{\{(\w+)\}\}/g, (_, name) => {
    const v = vars[name];
    return v == null ? `{{${name}}}` : String(v);
  });
}

// -----------------------------------------------------------------------------
// React state
// -----------------------------------------------------------------------------

interface LocaleState {
  locale: Locale;
  setLocale: (l: Locale) => void;
}

export const useLocaleStore = create<LocaleState>()(
  persist(
    (set) => ({
      locale: DEFAULT_LOCALE,
      setLocale: (l) => set({ locale: l }),
    }),
    { name: 'polyrocket.locale' },
  ),
);

/** Hook: returns the current locale + a `t(key, vars?)` helper. */
export function useT() {
  const locale = useLocaleStore((s) => s.locale);
  return {
    locale,
    t: (key: string, vars?: Record<string, string | number>) => translate(locale, key, vars),
  };
}
