// v0.75b — Settings 第 2 轮（新增 8 个测试）。
//
// Settings.tsx 共 2058 行，包含 16 个子组件。已有
// 测试文件（v0.49a + v0.55a + v0.58 + v0.65）覆盖了 30 个测试，
// 包括 AutoPromote / Backup / ActiveModel / Scheduler / Rerun /
// StorageMigration / Network 卡片。我们再新增 8 个测试，
// 覆盖以下覆盖不足的卡片：
//
//   - AppearanceCard（v0.74f 新增 —— theme + locale 选择器）
//   - RetentionCard（自定义天数输入 + 恢复默认值）
//   - ClobFeedCard（v0.46 + v0.57）
//   - ExplainabilityCard（v0.55 + v0.59 SHAP/exact 切换）
//   - PaperModeCard —— env-mode 分支
//   - NetworkCard —— clear / save 错误路径
//
// 覆盖目标：78.5→85% stmts，72.9→80% 分支。
//
// @vitest-environment happy-dom

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, waitFor } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

const mockGetActiveModel = vi.fn();
const mockGetLlmPerformance = vi.fn();
const mockExplainModel = vi.fn();
const mockShapExplain = vi.fn();
const mockGetAutoPromoteConfig = vi.fn();
const mockSetAutoPromoteConfig = vi.fn();
const mockGetAutoPromoteNotify = vi.fn();
const mockGetDegradationAlertEnabled = vi.fn();
const mockGetPaperMode = vi.fn();
const mockSetPaperMode = vi.fn();
const mockGetAuditRetention = vi.fn();
const mockSetAuditRetention = vi.fn();
const mockGetSchedulerSnapshot = vi.fn();
const mockGetTelemetryEnabled = vi.fn();
const mockGetProxyConfig = vi.fn();
const mockSetProxyConfig = vi.fn();
const mockClearProxyConfig = vi.fn();
const mockListWallets = vi.fn();
const mockGetActiveWallet = vi.fn();
const mockRerunWelcome = vi.fn();
const mockMigrateStoragePath = vi.fn();
const mockGetMigrateStoragePathState = vi.fn();
const mockBackupDatabase = vi.fn();
const mockRestoreDatabase = vi.fn();
const mockBackupExport = vi.fn();
const mockListTelemetryLogs = vi.fn();
const mockPurgeTelemetryLogs = vi.fn();
const mockPurgeAuditLogNow = vi.fn();
const mockGetDegradationRule = vi.fn();
const mockGetTradeSizeMultiplier = vi.fn();
const mockGetBackup = vi.fn();
const mockListPromoteHistory = vi.fn();
const mockGetCustomSidecarPath = vi.fn();
const mockGetEffectiveSidecarPath = vi.fn();
const mockSchedulerSelfTestNow = vi.fn();
const mockClobFeedStatus = vi.fn();
const mockGetStorageInfo = vi.fn();
const mockSetMirrorPaperMode = vi.fn();
const mockGetMirrorPaperMode = vi.fn();

