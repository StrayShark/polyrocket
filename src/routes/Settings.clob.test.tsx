// v0.99 — Settings.tsx ClobFeedCard 测试（+3 个测试，+~6 个语句）。
//
// ClobFeedCard（Settings.tsx:1199）展示 CLOB feed
// 状态（not_configured / configured / connected）并提供
// Refresh 按钮。已有 Settings 测试未覆盖
// 本卡片。相关分支为 3 种状态颜色，以及
// error / loading / loaded 三种 UI 状态。
//
// @vitest-environment happy-dom

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

const { mockGetAuditRetention, mockClobFeedStatus } = vi.hoisted(() => ({
  mockGetAuditRetention: vi.fn(),
  mockClobFeedStatus: vi.fn(),
}));

vi.mock('@/ipc', () => ({
  getAuditRetention: (...args: unknown[]) => mockGetAuditRetention(...args),
  clobFeedStatus: (...args: unknown[]) => mockClobFeedStatus(...args),
  // Settings 还会用到其他 IPC —— 一并 stub 掉
  setAuditRetention: vi.fn().mockResolvedValue(undefined),
  purgeAuditLogNow: vi.fn().mockResolvedValue(undefined),
  setAutoPromoteConfig: vi.fn().mockResolvedValue(undefined),
  getAutoPromoteConfig: vi.fn().mockResolvedValue({ enabled: false, brier_margin: 0.005, min_improvement_pct: 1.0 }),
  getRetentionPolicy: vi.fn().mockResolvedValue({ retain_days: 90, max_rows_k: 50, min_keep: 1000 }),
  setRetentionPolicy: vi.fn().mockResolvedValue({ purged: 0 }),
  setTelemetryEnabled: vi.fn().mockResolvedValue(undefined),
  getTelemetryEnabled: vi.fn().mockResolvedValue(false),
  listTelemetryLogs: vi.fn().mockResolvedValue([]),
  purgeTelemetryLogs: vi.fn().mockResolvedValue(0),
  getActiveModel: vi.fn().mockResolvedValue(null),
  schedulerSelfTestNow: vi.fn().mockResolvedValue({
    processStartedAtUnix: 1700000000, checkedAtUnixMs: 1700000010000,
    allHealthy: true, loops: [],
  }),
  getClobFeeds: vi.fn().mockResolvedValue([]),
  setClobFeeds: vi.fn().mockResolvedValue(undefined),
  explainModel: vi.fn().mockResolvedValue({ ok: true, message: 'ok', importance: [] }),
  shapExplain: vi.fn().mockResolvedValue({ ok: true, message: 'ok', importance: [] }),
  listWallets: vi.fn().mockResolvedValue([]),
  getBankrollConfig: vi.fn().mockResolvedValue({
    kelly_fraction: 0.25, per_signal_cap_pct: 10, reserve_pct: 20,
    min_edge_pct: 5, total_cap_pct: 80, min_confidence: 60,
  }),
  getStorageInfo: vi.fn().mockResolvedValue({
    defaultPath: '/tmp/db/polyrocket.db', currentPath: '/tmp/db/polyrocket.db',
    isCustom: false, exists: true, writable: true, freeBytes: null, restartRequired: false,
  }),
  migrateStoragePath: vi.fn().mockResolvedValue({
    from: '/tmp/db/polyrocket.db', to: '/Volumes/external/polyrocket',
    filesCopied: 2, bytesCopied: 12345, overwritten: false, noop: false,
  }),
  getProxyConfig: vi.fn().mockResolvedValue({ enabled: false, url: null, scheme: null }),
  setProxyConfig: vi.fn().mockResolvedValue(undefined),
  clearProxyConfig: vi.fn().mockResolvedValue(undefined),
  setMirrorPaperMode: vi.fn().mockResolvedValue(true),
  getMirrorPaperMode: vi.fn().mockResolvedValue(false),
  listPromoteHistory: vi.fn().mockResolvedValue({ ok: true, message: 'ok', count: 0, entries: [] }),
  listPromoteHistoryArchive: vi.fn().mockResolvedValue({ ok: true, message: 'ok', count: 0, entries: [] }),
  listLabRuns: vi.fn().mockResolvedValue([]),
  exportPrefs: vi.fn().mockResolvedValue('{"prefs":{}}'),
  importPrefs: vi.fn().mockResolvedValue(undefined),
  downloadPrefsAsFile: vi.fn().mockResolvedValue(undefined),
  parsePrefsFromString: vi.fn().mockResolvedValue({}),
  readFileAsText: vi.fn().mockResolvedValue('{}'),
  getSchedulerSnapshot: vi.fn().mockResolvedValue({ loops: [], events: [] }),
  getPaperMode: vi.fn().mockResolvedValue(false),
  onTrainStarted: vi.fn().mockReturnValue(Promise.resolve(() => {})),
  onTrainFinished: vi.fn().mockReturnValue(Promise.resolve(() => {})),
  onAutoPromoteFinished: vi.fn().mockReturnValue(Promise.resolve(() => {})),
}));

