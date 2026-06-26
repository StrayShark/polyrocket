// v0.100 — Settings.tsx SchedulerSelfTestCard 测试（+3 个测试，+~5 个语句）。
//
// SchedulerSelfTestCard（Settings.tsx:1284）展示
// scheduler loop 健康状态（allHealthy true/false + 各 loop
// 状态）。已有 Settings 测试未覆盖本卡片。
//
// @vitest-environment happy-dom

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

const { mockGetAuditRetention, mockSchedulerSelfTestNow } = vi.hoisted(() => ({
  mockGetAuditRetention: vi.fn(),
  mockSchedulerSelfTestNow: vi.fn(),
}));

vi.mock('@/ipc', () => ({
  getAuditRetention: (...args: unknown[]) => mockGetAuditRetention(...args),
  schedulerSelfTestNow: (...args: unknown[]) => mockSchedulerSelfTestNow(...args),
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
  clobFeedStatus: vi.fn().mockResolvedValue({
    state: 'not_configured', totalSnapshots: 0, marketsWithSnapshots: 0,
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
  clobStatus: vi.fn().mockResolvedValue({ state: 'not_configured' }),
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

describe('Settings SchedulerSelfTestCard (v0.100)', () => {
  it('renders scheduler self-test card with all_healthy', async () => {
    mockGetAuditRetention.mockResolvedValue({
      retain_recent_ms: 90 * 86_400_000, max_rows: 50 * 1000, min_keep_rows: 1000,
    });
    mockSchedulerSelfTestNow.mockResolvedValue({
      processStartedAtUnix: 1700000000,
      checkedAtUnixMs: 1700000010000,
      allHealthy: true,
      loops: [
        { name: 'train-poll', lastTickUnixMs: 1700000009, ageMs: 1000, healthy: true },
        { name: 'mirror-poll', lastTickUnixMs: 1700000008, ageMs: 2000, healthy: true },
      ],
    });
    render(wrap());
    await waitFor(() => {
      expect(screen.getByTestId('scheduler-self-test')).toBeInTheDocument();
    });
    await waitFor(() => {
      expect(screen.getByTestId('scheduler-overall').textContent).toMatch(/all_healthy|healthy/i);
    });
    expect(screen.getByTestId('scheduler-loop-train-poll')).toBeInTheDocument();
    expect(screen.getByTestId('scheduler-loop-mirror-poll')).toBeInTheDocument();
  });

  it('renders scheduler self-test card with some_unhealthy', async () => {
    mockGetAuditRetention.mockResolvedValue({
      retain_recent_ms: 90 * 86_400_000, max_rows: 50 * 1000, min_keep_rows: 1000,
    });
    mockSchedulerSelfTestNow.mockResolvedValue({
      processStartedAtUnix: 1700000000,
      checkedAtUnixMs: 1700000010000,
      allHealthy: false,
      loops: [
        { name: 'train-poll', lastTickUnixMs: 1700000009, ageMs: 1000, healthy: true },
        { name: 'mirror-poll', lastTickUnixMs: null, ageMs: null, healthy: false },
      ],
    });
    render(wrap());
    await waitFor(() => {
      expect(screen.getByTestId('scheduler-overall').textContent).toMatch(/some_unhealthy|unhealthy/i);
    });
    // mirror-poll loop 仍应渲染（即便 unhealthy）
    expect(screen.getByTestId('scheduler-loop-mirror-poll')).toBeInTheDocument();
  });

  it('renders loading state when scheduler self-test is fetching', async () => {
    mockGetAuditRetention.mockResolvedValue({
      retain_recent_ms: 90 * 86_400_000, max_rows: 50 * 1000, min_keep_rows: 1000,
    });
    // 永不 resolve → loading 状态持续
    mockSchedulerSelfTestNow.mockReturnValue(new Promise(() => {}));
    render(wrap());
    await waitFor(() => {
      expect(screen.getByTestId('scheduler-self-test')).toBeInTheDocument();
    });
    // 因 snap 为 null，应显示 "common.loading" 文本
    expect(screen.getByTestId('scheduler-overall').textContent).toMatch(/loading/i);
  });
});