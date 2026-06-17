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
  'nav.settings_section': 'Settings',
  'nav.preferences': 'Preferences',
  'nav.categories': 'Categories',
  // Page titles (v0.11a)
  'page.dashboard': 'Dashboard',
  'page.markets': 'Markets',
  'page.market_detail': 'Market Detail',
  'page.signals': 'Active Signals',
  'page.copy': 'Copy Trading',
  'page.pnl': 'P&L',
  'page.lab': 'Model Lab',
  'page.history': 'Bet History',
  'page.wallets': 'Wallets',
  'page.settings': 'Settings',
  'page.analysis': 'Analysis',
  'page.llm_perf': 'LLM Performance',
  'page.llm_mgmt': 'LLM Management',
  'page.brief': 'Daily Brief',
  'page.onboarding': 'Onboarding',
  'page.audit': 'Audit Log',
  'page.notifications': 'Notifications',
  'page.help': 'Help',
  // Status / generic
  'status.loading': 'Loading…',
  'status.empty': 'Nothing here yet.',
  'status.error_prefix': 'Error',
  // History page (v0.12b)
  'history.kpi.total': 'Total Bets',
  'history.kpi.open': 'Open',
  'history.kpi.winrate': 'Win Rate',
  'history.kpi.pnl': 'Realized PnL',
  'history.refresh': 'Refresh',
  'history.empty.title': 'No bets',
  'history.empty.all': 'No bets placed yet. Use a Market Detail page to place your first bet.',
  'history.empty.filtered': 'No bets with status "{{status}}".',
  'history.filter.all': 'all',
  'history.filter.open': 'open',
  'history.filter.won': 'won',
  'history.filter.lost': 'lost',
  'history.filter.cancelled': 'cancelled',
  'history.col.market': 'Market',
  'history.col.side': 'Side',
  'history.col.size': 'Size',
  'history.col.price': 'Price',
  'history.col.status': 'Status',
  'history.col.placed': 'Placed',
  'history.col.pnl': 'P&L',
  'history.col.tx': 'Tx',
  'history.profit': 'profit',
  'history.loss': 'loss',
  // Onboarding page
  'onboarding.welcome': 'Welcome to polyrocket',
  'onboarding.subtitle': 'Get started in 3 steps',
  'onboarding.welcome_toast': 'Welcome to polyrocket!',
  'onboarding.tagline': 'A Polymarket analysis desktop client with multi-LLM fan-out, signal detection, copy trading, and self-custodial key storage.',
  'onboarding.feature.multi_llm': 'Multi-LLM',
  'onboarding.feature.self_custody': 'Self-custody',
  'onboarding.feature.os_keyring': 'OS keyring',
  // Settings page
  'settings.section.appearance': 'Appearance',
  'settings.section.language': 'Language',
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
  'nav.settings_section': '设置',
  'nav.preferences': '偏好',
  'nav.categories': '分类',
  // Page titles (v0.11a)
  'page.dashboard': '仪表盘',
  'page.markets': '市场',
  'page.market_detail': '市场详情',
  'page.signals': '活跃信号',
  'page.copy': '跟单交易',
  'page.pnl': '盈亏',
  'page.lab': '模型实验室',
  'page.history': '投注历史',
  'page.wallets': '钱包',
  'page.settings': '设置',
  'page.analysis': '分析',
  'page.llm_perf': 'LLM 表现',
  'page.llm_mgmt': 'LLM 管理',
  'page.brief': '每日简报',
  'page.onboarding': '引导',
  'page.audit': '审计日志',
  'page.notifications': '通知',
  'page.help': '帮助',
  // Status / generic
  'status.loading': '加载中…',
  'status.empty': '暂无数据。',
  'status.error_prefix': '错误',
  // History page (v0.12b)
  'history.kpi.total': '总投注数',
  'history.kpi.open': '进行中',
  'history.kpi.winrate': '胜率',
  'history.kpi.pnl': '已实现盈亏',
  'history.refresh': '刷新',
  'history.empty.title': '暂无投注',
  'history.empty.all': '还没有投注记录。前往某个市场详情页下注。',
  'history.empty.filtered': '没有状态为 "{{status}}" 的投注。',
  'history.filter.all': '全部',
  'history.filter.open': '进行中',
  'history.filter.won': '已胜',
  'history.filter.lost': '已负',
  'history.filter.cancelled': '已取消',
  'history.col.market': '市场',
  'history.col.side': '方向',
  'history.col.size': '数量',
  'history.col.price': '价格',
  'history.col.status': '状态',
  'history.col.placed': '下单时间',
  'history.col.pnl': '盈亏',
  'history.col.tx': 'Tx',
  'history.profit': '盈利',
  'history.loss': '亏损',
  // Onboarding page
  'onboarding.welcome': '欢迎使用 polyrocket',
  'onboarding.subtitle': '3 步快速开始',
  'onboarding.welcome_toast': '欢迎使用 polyrocket！',
  'onboarding.tagline': 'Polymarket 桌面分析客户端, 支持多 LLM fan-out、信号检测、跟单交易与自托管密钥。',
  'onboarding.feature.multi_llm': '多 LLM',
  'onboarding.feature.self_custody': '自托管',
  'onboarding.feature.os_keyring': '系统钥匙串',
  // Settings page
  'settings.section.appearance': '外观',
  'settings.section.language': '语言',
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