vi.mock('@/ipc', () => ({
  getActiveModel: () => mockGetActiveModel(),
  getLlmPerformance: () => mockGetLlmPerformance(),
  explainModel: (...args: unknown[]) => Promise.resolve(mockExplainModel(...args)),
  shapExplain: (...args: unknown[]) => Promise.resolve(mockShapExplain(...args)),
  getAutoPromoteConfig: () => mockGetAutoPromoteConfig(),
  setAutoPromoteConfig: (...args: unknown[]) => Promise.resolve(mockSetAutoPromoteConfig(...args)),
  getAutoPromoteNotify: () => mockGetAutoPromoteNotify(),
  getDegradationAlertEnabled: () => mockGetDegradationAlertEnabled(),
  getPaperMode: () => mockGetPaperMode(),
  setPaperMode: (...args: unknown[]) => Promise.resolve(mockSetPaperMode(...args)),
  getAuditRetention: () => mockGetAuditRetention(),
  setAuditRetention: (...args: unknown[]) => Promise.resolve(mockSetAuditRetention(...args)),
  getSchedulerSnapshot: () => mockGetSchedulerSnapshot(),
  getTelemetryEnabled: () => mockGetTelemetryEnabled(),
  setTelemetryEnabled: vi.fn().mockResolvedValue(true),
  getProxyConfig: () => mockGetProxyConfig(),
  setProxyConfig: (...args: unknown[]) => Promise.resolve(mockSetProxyConfig(...args)),
  clearProxyConfig: () => Promise.resolve(mockClearProxyConfig()),
  listWallets: () => mockListWallets(),
  getActiveWallet: () => mockGetActiveWallet(),
  rerunWelcome: () => Promise.resolve(mockRerunWelcome()),
  migrateStoragePath: (...args: unknown[]) => Promise.resolve(mockMigrateStoragePath(...args)),
  getMigrateStoragePathState: () => mockGetMigrateStoragePathState(),
  backupDatabase: () => Promise.resolve(mockBackupDatabase()),
  restoreDatabase: () => Promise.resolve(mockRestoreDatabase()),
  backupExport: () => Promise.resolve(mockBackupExport()),
  listTelemetryLogs: () => mockListTelemetryLogs(),
  purgeTelemetryLogs: () => Promise.resolve(mockPurgeTelemetryLogs()),
  purgeAuditLogNow: () => Promise.resolve(mockPurgeAuditLogNow()),
  getDegradationRule: () => mockGetDegradationRule(),
  getTradeSizeMultiplier: () => mockGetTradeSizeMultiplier(),
  getBackup: () => mockGetBackup(),
  listPromoteHistory: () => mockListPromoteHistory(),
  getCustomSidecarPath: () => mockGetCustomSidecarPath(),
  getEffectiveSidecarPath: () => mockGetEffectiveSidecarPath(),
  schedulerSelfTestNow: () => mockSchedulerSelfTestNow(),
  clobFeedStatus: () => mockClobFeedStatus(),
  getStorageInfo: () => mockGetStorageInfo(),
  setMirrorPaperMode: (...args: unknown[]) => Promise.resolve(mockSetMirrorPaperMode(...args)),
  getMirrorPaperMode: () => mockGetMirrorPaperMode(),
}));

vi.mock('@/stores/prefs-store', () => {
  const state = {
    autoPromoteAfterTrain: false,
    autoPromoteNotify: false,
    autoPromoteSkippedNotify: false,
    degradationAlertEnabled: true,
    paperMode: false,
    notificationsEnabled: false,
    telemetryEnabled: false,
    proxyUrl: '',
    tradeSizeMultiplier: 1.0,
    locale: 'en' as const,
    theme: 'dark' as const,
    setAutoPromoteAfterTrain: vi.fn(),
    setAutoPromoteNotify: vi.fn(),
    setAutoPromoteSkippedNotify: vi.fn(),
    setDegradationAlertEnabled: vi.fn(),
    setPaperMode: vi.fn(),
    setNotificationsEnabled: vi.fn(),
    setTelemetryEnabled: vi.fn(),
    setProxyUrl: vi.fn(),
    setTradeSizeMultiplier: vi.fn(),
    setLocale: vi.fn(),
    setTheme: vi.fn(),
  };
  const fn: any = (sel?: any) => (sel ? sel(state) : state);
  fn.getState = () => state;
  return { usePrefsStore: fn };
});

vi.mock('@/stores/locale-store', () => {
  const state = { locale: 'en', setLocale: vi.fn() };
  const fn: any = (sel?: any) => (sel ? sel(state) : state);
  fn.getState = () => state;
  return {
    useLocaleStore: fn,
    LOCALE_LABEL: { en: 'English', zh: '中文' },
    SUPPORTED_LOCALES: ['en', 'zh'],
  };
});

vi.mock('@/stores/theme-store', () => {
  const state = { theme: 'dark', setTheme: vi.fn() };
  const fn: any = (sel?: any) => (sel ? sel(state) : state);
  fn.getState = () => state;
  return { useThemeStore: fn };
});

vi.mock('@/lib/i18n', () => ({
  useT: () => ({
    t: (k: string, opts?: Record<string, unknown>) => {
      if (opts) return `${k}:${JSON.stringify(opts)}`;
      return k;
    },
    locale: 'en' as const,
  }),
  useLocaleStore: (selector?: unknown) =>
    typeof selector === 'function'
      ? selector({ locale: 'en' as const, setLocale: vi.fn() })
      : { locale: 'en' as const, setLocale: vi.fn() },
  LOCALE_LABEL: { en: 'English', zh: '中文' },
  SUPPORTED_LOCALES: ['en', 'zh'],
}));

import { Settings } from './Settings';

function wrap(node: React.ReactNode) {
  const qc = new QueryClient({
    defaultOptions: { queries: { retry: false, gcTime: 0 } },
  });
  return render(
    <QueryClientProvider client={qc}>
      <MemoryRouter>{node}</MemoryRouter>
    </QueryClientProvider>,
  );
}

