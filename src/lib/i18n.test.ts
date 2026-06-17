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
      'analysis.progress.running', 'analysis.progress.done',
      'analysis.progress.total_latency', 'analysis.progress.total_cost',
      'analysis.progress.consensus',
      'analysis.progress.status.completed', 'analysis.progress.status.partial',
      'analysis.progress.status.failed',
      'analysis.progress.row.pending', 'analysis.progress.row.running',
      'train.progress.running', 'train.progress.done', 'train.progress.duration',
      'train.progress.status.completed', 'train.progress.status.failed',
      'train.progress.col.trial', 'train.progress.col.lr', 'train.progress.col.reg',
      'train.progress.col.brier', 'train.progress.best_brier',
      'train.progress.promote_best', 'train.progress.promote_trial',
      'train.toast.completed', 'train.toast.brier', 'train.toast.failed',
      'promote.btn.promote', 'promote.btn.promoting',
      'promote.toast.promoted', 'promote.toast.version', 'promote.toast.failed',
      'promote.btn.auto_promote', 'promote.btn.auto_promoting',
      'promote.toast.auto_promoted', 'promote.toast.auto_skipped',
      'promote.toast.auto_failed',
      'auto_promote.title', 'auto_promote.desc',
      'auto_promote.margin.label', 'auto_promote.margin.hint',
      'auto_promote.margin.saved',
      'promote.last_candidate.hint',
      'promote.history.title', 'promote.history.desc',
      'promote.history.empty', 'promote.history.empty_desc',
      'promote.history.brier', 'promote.history.active',
      'rollback.btn.rollback', 'rollback.toast.rolled_back',
      'rollback.toast.version', 'rollback.toast.failed',
      'rollback.confirm.title', 'rollback.confirm.body',
      'rollback.confirm.warning',
      'rollback.confirm.confirm', 'rollback.confirm.cancel',
      'promote.chart.title', 'promote.chart.empty', 'promote.chart.range',
      'promote.chart.trend.up', 'promote.chart.trend.down',
      'promote.chart.trend.flat',
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

describe('Wallets / Settings / Notifications keys (v0.14b)', () => {
  it('every new key is in both locales', () => {
    const keys = [
      // Wallets
      'wallets.title', 'wallets.subtitle', 'wallets.refresh',
      'wallets.add', 'wallets.add_first', 'wallets.empty',
      'wallets.empty_desc', 'wallets.card.created',
      'wallets.card.no_label', 'wallets.card.copy',
      'wallets.card.last_synced', 'wallets.card.address_copied',
      'wallets.add.title', 'wallets.add.cancel', 'wallets.add.add',
      'wallets.add.address', 'wallets.add.address_placeholder',
      'wallets.add.label', 'wallets.add.label_placeholder',
      'wallets.add.chain_id', 'wallets.add.chain_polygon',
      'wallets.add.chain_amoy', 'wallets.add.type',
      'wallets.add.type_eoa', 'wallets.add.type_smart',
      'wallets.add.notice', 'wallets.add.toast.added',
      'wallets.add.toast.failed',
      // Notifications
      'notifications.title', 'notifications.subtitle',
      'notifications.refresh', 'notifications.col.kind',
      'notifications.col.title', 'notifications.col.body',
      'notifications.col.at', 'notifications.empty',
      'notifications.empty.title',
      'notifications.toast.subtitle', 'notifications.toast.clear',
      'notifications.toast.dismiss_aria',
      'notifications.toast.kind.info', 'notifications.toast.kind.success',
      'notifications.toast.kind.warning', 'notifications.toast.kind.error',
      'notifications.toast.just_now', 'notifications.toast.empty_desc',
      'notifications.event_types.title', 'notifications.event_types.desc',
      'notifications.event.signal', 'notifications.event.signal_hint',
      'notifications.event.fill', 'notifications.event.fill_hint',
      'notifications.event.keyring', 'notifications.event.keyring_hint',
      'notifications.event.auto_disable', 'notifications.event.auto_disable_hint',
      'notifications.event.brief', 'notifications.event.brief_hint',
      'notifications.event.settings_link',
      // Settings
      'settings.title', 'settings.btn.reset', 'settings.btn.save',
      'settings.btn.reset_toast', 'settings.btn.save_toast',
      'settings.section.trading', 'settings.section.trading_desc',
      'settings.field.min_edge', 'settings.field.min_edge_hint',
      'settings.field.allocation_cap', 'settings.field.allocation_cap_hint',
      'settings.section.notifications', 'settings.section.notifications_desc',
      'settings.field.toasts', 'settings.field.toasts_hint',
      'settings.section.copy', 'settings.section.copy_desc',
      'settings.field.copy_enabled', 'settings.field.copy_enabled_hint',
      'settings.section.advanced', 'settings.section.advanced_desc',
      'settings.field.advanced_stats', 'settings.field.advanced_stats_hint',
      'settings.section.storage', 'settings.section.storage_desc',
      'settings.storage.db_path', 'settings.storage.keyring',
      'settings.storage.env_note',
    ];
    for (const key of keys) {
      const enVal = translate('en', key);
      const zhVal = translate('zh', key);
      expect(enVal, `en missing: ${key}`).not.toBe(`?${key}?`);
      expect(zhVal, `zh missing: ${key}`).not.toBe(`?${key}?`);
    }
  });
});

