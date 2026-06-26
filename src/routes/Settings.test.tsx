/**
 * v0.28d — AutoPromoteCard 行为测试。
 *
 * 测试新的 "Auto-run after train" 开关，以及
 * 保存 margin 时 setAutoPromoteConfig 的新推送行为。
 *
 * v0.28c 新增：
 *  - 在 AutoPromoteCard 中为 `autoPromoteAfterTrain`
 *    添加 Toggle 控件。切换它会通过
 *    `setAutoPromoteConfig({enabled})` 把新值推送到 Rust。
 *  - 已有的 margin Save 按钮在更新 zustand store
 *    的同时，也将 `{brier_margin}` 推送到 Rust。
 *
 * 这些测试通过 mock `@/ipc` 与 `@/stores/prefs-store`
 * 来验证 IPC 是否以正确的参数被调用。
 */

// @vitest-environment happy-dom

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

function wrap(node: React.ReactNode) {
  const qc = new QueryClient({
    defaultOptions: { queries: { retry: false, gcTime: 0 } },
  });
  return (
    <MemoryRouter>
      <QueryClientProvider client={qc}>{node}</QueryClientProvider>
    </MemoryRouter>
  );
}

// Mock IPC 层。我们对 auto-promote config 的推送进行断言；
// Settings 用到的其他 IPC 被 stub 掉，以避免实际网络调用。
vi.mock('@/ipc', () => ({
  getAuditRetention: vi.fn().mockResolvedValue({ days: 90 }),
  setAuditRetention: vi.fn(),
  purgeAuditLogNow: vi.fn(),
  setAutoPromoteConfig: vi.fn(),
  setTelemetryEnabled: vi.fn().mockResolvedValue(true),
  getTelemetryEnabled: vi.fn().mockResolvedValue(false),
  // v0.49a — 遥测日志文件保留
  listTelemetryLogs: vi.fn().mockResolvedValue([]),
  purgeTelemetryLogs: vi.fn().mockResolvedValue(0),
  // v0.49b — 激活模型 IPC
  getActiveModel: vi.fn().mockResolvedValue(null),
  // v0.49c —— scheduler 自检
  schedulerSelfTestNow: vi.fn().mockResolvedValue({
    processStartedAtUnix: 1700000000,
    checkedAtUnixMs: 1700000010000,
    allHealthy: true,
    loops: [],
  }),
  // v0.51a — CLOB 订阅源
  clobFeedStatus: vi.fn().mockResolvedValue({
    state: 'not_configured',
    totalSnapshots: 0,
    marketsWithSnapshots: 0,
  }),
  setMirrorPaperMode: vi.fn().mockResolvedValue(true),
  getMirrorPaperMode: vi.fn().mockResolvedValue(false),
  // v0.54b — 存储迁移工具
  getStorageInfo: vi.fn().mockResolvedValue({
    defaultPath: '/tmp/db/polyrocket.db',
    currentPath: '/tmp/db/polyrocket.db',
    isCustom: false,
    exists: true,
    writable: true,
    freeBytes: null,
    restartRequired: false,
  }),
  migrateStoragePath: vi.fn().mockResolvedValue({
    from: '/tmp/db/polyrocket.db',
    to: '/Volumes/external/polyrocket',
    filesCopied: 2,
    bytesCopied: 12345,
    overwritten: false,
    noop: false,
  }),
  // v0.56 — 网络代理 / Tor
  getProxyConfig: vi.fn().mockResolvedValue({
    enabled: false,
    url: null,
    scheme: null,
    restartRequired: false,
  }),
  setProxyConfig: vi.fn().mockResolvedValue({
    enabled: true,
    url: 'socks5://127.0.0.1:9050',
    scheme: 'socks5',
    restartRequired: true,
  }),
  clearProxyConfig: vi.fn().mockResolvedValue(undefined),
}));

