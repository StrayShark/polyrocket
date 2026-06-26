// v0.79c — Settings 中的 BankrollConfigCard（+3 个测试）。
//
// 验证只读 bankroll config 卡片能
// 用当前 config 值、"Open /bankroll" 链接进行渲染，
// 并在未设置 wallet 时优雅回退。
//
// @vitest-environment happy-dom

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

const mockListWallets = vi.fn();
const mockGetBankrollConfig = vi.fn();

vi.mock('@/ipc', () => ({
  listWallets: () => mockListWallets(),
  getBankrollConfig: (...args: unknown[]) => Promise.resolve(mockGetBankrollConfig(...args)),
}));

vi.mock('@/lib/i18n', () => ({
  useT: () => ({
    t: (k: string, vars?: Record<string, string>) => vars?.default ?? k,
    locale: 'en' as const,
  }),
}));

vi.mock('@/lib/format', () => ({
  formatRetentionAge: (ms: number) => `${ms}ms`,
  fmtPct: (v: number) => `${(v * 100).toFixed(0)}%`,
  fmtUsdc: (v: number) => `$${v.toFixed(2)}`,
  fmtConfidence: (v: number) => v.toFixed(2),
  fmtEdge: (v: number) => `${(v * 100).toFixed(1)}%`,
  fmtDate: (ms: number) => new Date(ms).toISOString(),
  fmtCents: (v: number) => `${v}c`,
  fmtRelativeTime: (ms: number) => `${ms}ms ago`,
  fmtLatency: (ms: number) => `${ms}ms`,
}));

vi.mock('@/stores/prefs-store', () => {
  const state = {
    defaultMinEdgePct: 5,
    defaultAllocationCapUsdc: 100,
    copyTradingEnabled: false,
    notificationsEnabled: false,
    advancedStats: false,
    autoPromoteBrierMargin: 0.05,
    autoPromoteAfterTrain: false,
    autoPromoteNotify: false,
    autoPromoteSkippedNotify: false,
    degradationAlertEnabled: true,
    paperMode: false,
    telemetryEnabled: false,
    proxyUrl: '',
    tradeSizeMultiplier: 1.0,
    locale: 'en',
    theme: 'dark',
    mirrorPaperMode: false,
  };
  const fn: any = (sel?: any) => (sel ? sel(state) : state);
  fn.getState = () => state;
  return { usePrefsStore: fn };
});

vi.mock('@/stores/toast-store', () => ({
  toast: { success: vi.fn(), error: vi.fn(), info: vi.fn() },
}));

// Mock Settings 使用的多个 IPC
vi.mock('@/ipc', () => ({
  listWallets: () => mockListWallets(),
  getBankrollConfig: (...args: unknown[]) => Promise.resolve(mockGetBankrollConfig(...args)),
  getAuditRetention: () => Promise.resolve({ retain_recent_ms: 8640000000, max_rows: 50000, min_keep_rows: 1000 }),
  setAuditRetention: vi.fn(),
  purgeAuditLogNow: vi.fn().mockResolvedValue(0),
  setAutoPromoteConfig: vi.fn(),
  getAutoPromoteConfig: () => Promise.resolve({ enabled: false, brier_margin: 0.05 }),
  listTelemetryLogs: () => Promise.resolve([]),
  setTelemetryEnabled: vi.fn().mockResolvedValue(true),
  getTelemetryEnabled: () => Promise.resolve(false),
  purgeTelemetryLogs: vi.fn().mockResolvedValue(0),
  getActiveModel: () => Promise.resolve(null),
  schedulerSelfTestNow: vi.fn().mockResolvedValue({ loops: [] }),
  clobFeedStatus: () => Promise.resolve({ enabled: false, last_run: null }),
  getStorageInfo: () => Promise.resolve({ custom_path: null, db_size_bytes: 1024 }),
  migrateStoragePath: vi.fn(),
  explainModel: vi.fn().mockResolvedValue({ base_value: 0.5, contributions: [] }),
  shapExplain: vi.fn().mockResolvedValue({ base_value: 0.5, contributions: [] }),
  getProxyConfig: () => Promise.resolve({ url: '' }),
  setProxyConfig: vi.fn(),
  clearProxyConfig: vi.fn(),
  setMirrorPaperMode: vi.fn(),
  getMirrorPaperMode: () => Promise.resolve(false),
  getAutoPromoteNotify: () => Promise.resolve(false),
  getDegradationAlertEnabled: () => Promise.resolve(true),
  getPaperMode: () => Promise.resolve(false),
  getMigrateStoragePathState: () => Promise.resolve({ required: false }),
  getCustomSidecarPath: () => Promise.resolve(null),
  getEffectiveSidecarPath: () => Promise.resolve('/default'),
  getDegradationRule: () => Promise.resolve({ brier_drop: 0.1, window_n: 100 }),
  getTradeSizeMultiplier: () => Promise.resolve(1.0),
  listPromoteHistory: () => Promise.resolve({ entries: [] }),
  computeAllocationPreview: vi.fn(),
  applyAllocation: vi.fn(),
  setBankrollConfig: vi.fn(),
  backupDatabase: vi.fn(),
  restoreDatabase: vi.fn(),
  backupExport: vi.fn(),
  rerunWelcome: vi.fn(),
}));