const { mockPrefsState } = vi.hoisted(() => {
  const mockPrefsState = {
    defaultMinEdgePct: 5,
    defaultAllocationCapUsdc: 100,
    copyTradingEnabled: false,
    notificationsEnabled: true,
    advancedStats: false,
    autoPromoteBrierMargin: 0.005,
    autoPromoteAfterTrain: false,
    autoPromoteNotify: true,
    autoPromoteSkippedNotify: false,
    telemetryEnabled: false,
    mirrorPaperMode: false,
    degradationAlertNotify: true,
    setPref: vi.fn(),
    reset: vi.fn(),
  };
  return { mockPrefsState };
});
vi.mock('@/stores/prefs-store', () => ({
  usePrefsStore: (selector?: (s: Record<string, unknown>) => unknown) => {
    if (typeof selector === 'function') return selector(mockPrefsState);
    return mockPrefsState;
  },
}));

vi.mock('@/stores/toast-store', () => ({
  toast: { success: vi.fn(), error: vi.fn(), info: vi.fn() },
  useToastStore: () => ({ toasts: [], push: vi.fn(), dismiss: vi.fn() }),
}));

vi.mock('@/lib/i18n', () => ({
  useT: () => ({ t: (k: string) => k, locale: 'en' as const }),
  useLocaleStore: (selector?: unknown) =>
    typeof selector === 'function'
      ? selector({ locale: 'en' as const, setLocale: vi.fn() })
      : { locale: 'en' as const, setLocale: vi.fn() },
  LOCALE_LABEL: { en: 'English', zh: '中文' },
  SUPPORTED_LOCALES: ['en', 'zh'] as const,
}));

vi.mock('@/stores/theme-store', () => ({
  useThemeStore: (selector?: unknown) =>
    typeof selector === 'function'
      ? selector({ theme: 'dark' as const, setTheme: vi.fn() })
      : { theme: 'dark' as const, setTheme: vi.fn() },
}));

import { Settings } from './Settings';

function wrap() {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false, gcTime: 0 } } });
  return (
    <QueryClientProvider client={qc}>
      <MemoryRouter>
        <Settings />
      </MemoryRouter>
    </QueryClientProvider>
  );
}

beforeEach(() => {
  vi.clearAllMocks();
});

describe('Settings ClobFeedCard (v0.99)', () => {
  it('renders ClobFeedCard with "not_configured" state', async () => {
    mockGetAuditRetention.mockResolvedValue({
      retain_recent_ms: 90 * 86_400_000, max_rows: 50 * 1000, min_keep_rows: 1000,
    });
    mockClobFeedStatus.mockResolvedValue({
      state: 'not_configured', totalSnapshots: 0, marketsWithSnapshots: 0,
    });
    render(wrap());
    await waitFor(() => {
      expect(screen.getByTestId('clob-feed-card')).toBeInTheDocument();
    });
    await waitFor(() => {
      const stateEl = screen.getByTestId('clob-feed-state');
      expect(stateEl.textContent).toMatch(/not_configured|not configured/i);
    });
  });

  it('renders ClobFeedCard with "connected" state', async () => {
    mockGetAuditRetention.mockResolvedValue({
      retain_recent_ms: 90 * 86_400_000, max_rows: 50 * 1000, min_keep_rows: 1000,
    });
    mockClobFeedStatus.mockResolvedValue({
      state: 'connected', totalSnapshots: 150, marketsWithSnapshots: 30,
    });
    render(wrap());
    await waitFor(() => {
      expect(screen.getByTestId('clob-feed-state').textContent).toMatch(/connected/i);
    });
  });

  it('renders ClobFeedCard error state when IPC rejects', async () => {
    mockGetAuditRetention.mockResolvedValue({
      retain_recent_ms: 90 * 86_400_000, max_rows: 50 * 1000, min_keep_rows: 1000,
    });
    mockClobFeedStatus.mockRejectedValue(new Error('sidecar offline'));
    render(wrap());
    await waitFor(() => {
      expect(screen.getByTestId('clob-feed-error')).toBeInTheDocument();
    });
    expect(screen.getByTestId('clob-feed-error').textContent).toMatch(/offline/i);
  });
});