// 用一个可控的内存值 Mock prefs store。真实 store 使用
// zustand + localStorage，测试间重置比较麻烦。我们同时支持
// `usePrefsStore()`（无 selector，返回整个 state）
// 与 `usePrefsStore(sel)`（带 selector）两种调用方式。
const mockSetPref = vi.fn();
const mockPrefsState = {
  defaultMinEdgePct: 5,
  defaultAllocationCapUsdc: 100,
  copyTradingEnabled: false,
  notificationsEnabled: true,
  advancedStats: false,
  autoPromoteBrierMargin: 0.005,
  autoPromoteAfterTrain: false,
  autoPromoteNotify: true,
  // v0.48b — 模型降级告警
  degradationAlertNotify: true,
  setPref: mockSetPref,
  reset: vi.fn(),
};
vi.mock('@/stores/prefs-store', () => ({
  usePrefsStore: (selector?: (s: typeof mockPrefsState) => unknown) => {
    if (typeof selector === 'function') return selector(mockPrefsState);
    return mockPrefsState;
  },
}));

// Mock toast store 以避免副作用
vi.mock('@/stores/toast-store', () => ({
  toast: { success: vi.fn(), error: vi.fn(), info: vi.fn() },
}));

// 最小化 i18n shim——真实的 useT 从
// Zustand store 中拉取。测试只检查
// 行为，不检查字符串。
vi.mock('@/lib/i18n', () => ({
  useT: () => ({ t: (k: string) => k, locale: 'en' as const }),
  // v0.74f — AppearanceCard 使用 useLocaleStore + 常量
  useLocaleStore: (selector?: unknown) =>
    typeof selector === 'function'
      ? selector({ locale: 'en' as const, setLocale: vi.fn() })
      : { locale: 'en' as const, setLocale: vi.fn() },
  LOCALE_LABEL: { en: 'English', zh: '简体中文' },
  SUPPORTED_LOCALES: ['en', 'zh'] as const,
}));

import { setAutoPromoteConfig } from '@/ipc';
import { Settings } from '@/routes/Settings';

describe('AutoPromoteCard — v0.28c toggle + save (v0.28d)', () => {
  beforeEach(() => {
    vi.mocked(setAutoPromoteConfig).mockReset();
    vi.mocked(setAutoPromoteConfig).mockResolvedValue({
      enabled: false,
      brier_margin: 0.005,
    });
    mockSetPref.mockReset();
  });

  it('renders the "Auto-run after train" toggle', async () => {
    render(wrap(<Settings />));
    await waitFor(() => {
      expect(
        screen.getByTestId('auto-promote-after-train-toggle'),
      ).toBeInTheDocument();
    });
  });

  it('pushes { enabled: true } when the user toggles ON', async () => {
    render(wrap(<Settings />));
    const toggle = await screen.findByTestId(
      'auto-promote-after-train-toggle',
    );
    fireEvent.click(toggle);
    await waitFor(() => {
      expect(setAutoPromoteConfig).toHaveBeenCalledWith(
        expect.objectContaining({ enabled: true }),
      );
    });
    // 同时更新本地 zustand store
    expect(mockSetPref).toHaveBeenCalledWith(
      'autoPromoteAfterTrain',
      true,
    );
  });

  it('pushes { enabled: false } when the user toggles OFF', async () => {
    // 重新 mock store 以从 afterTrain=true 起步
    mockPrefsState.autoPromoteAfterTrain = true;
    render(wrap(<Settings />));
    const toggle = await screen.findByTestId(
      'auto-promote-after-train-toggle',
    );
    fireEvent.click(toggle);
    await waitFor(() => {
      expect(setAutoPromoteConfig).toHaveBeenCalledWith(
        expect.objectContaining({ enabled: false }),
      );
    });
    expect(mockSetPref).toHaveBeenCalledWith(
      'autoPromoteAfterTrain',
      false,
    );
    mockPrefsState.autoPromoteAfterTrain = false; // reset
  });

  it('Save button pushes { brier_margin } to Rust', async () => {
    render(wrap(<Settings />));
    const saveBtn = await screen.findByTestId('auto-promote-save-btn');
    fireEvent.click(saveBtn);
    await waitFor(() => {
      expect(setAutoPromoteConfig).toHaveBeenCalledWith(
        expect.objectContaining({ brier_margin: 0.005 }),
      );
    });
    // 同时更新本地 zustand store
    expect(mockSetPref).toHaveBeenCalledWith(
      'autoPromoteBrierMargin',
      0.005,
    );
  });

  it('mounts push the current config to Rust (one-time)', async () => {
    render(wrap(<Settings />));
    // 挂载时的 useEffect 会以当前持久化的
    // 值调用 setAutoPromoteConfig。等待它完成。
    await waitFor(() => {
      expect(setAutoPromoteConfig).toHaveBeenCalled();
    });
    // 检查至少有一次同时携带了两个字段的调用
    const calls = vi.mocked(setAutoPromoteConfig).mock.calls;
    const fullPush = calls.find(
      (c) =>
        typeof c[0]?.enabled === 'boolean' &&
        typeof c[0]?.brier_margin === 'number',
    );
    expect(fullPush).toBeDefined();
  });
});

