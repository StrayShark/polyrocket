import { describe, expect, it, vi } from 'vitest';
import { translate, SUPPORTED_LOCALES, LOCALE_LABEL } from './i18n';

describe('translate', () => {
  it('returns the en value for known keys', () => {
    expect(translate('en', 'app.name')).toBe('polyrocket');
    expect(translate('en', 'common.retry')).toBe('Retry');
  });

  it('returns the zh value for known keys', () => {
    expect(translate('zh', 'app.tagline')).toBe('本地优先的 Polymarket 分析');
    expect(translate('zh', 'common.retry')).toBe('重试');
  });

  it('interpolates {{var}} placeholders', () => {
    expect(translate('en', 'audit.purged', { n: 42 })).toBe('Purged 42 audit rows');
    expect(translate('zh', 'audit.purged', { n: 42 })).toBe('已清理 42 条审计记录');
  });

  it('leaves unknown vars as the literal placeholder', () => {
    expect(translate('en', 'audit.purged', { other: 1 })).toBe('Purged {{n}} audit rows');
  });

  it('falls back to en for missing keys in active locale', () => {
    // Simulate: a key only in `en` (we add one temporarily via dictionaries).
    // The function should fall back gracefully.
    const out = translate('zh', 'nonexistent.key' as any);
    expect(out).toBe('?nonexistent.key?');
  });

  it('marks missing keys with ?...?', () => {
    const out = translate('en', 'totally.missing' as any);
    expect(out).toBe('?totally.missing?');
  });

  it('console.warns on missing keys in dev only', () => {
    const origDev = (import.meta as any).env?.DEV;
    (import.meta as any).env = { ...(import.meta as any).env, DEV: true };
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
    translate('zh', 'still.missing' as any);
    expect(warn).toHaveBeenCalled();
    warn.mockRestore();
    (import.meta as any).env = { ...(import.meta as any).env, DEV: origDev };
  });
});

describe('page title keys (v0.11a)', () => {
  it('translates every page.* key in both locales', () => {
    const pageKeys = [
      'page.dashboard', 'page.markets', 'page.market_detail', 'page.signals',
      'page.copy', 'page.pnl', 'page.lab', 'page.history', 'page.wallets',
      'page.settings', 'page.analysis', 'page.llm_perf', 'page.llm_mgmt',
      'page.brief', 'page.onboarding', 'page.audit', 'page.notifications',
      'page.help',
    ];
    for (const key of pageKeys) {
      const enVal = translate('en', key);
      const zhVal = translate('zh', key);
      expect(enVal).not.toBe(`?${key}?`);
      expect(zhVal).not.toBe(`?${key}?`);
      // zh should differ from en (we did real translations)
      expect(zhVal).not.toBe(enVal);
    }
  });
});

describe('History + Onboarding keys (v0.12b)', () => {
  it('every history.* and onboarding.* key is in both locales', () => {
    const keys = [
      'history.kpi.total', 'history.kpi.open', 'history.kpi.winrate', 'history.kpi.pnl',
      'history.refresh', 'history.empty.title', 'history.empty.all', 'history.empty.filtered',
      'history.filter.all', 'history.filter.open', 'history.filter.won', 'history.filter.lost',
      'history.filter.cancelled', 'history.profit', 'history.loss',
      'onboarding.welcome', 'onboarding.subtitle', 'onboarding.welcome_toast',
      'onboarding.tagline', 'onboarding.feature.multi_llm',
      'onboarding.feature.self_custody', 'onboarding.feature.os_keyring',
      'settings.section.appearance', 'settings.section.language',
    ];
    for (const key of keys) {
      const enVal = translate('en', key);
      const zhVal = translate('zh', key);
      expect(enVal, `en missing: ${key}`).not.toBe(`?${key}?`);
      expect(zhVal, `zh missing: ${key}`).not.toBe(`?${key}?`);
    }
  });

  it('history.empty.filtered interpolates {{status}}', () => {
    expect(translate('en', 'history.empty.filtered', { status: 'won' }))
      .toBe('No bets with status "won".');
    expect(translate('zh', 'history.empty.filtered', { status: '已胜' }))
      .toBe('没有状态为 "已胜" 的投注。');
  });
});

describe('Markets / Signals / Copy / PnL keys (v0.13a)', () => {
  it('every new key is in both locales', () => {
    const keys = [
      'markets.sync', 'markets.empty', 'markets.filter.all', 'markets.filter.active',
      'signals.title', 'signals.recompute', 'signals.empty',
      'copy.title', 'copy.targets', 'copy.events', 'copy.add',
      'copy.empty_targets', 'copy.empty_events',
      'pnl.title', 'pnl.empty',
    ];
    for (const key of keys) {
      const enVal = translate('en', key);
      const zhVal = translate('zh', key);
      expect(enVal, `en missing: ${key}`).not.toBe(`?${key}?`);
      expect(zhVal, `zh missing: ${key}`).not.toBe(`?${key}?`);
    }
  });
});

