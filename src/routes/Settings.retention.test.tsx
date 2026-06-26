// v0.99 — Settings.tsx RetentionCard 测试（+4 个测试，+~10 个语句）。
//
// RetentionCard（Settings.tsx:439）是一个用于
// audit 保留策略的自包含卡片。它包含 3 个 NumberHintField
// 输入（days、max rows、min keep）、一个 Save 按钮，
// 以及一个 Purge now 按钮。所有交互均使用前几轮
// 新增的 `retention-*` testid。
//
// @vitest-environment happy-dom

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

const { mockGetAuditRetention, mockSetAuditRetention, mockPurgeAuditLogNow } = vi.hoisted(() => ({
  mockGetAuditRetention: vi.fn(),
  mockSetAuditRetention: vi.fn(),
  mockPurgeAuditLogNow: vi.fn(),
}));

vi.mock('@/ipc', () => ({
  getAuditRetention: (...args: unknown[]) => mockGetAuditRetention(...args),
  setAuditRetention: (...args: unknown[]) => mockSetAuditRetention(...args),
  purgeAuditLogNow: (...args: unknown[]) => mockPurgeAuditLogNow(...args),
  // Settings 还会用到其他 IPC —— 一并 stub 掉
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
  mockPrefsState.copyTradingEnabled = false;
});

describe('Settings RetentionCard (v0.99)', () => {
  it('renders retention card with default values when query resolves', async () => {
    mockGetAuditRetention.mockResolvedValue({
      retain_recent_ms: 90 * 86_400_000,
      max_rows: 50 * 1000,
      min_keep_rows: 1000,
    });
    render(wrap());
    await waitFor(() => {
      expect(screen.getByTestId('audit-retention-card')).toBeInTheDocument();
    });
    // 3 个 NumberHintField 输入以默认值渲染
    expect(screen.getByDisplayValue('90')).toBeInTheDocument();
    expect(screen.getByDisplayValue('50')).toBeInTheDocument();
    expect(screen.getByDisplayValue('1000')).toBeInTheDocument();
  });

  it('Save & purge now button calls setAuditRetention with correct values', async () => {
    mockGetAuditRetention.mockResolvedValue({
      retain_recent_ms: 90 * 86_400_000, max_rows: 50 * 1000, min_keep_rows: 1000,
    });
    mockSetAuditRetention.mockResolvedValue(0);
    render(wrap());
    await waitFor(() => {
      expect(screen.getByTestId('retention-save')).toBeInTheDocument();
    });
    fireEvent.click(screen.getByTestId('retention-save'));
    await waitFor(() => {
      expect(mockSetAuditRetention).toHaveBeenCalled();
    });
    // 应使用 retain_recent_ms = 90 天 * 86_400_000 调用
    const call = mockSetAuditRetention.mock.calls[0][0];
    expect(call.retain_recent_ms).toBe(90 * 86_400_000);
    expect(call.max_rows).toBe(50 * 1000);
    expect(call.min_keep_rows).toBe(1000);
  });

  it('Purge now button calls purgeAuditLogNow', async () => {
    mockGetAuditRetention.mockResolvedValue({
      retain_recent_ms: 90 * 86_400_000, max_rows: 50 * 1000, min_keep_rows: 1000,
    });
    mockPurgeAuditLogNow.mockResolvedValue(5);
    render(wrap());
    await waitFor(() => {
      expect(screen.getByTestId('retention-purge-now')).toBeInTheDocument();
    });
    fireEvent.click(screen.getByTestId('retention-purge-now'));
    await waitFor(() => {
      expect(mockPurgeAuditLogNow).toHaveBeenCalled();
    });
  });

  it('Save button shows success toast with row count when setAuditRetention returns 1', async () => {
    mockGetAuditRetention.mockResolvedValue({
      retain_recent_ms: 90 * 86_400_000, max_rows: 50 * 1000, min_keep_rows: 1000,
    });
    mockSetAuditRetention.mockResolvedValue(1);
    render(wrap());
    await waitFor(() => {
      expect(screen.getByTestId('retention-save')).toBeInTheDocument();
    });
    fireEvent.click(screen.getByTestId('retention-save'));
    await waitFor(() => {
      expect(mockToastSuccess).toHaveBeenCalledWith(
        expect.stringContaining('1 row'),
      );
    });
  });
});