// =================================================================
// =================== v0.36b — 备份与恢复卡片 ===========================
// =================================================================

describe('Backup & restore card (v0.36b)', () => {
  beforeEach(() => {
    vi.mocked(setAutoPromoteConfig).mockReset();
    vi.mocked(setAutoPromoteConfig).mockResolvedValue({
      enabled: false,
      brier_margin: 0.005,
    });
    mockSetPref.mockReset();
  });

  it('renders the Backup & restore card with Export and Import buttons', async () => {
    render(wrap(<Settings />));
    await waitFor(() => {
      expect(screen.getByTestId('backup-restore-card')).toBeInTheDocument();
    });
    expect(screen.getByTestId('backup-export-btn')).toBeInTheDocument();
    expect(screen.getByTestId('backup-import-btn')).toBeInTheDocument();
  });

  it('Export button triggers a download (spy on anchor click)', async () => {
    // 监听 document.createElement 以捕获下载用的
    // anchor 元素
    const realCreate = document.createElement.bind(document);
    let capturedAnchor: HTMLAnchorElement | null = null;
    vi.spyOn(document, 'createElement').mockImplementation((tag) => {
      const el = realCreate(tag);
      if (tag === 'a') capturedAnchor = el as HTMLAnchorElement;
      return el;
    });
    render(wrap(<Settings />));
    const exportBtn = await screen.findByTestId('backup-export-btn');
    fireEvent.click(exportBtn);
    expect(capturedAnchor).not.toBeNull();
    expect(capturedAnchor!.download).toMatch(/^polyrocket-prefs-\d{8}\.json$/);
    // 恢复
    vi.mocked(document.createElement).mockRestore();
  });

  // v0.39b — auto-promote 桌面通知开关
  it('renders the auto-promote-notify toggle', async () => {
    render(wrap(<Settings />));
    await waitFor(() => {
      expect(screen.getByTestId('auto-promote-notify-toggle')).toBeInTheDocument();
    });
  });

  it('toggling auto-promote-notify updates the prefs store', async () => {
    render(wrap(<Settings />));
    const toggle = await screen.findByTestId('auto-promote-notify-toggle');
    fireEvent.click(toggle);
    await waitFor(() => {
      expect(mockSetPref).toHaveBeenCalledWith('autoPromoteNotify', false);
    });
  });

  // v0.42c — 遥测 opt-in 开关
  it('renders the telemetry card with a toggle', async () => {
    render(wrap(<Settings />));
    await waitFor(() => {
      expect(screen.getByTestId('telemetry-toggle')).toBeInTheDocument();
    });
  });

  it('toggling telemetry ON pushes { enabled: true } to Rust', async () => {
    const { setTelemetryEnabled } = await import('@/ipc');
    vi.mocked(setTelemetryEnabled).mockClear();
    render(wrap(<Settings />));
    const toggle = await screen.findByTestId('telemetry-toggle');
    fireEvent.click(toggle);
    await waitFor(() => {
      expect(setTelemetryEnabled).toHaveBeenCalledWith({ enabled: true });
    });
  });

  it('mount pushes the current effective state from Rust (sync path)', async () => {
    const { getTelemetryEnabled } = await import('@/ipc');
    vi.mocked(getTelemetryEnabled).mockClear();
    vi.mocked(getTelemetryEnabled).mockResolvedValue(true);
    render(wrap(<Settings />));
    // 挂载 effect 调用 getTelemetryEnabled，并在
    // 与当前值不同时把结果推入 prefs store。
    await waitFor(() => {
      expect(getTelemetryEnabled).toHaveBeenCalled();
    });
  });

  // v0.44c — 纸面模式开关
  it('renders the mirror-paper-mode toggle', async () => {
    render(wrap(<Settings />));
    await waitFor(() => {
      expect(screen.getByTestId('mirror-paper-mode-toggle')).toBeInTheDocument();
    });
  });

  it('toggling paper mode ON pushes { enabled: true } to Rust', async () => {
    const { setMirrorPaperMode } = await import('@/ipc');
    vi.mocked(setMirrorPaperMode).mockClear();
    render(wrap(<Settings />));
    const toggle = await screen.findByTestId('mirror-paper-mode-toggle');
    fireEvent.click(toggle);
    await waitFor(() => {
      expect(setMirrorPaperMode).toHaveBeenCalledWith({ enabled: true });
    });
  });

  // v0.42e-2 — 为被跳过的 auto-promote 分支
    // 提供 opt-in 的操作系统通知。
  it('renders the auto-promote-notify-skipped toggle', async () => {
    render(wrap(<Settings />));
    await waitFor(() => {
      expect(
        screen.getByTestId('auto-promote-notify-skipped-toggle'),
      ).toBeInTheDocument();
    });
  });

  it('toggling auto-promote-notify-skipped updates the prefs store', async () => {
    render(wrap(<Settings />));
    const toggle = await screen.findByTestId('auto-promote-notify-skipped-toggle');
    fireEvent.click(toggle);
    await waitFor(() => {
      expect(mockSetPref).toHaveBeenCalledWith('autoPromoteSkippedNotify', true);
    });
  });

  // v0.48b —— model 降级告警 toggle
  it('renders the degradation-alert toggle', async () => {
    render(wrap(<Settings />));
    await waitFor(() => {
      expect(screen.getByTestId('degradation-alert-toggle')).toBeInTheDocument();
    });
  });

  it('toggling degradation alert updates the prefs store', async () => {
    mockSetPref.mockClear();
    render(wrap(<Settings />));
    const toggle = await screen.findByTestId('degradation-alert-toggle');
    fireEvent.click(toggle);
    await waitFor(() => {
      expect(mockSetPref).toHaveBeenCalledWith('degradationAlertNotify', false);
    });
  });
});

