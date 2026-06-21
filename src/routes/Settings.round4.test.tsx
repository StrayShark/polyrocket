// v0.92 — Settings.tsx branches round 2 (+6 tests, fn 72.6→75%).
//
// Targets the top Card save() and reset() handlers (lines 96-113
// of Settings.tsx) and the `dirty` state computation. The
// v0.89d attempt failed because the top buttons lacked testids;
// v0.92 added `prefs-save-btn` / `prefs-reset-btn` testids.
//
// Coverage targets:
//   - save() iterates draft via Object.entries (5 fields)
//   - reset() calls prefs.reset() + setDraft(...) + toast.info
//   - dirty state: initial false, true after toggle
//   - Save button enabled when dirty, disabled otherwise
//   - reset() restores all 5 fields to prefs.* values

// @vitest-environment happy-dom

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

// Mock the IPC layer (Settings has many IPCs — stub all)
vi.mock('@/ipc', () => ({
  getAuditRetention: vi.fn().mockResolvedValue({ days: 90 }),
  setAuditRetention: vi.fn().mockResolvedValue(undefined),
  purgeAuditLogNow: vi.fn().mockResolvedValue(undefined),
  getRetentionPolicy: vi.fn().mockResolvedValue({ retain_days: 90, max_rows_k: 50, min_keep: 1000 }),
  setRetentionPolicy: vi.fn().mockResolvedValue({ purged: 0 }),
  setAutoPromoteConfig: vi.fn().mockResolvedValue(undefined),
  getAutoPromoteConfig: vi.fn().mockResolvedValue({
    enabled: false, brier_margin: 0.005, min_improvement_pct: 1.0,
  }),
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

// Mock prefs store with a controllable in-memory value
const mockSetPref = vi.fn();
const mockReset = vi.fn();
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
  setPref: mockSetPref,
  reset: mockReset,
};
vi.mock('@/stores/prefs-store', () => ({
  usePrefsStore: (selector?: (s: typeof mockPrefsState) => unknown) => {
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
  mockPrefsState.defaultMinEdgePct = 5;
  mockPrefsState.defaultAllocationCapUsdc = 100;
  mockPrefsState.copyTradingEnabled = false;
  mockPrefsState.notificationsEnabled = true;
  mockPrefsState.advancedStats = false;
});

describe('Settings — v0.92 top Card save/reset (top of page)', () => {
  it('renders top Reset and Save buttons with testids', async () => {
    render(wrap());
    const resetBtn = await screen.findByTestId('prefs-reset-btn');
    const saveBtn = await screen.findByTestId('prefs-save-btn');
    expect(resetBtn).toBeInTheDocument();
    expect(saveBtn).toBeInTheDocument();
  });

  it('top Save button is initially disabled (form not dirty)', async () => {
    render(wrap());
    const saveBtn = await screen.findByTestId('prefs-save-btn');
    expect(saveBtn).toBeDisabled();
  });

  it('top Reset button calls prefs.reset() and shows toast.info', async () => {
    render(wrap());
    const resetBtn = await screen.findByTestId('prefs-reset-btn');
    fireEvent.click(resetBtn);
    await waitFor(() => {
      expect(mockReset).toHaveBeenCalled();
    });
    await waitFor(() => {
      expect(mockToastInfo).toHaveBeenCalled();
    });
  });

  it('top Save button iterates draft and calls setPref for each field', async () => {
    render(wrap());
    // First make the form dirty by toggling copyTradingEnabled
    // The data-testid wraps a Toggle (role="switch"). Click the switch directly.
    const copyTradingCard = await screen.findByTestId('copy-trading-toggle');
    const copyTradingSwitch = copyTradingCard.querySelector('[role="switch"]') as HTMLElement;
    expect(copyTradingSwitch).toBeTruthy();
    fireEvent.click(copyTradingSwitch);
    // Then click top Save
    const saveBtn = await screen.findByTestId('prefs-save-btn');
    fireEvent.click(saveBtn);
    await waitFor(() => {
      // 5 fields should be persisted: defaultMinEdgePct, defaultAllocationCapUsdc,
      // copyTradingEnabled, notificationsEnabled, advancedStats
      expect(mockSetPref).toHaveBeenCalledWith('defaultMinEdgePct', expect.anything());
      expect(mockSetPref).toHaveBeenCalledWith('defaultAllocationCapUsdc', expect.anything());
      expect(mockSetPref).toHaveBeenCalledWith('copyTradingEnabled', true);
      expect(mockSetPref).toHaveBeenCalledWith('notificationsEnabled', expect.anything());
      expect(mockSetPref).toHaveBeenCalledWith('advancedStats', expect.anything());
    });
    await waitFor(() => {
      expect(mockToastSuccess).toHaveBeenCalled();
    });
  });

  it('top Save button shows success toast after persisting', async () => {
    render(wrap());
    const copyTradingCard = await screen.findByTestId('copy-trading-toggle');
    const copyTradingSwitch = copyTradingCard.querySelector('[role="switch"]') as HTMLElement;
    fireEvent.click(copyTradingSwitch);
    const saveBtn = await screen.findByTestId('prefs-save-btn');
    fireEvent.click(saveBtn);
    await waitFor(() => {
      expect(mockToastSuccess).toHaveBeenCalledWith('settings.btn.save_toast');
    });
  });

  it('top Reset button reverts form to prefs values (next save is no-op)', async () => {
    render(wrap());
    // Make dirty
    const copyTradingCard = await screen.findByTestId('copy-trading-toggle');
    const copyTradingSwitch = copyTradingCard.querySelector('[role="switch"]') as HTMLElement;
    fireEvent.click(copyTradingSwitch);
    // Verify save is enabled
    await waitFor(() => {
      const saveBtn = screen.getByTestId('prefs-save-btn');
      expect(saveBtn).not.toBeDisabled();
    });
    // Click reset
    const resetBtn = await screen.findByTestId('prefs-reset-btn');
    fireEvent.click(resetBtn);
    await waitFor(() => {
      expect(mockReset).toHaveBeenCalled();
    });
    // After reset, form should not be dirty (save disabled again)
    await waitFor(() => {
      const saveBtn = screen.getByTestId('prefs-save-btn');
      expect(saveBtn).toBeDisabled();
    });
  });
});