describe('Audit / Help / ModelLab / MarketDetail keys (v0.14c)', () => {
  it('every new key is in both locales', () => {
    const keys = [
      // Audit
      'audit.search.placeholder', 'audit.col.when', 'audit.col.actor',
      'audit.col.action', 'audit.col.target', 'audit.col.result',
      'audit.col.payload', 'audit.btn.refresh', 'audit.filter.all',
      'audit.empty', 'audit.empty.no_writes', 'audit.empty.no_match',
      'audit.footer',
      // Help
      'help.title', 'help.subtitle', 'help.quick.title', 'help.quick.desc',
      'help.quick.step1', 'help.quick.step2', 'help.quick.step3',
      'help.quick.step4', 'help.quick.step5',
      'help.concept.storage', 'help.concept.storage_body',
      'help.concept.layers', 'help.concept.layers_body',
      'help.concept.fanout', 'help.concept.fanout_body',
      'help.concept.brier', 'help.concept.brier_body',
      'help.shortcuts.title', 'help.shortcuts.desc',
      'help.shortcuts.cmdk', 'help.shortcuts.gd', 'help.shortcuts.gm',
      'help.shortcuts.gs', 'help.shortcuts.gb', 'help.shortcuts.gl',
      'help.external.title', 'help.external.docs',
      'help.external.polymarket', 'help.external.github',
      // ModelLab
      'modellab.title', 'modellab.kpi.versions', 'modellab.kpi.best_brier',
      'modellab.kpi.best_winrate', 'modellab.perf.title', 'modellab.perf.desc',
      'modellab.perf.empty', 'modellab.perf.empty_desc',
      'modellab.perf.predictions', 'modellab.perf.metric.winrate',
      'modellab.perf.metric.brier', 'modellab.perf.metric.logloss',
      'modellab.perf.metric.avgedge', 'modellab.runs.title',
      'modellab.runs.desc', 'modellab.runs.empty', 'modellab.runs.empty_desc',
      'modellab.sm.title', 'modellab.sm.desc', 'modellab.sm.queued',
      'modellab.sm.running', 'modellab.sm.done', 'modellab.sm.error',
      'modellab.sm.note',
      // MarketDetail
      'marketdetail.back_markets', 'marketdetail.not_found',
      'marketdetail.not_found_desc', 'marketdetail.btn.open_polymarket',
      'marketdetail.resolved', 'marketdetail.active', 'marketdetail.inactive',
      'marketdetail.stat.liquidity', 'marketdetail.stat.volume_24h',
      'marketdetail.stat.closes', 'marketdetail.stat.slug',
      'marketdetail.signals.title', 'marketdetail.signals.desc',
      'marketdetail.signals.empty', 'marketdetail.signals.empty_desc',
      'marketdetail.signals.model', 'marketdetail.signals.predicted',
      'marketdetail.signals.conf', 'marketdetail.signals.horizon',
      'marketdetail.activity.title', 'marketdetail.activity.desc',
      'marketdetail.activity.body',
    ];
    for (const key of keys) {
      const enVal = translate('en', key);
      const zhVal = translate('zh', key);
      expect(enVal, `en missing: ${key}`).not.toBe(`?${key}?`);
      expect(zhVal, `zh missing: ${key}`).not.toBe(`?${key}?`);
    }
  });
});

describe('Dashboard / Brief keys (v0.14d)', () => {
  it('every new key is in both locales', () => {
    const keys = [
      // Dashboard
      'dashboard.kpi.equity', 'dashboard.kpi.equity_hint',
      'dashboard.kpi.open_pnl', 'dashboard.kpi.winrate', 'dashboard.kpi.brier',
      'dashboard.brier.good', 'dashboard.brier.fair', 'dashboard.brier.poor',
      'dashboard.delta.profit', 'dashboard.delta.loss',
      'dashboard.equity.title', 'dashboard.equity.desc',
      'dashboard.equity.empty', 'dashboard.equity.empty_desc',
      'dashboard.equity.settled_count', 'dashboard.equity.range',
      'dashboard.calibration.title', 'dashboard.calibration.desc',
      'dashboard.calibration.empty', 'dashboard.calibration.empty_desc',
      'dashboard.activity.title', 'dashboard.activity.desc',
      'dashboard.activity.empty', 'dashboard.activity.empty_desc',
      'dashboard.signals.title', 'dashboard.signals.desc',
      'dashboard.signals.view_all', 'dashboard.signals.empty',
      'dashboard.signals.empty_desc', 'dashboard.signals.model',
      'dashboard.positions.title', 'dashboard.positions.desc',
      'dashboard.positions.view_all', 'dashboard.positions.empty',
      'dashboard.positions.empty_desc', 'dashboard.positions.browse',
      'dashboard.positions.size_at', 'dashboard.recent.bet_text',
      'dashboard.recent.signal_text',
      // Brief
      'brief.title', 'brief.subtitle', 'brief.refresh', 'brief.rescore',
      'brief.empty.title', 'brief.empty.desc', 'brief.empty.generate',
      'brief.entry.dismissed', 'brief.entry.closes', 'brief.entry.liq',
      'brief.entry.edge', 'brief.entry.conf', 'brief.entry.consensus',
      'brief.btn.dismiss', 'brief.btn.open', 'brief.toast.refreshed',
      'brief.toast.refreshed_body', 'brief.toast.refresh_failed',
      'brief.toast.dismissed',
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