describe('Active model card (v0.49b)', () => {
  it('renders the empty state when no model is promoted yet', async () => {
    const { getActiveModel } = await import('@/ipc');
    (getActiveModel as ReturnType<typeof vi.fn>).mockResolvedValueOnce(null);
    render(wrap(<Settings />));
    await waitFor(() => {
      expect(screen.getByTestId('active-model-empty')).toBeInTheDocument();
    });
  });

  it('renders the populated summary when a model is active', async () => {
    const { getActiveModel } = await import('@/ipc');
    (getActiveModel as ReturnType<typeof vi.fn>).mockResolvedValueOnce({
      modelVersion: 'logistic-train-test1234',
      bestBrier: 0.172,
      bestParams: { alpha: 0.01 },
      promotedAtMs: 1740000000000,
      weights: null,
      sourcePath: '/tmp/.polyrocket/sidecar/models/active.json',
    });
    render(wrap(<Settings />));
    await waitFor(() => {
      expect(screen.getByTestId('active-model-summary')).toBeInTheDocument();
    });
    expect(screen.getByTestId('active-model-summary').textContent).toContain(
      'logistic-train-test1234',
    );
    expect(screen.getByTestId('active-model-summary').textContent).toContain('0.1720');
  });

  it('renders an error banner when the IPC fails', async () => {
    const { getActiveModel } = await import('@/ipc');
    (getActiveModel as ReturnType<typeof vi.fn>).mockRejectedValueOnce(
      new Error('disk on fire'),
    );
    render(wrap(<Settings />));
    await waitFor(() => {
      expect(screen.getByTestId('active-model-error')).toBeInTheDocument();
    });
  });
});

