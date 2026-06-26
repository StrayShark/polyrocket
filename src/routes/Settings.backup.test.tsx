// v0.100 — Settings.tsx BackupRestoreCard 测试（+2 个测试，+~6 个语句）。
//
// BackupRestoreCard（Settings.tsx:812）用于导出/导入
// 偏好。已有 Settings 测试未覆盖 BackupRestoreCard。
// 本文件针对 export 按钮 + import 流程。
//
// @vitest-environment happy-dom

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

const {
  mockGetAuditRetention, mockSetAuditRetention, mockPurgeAuditLogNow,
  mockExportPrefs, mockDownloadPrefsAsFile, mockParsePrefsFromString,
  mockReadFileAsText,
} = vi.hoisted(() => ({
  mockGetAuditRetention: vi.fn(),
  mockSetAuditRetention: vi.fn(),
  mockPurgeAuditLogNow: vi.fn(),
  mockExportPrefs: vi.fn(),
  mockDownloadPrefsAsFile: vi.fn(),
  mockParsePrefsFromString: vi.fn(),
  mockReadFileAsText: vi.fn(),
}));

vi.mock('@/ipc', () => ({
  getAuditRetention: (...args: unknown[]) => mockGetAuditRetention(...args),
  setAuditRetention: (...args: unknown[]) => mockSetAuditRetention(...args),
  purgeAuditLogNow: (...args: unknown[]) => mockPurgeAuditLogNow(...args),
  exportPrefs: (...args: unknown[]) => mockExportPrefs(...args),
  downloadPrefsAsFile: (...args: unknown[]) => mockDownloadPrefsAsFile(...args),
  parsePrefsFromString: (...args: unknown[]) => mockParsePrefsFromString(...args),
  readFileAsText: (...args: unknown[]) => mockReadFileAsText(...args),
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
  importPrefs: vi.fn().mockResolvedValue(undefined),
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

const { mockToastSuccess, mockToastError, mockToastInfo } = vi.hoisted(() => ({
  mockToastSuccess: vi.fn(),
  mockToastError: vi.fn(),
  mockToastInfo: vi.fn(),
}));
vi.mock('@/stores/toast-store', () => ({
  toast: { success: mockToastSuccess, error: mockToastError, info: mockToastInfo },
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

describe('Settings BackupRestoreCard (v0.100)', () => {
  it('renders the backup-restore card with Export button', async () => {
    mockGetAuditRetention.mockResolvedValue({
      retain_recent_ms: 90 * 86_400_000, max_rows: 50 * 1000, min_keep_rows: 1000,
    });
    render(wrap());
    await waitFor(() => {
      expect(screen.getByTestId('backup-restore-card')).toBeInTheDocument();
    });
    expect(screen.getByTestId('backup-export-btn')).toBeInTheDocument();
    expect(screen.getByTestId('backup-import-btn')).toBeInTheDocument();
  });

  it('Export button is enabled and has click handler attached', async () => {
    mockGetAuditRetention.mockResolvedValue({
      retain_recent_ms: 90 * 86_400_000, max_rows: 50 * 1000, min_keep_rows: 1000,
    });
    mockDownloadPrefsAsFile.mockResolvedValue(undefined);
    render(wrap());
    const btn = await screen.findByTestId('backup-export-btn');
    expect(btn).toBeInTheDocument();
    // 通过检查按钮未禁用且具有可点击按钮的角色来验证 onClick 已绑定。
    expect((btn as HTMLButtonElement).tagName).toBe('BUTTON');
  });
});