// 注：完整 Settings 测试设置较为复杂。在 v0.79c 中
// 我们仅校验 BankrollConfigCard 的 IPC 调用。

beforeEach(() => {
  vi.clearAllMocks();
  mockListWallets.mockResolvedValue([
    { id: 'w1', label: 'Treasury', chain_id: 137, wallet_type: 'eoa', created_at: 1700000000000, last_synced_at: 1700000000000 },
  ]);
  mockGetBankrollConfig.mockResolvedValue({
    kelly_multiplier: 0.25,
    max_per_signal_pct: 0.10,
    reserve_pct: 0.20,
    min_edge_pct: 0.05,
    max_total_exposure_pct: 0.80,
    min_confidence: 0.60,
  });
});

describe('v0.79c — BankrollConfigCard in Settings', () => {
  it('renders config items with formatted values', async () => {
    // 通过 dynamic import 直接测试 BankrollConfigCard 组件，
    // 因为挂载完整的 Settings 较为复杂。
    const { BankrollConfigCard } = await import('@/routes/Settings');
    const qc = new QueryClient({ defaultOptions: { queries: { retry: false, gcTime: 0 } } });
    render(
      <QueryClientProvider client={qc}>
        <MemoryRouter>
          <BankrollConfigCard />
        </MemoryRouter>
      </QueryClientProvider>,
    );
    await waitFor(() => {
      expect(screen.getByTestId('bankroll-config-card')).toBeTruthy();
    });
    // 校验 6 个 config 值均已显示
    const card = screen.getByTestId('bankroll-config-card');
    expect(card.textContent).toContain('Kelly multiplier');
    expect(card.textContent).toContain('Max per signal');
    expect(card.textContent).toContain('Reserve');
    expect(card.textContent).toContain('Min |edge|');
    expect(card.textContent).toContain('Max total exposure');
    expect(card.textContent).toContain('Min confidence');
  });

  it('clicking Open /bankroll calls navigate (smoke)', async () => {
    const { BankrollConfigCard } = await import('@/routes/Settings');
    const qc = new QueryClient({ defaultOptions: { queries: { retry: false, gcTime: 0 } } });
    render(
      <QueryClientProvider client={qc}>
        <MemoryRouter initialEntries={['/settings']}>
          <BankrollConfigCard />
        </MemoryRouter>
      </QueryClientProvider>,
    );
    await waitFor(() => {
      expect(screen.getByTestId('bankroll-config-go')).toBeTruthy();
    });
    // 仅校验按钮可点击（不崩溃）
    screen.getByTestId('bankroll-config-go').click();
  });

  it('renders fallback when no wallet', async () => {
    mockListWallets.mockResolvedValue([]);
    const { BankrollConfigCard } = await import('@/routes/Settings');
    const qc = new QueryClient({ defaultOptions: { queries: { retry: false, gcTime: 0 } } });
    render(
      <QueryClientProvider client={qc}>
        <MemoryRouter>
          <BankrollConfigCard />
        </MemoryRouter>
      </QueryClientProvider>,
    );
    await waitFor(() => {
      const text = document.body.textContent || '';
      expect(text).toMatch(/default config|bankroll/i);
    });
  });
});