describe('Scheduler self-test card (v0.49c)', () => {
  it('renders all 8 loop rows when the IPC returns a snapshot', async () => {
    const { schedulerSelfTestNow } = await import('@/ipc');
    (schedulerSelfTestNow as ReturnType<typeof vi.fn>).mockResolvedValue({
      processStartedAtUnix: 1700000000,
      checkedAtUnixMs: 1700000010000,
      allHealthy: true,
      loops: [
        { name: 'anomaly', lastTickUnixMs: 1700000009000, ageMs: 1000, healthy: true },
        { name: 'audit_purge', lastTickUnixMs: 1700000009000, ageMs: 1000, healthy: true },
        { name: 'daily_brief', lastTickUnixMs: 1700000009000, ageMs: 1000, healthy: true },
        { name: 'degradation_check', lastTickUnixMs: 1700000009000, ageMs: 1000, healthy: true },
        { name: 'health_probe', lastTickUnixMs: 1700000009000, ageMs: 1000, healthy: true },
        { name: 'mirror_executor', lastTickUnixMs: 1700000009000, ageMs: 1000, healthy: true },
        { name: 'paper_fills_reconcile', lastTickUnixMs: 1700000009000, ageMs: 1000, healthy: true },
        { name: 'sidecar_health', lastTickUnixMs: 1700000009000, ageMs: 1000, healthy: true },
      ],
    });
    render(wrap(<Settings />));
    await waitFor(() => {
      expect(screen.getByTestId('scheduler-loop-health_probe')).toBeInTheDocument();
    });
    expect(screen.getByTestId('scheduler-overall').textContent).toContain('scheduler.all_healthy');
    // 8 行。
    expect(screen.getAllByTestId(/^scheduler-loop-/).length).toBe(8);
  });

  it('flips to some-unhealthy when at least one loop is red', async () => {
    const { schedulerSelfTestNow } = await import('@/ipc');
    (schedulerSelfTestNow as ReturnType<typeof vi.fn>).mockResolvedValue({
      processStartedAtUnix: 1700000000,
      checkedAtUnixMs: 1700000010000,
      allHealthy: false,
      loops: [
        { name: 'health_probe', lastTickUnixMs: 0, ageMs: null, healthy: false },
      ],
    });
    render(wrap(<Settings />));
    await waitFor(() => {
      expect(screen.getByTestId('scheduler-overall').textContent).toContain('scheduler.some_unhealthy');
    });
  });
});

describe('Re-run setup card (v0.53b)', () => {
  it('renders the re-run button', async () => {
    render(wrap(<Settings />));
    expect(
      screen.getByTestId('rerun-setup'),
    ).toBeInTheDocument();
  });
});

