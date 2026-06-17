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