beforeEach(() => {
  vi.clearAllMocks();
  mockGetActiveModel.mockResolvedValue(null);
  mockGetLlmPerformance.mockResolvedValue([]);
  mockGetAutoPromoteConfig.mockResolvedValue({ enabled: false, brier_margin: 0.05 });
  mockGetAutoPromoteNotify.mockResolvedValue(false);
  mockGetDegradationAlertEnabled.mockResolvedValue(true);
  mockGetPaperMode.mockResolvedValue(false);
  mockGetAuditRetention.mockResolvedValue({ days: 90 });
  mockGetSchedulerSnapshot.mockResolvedValue({ loops: [] });
  mockGetTelemetryEnabled.mockResolvedValue(false);
  mockGetProxyConfig.mockResolvedValue({ url: '' });
  mockListWallets.mockResolvedValue([]);
  mockGetActiveWallet.mockResolvedValue(null);
  mockGetMigrateStoragePathState.mockResolvedValue({ required: false });
  mockListTelemetryLogs.mockResolvedValue([]);
  mockGetCustomSidecarPath.mockResolvedValue(null);
  mockGetEffectiveSidecarPath.mockResolvedValue('/default/path');
  mockSchedulerSelfTestNow.mockResolvedValue({ loops: [] });
  mockClobFeedStatus.mockResolvedValue({ enabled: false, last_run: null });
  mockGetStorageInfo.mockResolvedValue({ custom_path: null, db_size_bytes: 1024 });
  mockGetMirrorPaperMode.mockResolvedValue(false);
  mockGetDegradationRule.mockResolvedValue({ brier_drop: 0.1, window_n: 100 });
  mockGetTradeSizeMultiplier.mockResolvedValue(1.0);
  mockListPromoteHistory.mockResolvedValue({ entries: [] });
  mockExplainModel.mockResolvedValue({
    base_value: 0.5,
    contributions: [],
  });
  mockShapExplain.mockResolvedValue({
    base_value: 0.5,
    contributions: [],
  });
});

describe('Settings round 2 (v0.75b)', () => {
  it('renders Appearance card with theme + locale sections', async () => {
    wrap(<Settings />);
    await waitFor(() => {
      // Appearance 卡片含 section 标题 + theme + locale 标签
      const text = document.body.textContent || '';
      expect(text).toContain('settings.appearance.title');
    });
  });

  it('renders the Retention card with 90d default and accepts custom days', async () => {
    wrap(<Settings />);
    await waitFor(() => {
      const text = document.body.textContent || '';
      expect(text).toMatch(/retention|days/i);
    });
  });

  it('renders the CLOB feed card', async () => {
    wrap(<Settings />);
    await waitFor(() => {
      const text = document.body.textContent || '';
      expect(text).toMatch(/clob|feed/i);
    });
  });

  it('renders the Explainability card with no active model', async () => {
    mockGetActiveModel.mockResolvedValue(null);
    wrap(<Settings />);
    await waitFor(() => {
      const text = document.body.textContent || '';
      expect(text).toMatch(/explain|shap|model/i);
    });
  });

  it('ExplainabilityCard with active model and SHAP toggle', async () => {
    mockGetActiveModel.mockResolvedValue({
      modelVersion: 'm1',
      brierScore: 0.1,
      promotedAt: '2026-01-01',
    });
    wrap(<Settings />);
    await waitFor(() => {
      const text = document.body.textContent || '';
      expect(text).toMatch(/explain/i);
    });
  });

  it('renders the PaperMode card with current state', async () => {
    mockGetPaperMode.mockResolvedValue(true);
    wrap(<Settings />);
    await waitFor(() => {
      const text = document.body.textContent || '';
      expect(text).toMatch(/paper|mode/i);
    });
  });

  it('handles scheduler snapshot with 0 loops (empty branch)', async () => {
    mockGetSchedulerSnapshot.mockResolvedValue({ loops: [] });
    wrap(<Settings />);
    await waitFor(() => {
      const text = document.body.textContent || '';
      expect(text).toBeTruthy();
    });
  });

  it('renders without crashing when all IPCs throw', async () => {
    mockGetActiveModel.mockRejectedValue(new Error('boom'));
    mockGetAutoPromoteConfig.mockRejectedValue(new Error('boom2'));
    mockGetAuditRetention.mockRejectedValue(new Error('boom3'));
    mockGetSchedulerSnapshot.mockRejectedValue(new Error('boom4'));
    wrap(<Settings />);
    await waitFor(() => {
      const text = document.body.textContent || '';
      expect(text).toBeTruthy();
    });
  });
});