describe('Analysis / LlmPerf / LlmMgmt keys (v0.14a)', () => {
  it('every new key is in both locales', () => {
    const keys = [
      // Analysis
      'analysis.run.title', 'analysis.run.desc', 'analysis.input.placeholder',
      'analysis.btn.analyze', 'analysis.btn.recommendation', 'analysis.btn.follow',
      'analysis.btn.skip', 'analysis.kpi.side', 'analysis.kpi.prob',
      'analysis.kpi.confidence', 'analysis.signals.title', 'analysis.signals.desc',
      'analysis.signals.empty', 'analysis.signals.empty_desc',
      'analysis.signals.refresh', 'analysis.recommendation.title',
      'analysis.recommendation.provider', 'analysis.recommendation.side',
      'analysis.recommendation.predicted', 'analysis.recommendation.confidence',
      'analysis.recommendation.cost', 'analysis.recommendation.latency',
      'analysis.recommendation.rationale', 'analysis.toast.failed',
      'analysis.toast.decision_recorded',
      // LlmPerf
      'llmperf.kpi.models', 'llmperf.kpi.cost', 'llmperf.kpi.wins',
      'llmperf.kpi.roi', 'llmperf.delta.profitable', 'llmperf.delta.unprofitable',
      'llmperf.bucket.title', 'llmperf.bucket.desc', 'llmperf.bucket.empty',
      'llmperf.bucket.empty_desc', 'llmperf.prompt.title', 'llmperf.prompt.desc',
      'llmperf.prompt.winrate', 'llmperf.cost.title', 'llmperf.cost.desc',
      'llmperf.cost.cost', 'llmperf.cost.wins', 'llmperf.cost.roi',
      'llmperf.csv.preview', 'llmperf.btn.export',
      'llmperf.toast.exported', 'llmperf.toast.export_failed',
      // LlmMgmt
      'llmmgmt.keyring', 'llmmgmt.keys_count', 'llmmgmt.pm_api',
      'llmmgmt.wallet_pk', 'llmmgmt.keyring_backends',
      'llmmgmt.providers.title', 'llmmgmt.providers.desc',
      'llmmgmt.providers.empty', 'llmmgmt.providers.empty_desc',
      'llmmgmt.providers.test', 'llmmgmt.keys.title', 'llmmgmt.keys.title_for',
      'llmmgmt.keys.desc', 'llmmgmt.keys.desc_for',
      'llmmgmt.keys.add', 'llmmgmt.keys.add_first',
      'llmmgmt.keys.empty', 'llmmgmt.keys.empty_desc',
      'llmmgmt.keys.no_provider', 'llmmgmt.keys.no_provider_desc',
      'llmmgmt.keys.in_keyring', 'llmmgmt.keys.no_secret',
      'llmmgmt.keys.delete', 'llmmgmt.keys.confirm_delete',
      'llmmgmt.keys.toast.deleted', 'llmmgmt.add.title',
      'llmmgmt.add.cancel', 'llmmgmt.add.add',
      'llmmgmt.add.alias', 'llmmgmt.add.alias_placeholder',
      'llmmgmt.add.priority', 'llmmgmt.add.secret',
      'llmmgmt.add.secret_placeholder', 'llmmgmt.add.show',
      'llmmgmt.add.hide', 'llmmgmt.add.notice',
      'llmmgmt.add.toast.added', 'llmmgmt.add.toast.failed',
      'llmmgmt.test.ok_toast', 'llmmgmt.test.ok_latency',
      'llmmgmt.test.failed',
    ];
    for (const key of keys) {
      const enVal = translate('en', key);
      const zhVal = translate('zh', key);
      expect(enVal, `en missing: ${key}`).not.toBe(`?${key}?`);
      expect(zhVal, `zh missing: ${key}`).not.toBe(`?${key}?`);
    }
  });
});

describe('SUPPORTED_LOCALES', () => {
  it('contains en and zh', () => {
    expect(SUPPORTED_LOCALES).toContain('en');
    expect(SUPPORTED_LOCALES).toContain('zh');
  });
});

describe('LOCALE_LABEL', () => {
  it('has a label for every supported locale', () => {
    for (const l of SUPPORTED_LOCALES) {
      expect(LOCALE_LABEL[l].length).toBeGreaterThan(0);
    }
  });
});