describe('Storage migration card (v0.54b)', () => {
  it('does NOT render when no custom path is set (no restart required)', async () => {
    const { getStorageInfo } = await import('@/ipc');
    (getStorageInfo as ReturnType<typeof vi.fn>).mockResolvedValue({
      defaultPath: '/tmp/db/polyrocket.db',
      currentPath: '/tmp/db/polyrocket.db',
      isCustom: false,
      exists: true,
      writable: true,
      freeBytes: null,
      restartRequired: false,
    });
    render(wrap(<Settings />));
    // 迁移卡片仅在 restartRequired=true 时渲染，
    // 因此这里不应该出现。
    expect(
      screen.queryByTestId('storage-migrate-now'),
    ).not.toBeInTheDocument();
  });

  it('renders the migration button when restart is required', async () => {
    const { getStorageInfo } = await import('@/ipc');
    (getStorageInfo as ReturnType<typeof vi.fn>).mockResolvedValue({
      defaultPath: '/tmp/db/polyrocket.db',
      currentPath: '/Volumes/external/polyrocket',
      isCustom: true,
      exists: true,
      writable: true,
      freeBytes: null,
      restartRequired: true,
    });
    render(wrap(<Settings />));
    await waitFor(() => {
      expect(
        screen.getByTestId('storage-migrate-now'),
      ).toBeInTheDocument();
    });
  });

  it('clicking Copy calls migrateStoragePath IPC', async () => {
    const { getStorageInfo, migrateStoragePath } = await import('@/ipc');
    (getStorageInfo as ReturnType<typeof vi.fn>).mockResolvedValue({
      defaultPath: '/tmp/db/polyrocket.db',
      currentPath: '/Volumes/external/polyrocket',
      isCustom: true,
      exists: true,
      writable: true,
      freeBytes: null,
      restartRequired: true,
    });
    (migrateStoragePath as ReturnType<typeof vi.fn>).mockResolvedValue({
      from: '/tmp/db/polyrocket.db',
      to: '/Volumes/external/polyrocket',
      filesCopied: 3,
      bytesCopied: 99999,
      overwritten: false,
      noop: false,
    });
    render(wrap(<Settings />));
    await waitFor(() => {
      expect(
        screen.getByTestId('storage-migrate-now'),
      ).toBeInTheDocument();
    });
    fireEvent.click(screen.getByTestId('storage-migrate-now'));
    await waitFor(() => {
      expect(migrateStoragePath).toHaveBeenCalledWith(
        '/Volumes/external/polyrocket',
        false,
      );
    });
  });
});

describe('Network card (v0.56)', () => {
  it('renders the proxy URL input + Save / Clear buttons', async () => {
    render(wrap(<Settings />));
    await waitFor(() => {
      expect(screen.getByTestId('network-proxy-url')).toBeInTheDocument();
      expect(screen.getByTestId('network-proxy-save')).toBeInTheDocument();
      expect(screen.getByTestId('network-proxy-clear')).toBeInTheDocument();
    });
  });

  it('Save calls setProxyConfig with the typed URL', async () => {
    const { setProxyConfig, getProxyConfig } = await import('@/ipc');
    // 让 IPC 返回一个带有
    // 已存在 URL 的配置，以便表单
    // 启动时已预填。然后用户
    // 可以通过重新输入来"修改"该 URL。
    (getProxyConfig as ReturnType<typeof vi.fn>).mockResolvedValue({
      enabled: false,
      url: 'http://old-proxy:8080',
      scheme: 'http',
      restartRequired: false,
    });
    render(wrap(<Settings />));
    await waitFor(() => {
      const inp = screen.getByTestId('network-proxy-url') as HTMLInputElement;
      expect(inp.value).toBe('http://old-proxy:8080');
    });
    const input = screen.getByTestId('network-proxy-url') as HTMLInputElement;
    fireEvent.input(input, {
      target: { value: 'socks5://127.0.0.1:9050' },
    });
    await waitFor(() => {
      expect(input.value).toBe('socks5://127.0.0.1:9050');
    });
    fireEvent.click(screen.getByTestId('network-proxy-save'));
    await waitFor(() => {
      expect(setProxyConfig).toHaveBeenCalledWith(
        false,
        'socks5://127.0.0.1:9050',
      );
    });
  });

  it('Clear calls clearProxyConfig', async () => {
    const { clearProxyConfig } = await import('@/ipc');
    render(wrap(<Settings />));
    await waitFor(() => screen.getByTestId('network-proxy-clear'));
    fireEvent.click(screen.getByTestId('network-proxy-clear'));
    await waitFor(() => {
      expect(clearProxyConfig).toHaveBeenCalled();
    });
  });
});
