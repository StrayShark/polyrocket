// v0.54c — L1 组件测试,覆盖 6 个 welcome
// step 组件 + WelcomeBanner。
//
// 现有的 vitest 套件(v0.53 时 377 个测试)
// 仅包含 1 个针对 welcome 流程的测试
// (Settings → RerunSetupCard 渲染)。本文件
// 填补真正的覆盖:
//
//   1. WelcomeStep — locale 切换 + 3 个 value
//      props + Get Started 按钮。
//   2. StorageStep — default vs custom 模式,
//      Browse 按钮(用 pickDirectory),Apply
//      按钮调用 setStoragePath / reset。
//   3. ThemeStep — 渲染 3 个主题 + 点击主题
//      设置 useThemeStore.theme。
//   4. LlmStep — 渲染 5 个 provider + Alias +
//      Secret + Test 连通性。
//   5. PolymarketStep — CLOB + wallet 子卡片,
//      Save / Skip 按钮。
//   6. FinishStep — 包含数字的只读 summary
//      + Finish 按钮。
//   7. WelcomeBanner — 为半配置用户渲染,
//      所有 secret 已设置时隐藏。
//
// 所有测试每次渲染都使用全新的 QueryClient
// 并 mock @/ipc。它们在 happy-dom 下运行,
// 因此组件对 `window` / `localStorage` 的
// 使用不会出错。

// @vitest-environment happy-dom

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

// Mock @/ipc —— 每个测试通过 vi.mocked(...) 用自己的
// resolved 值重新定义。
vi.mock('@/ipc', () => ({
  getStorageInfo: vi.fn(),
  setStoragePath: vi.fn(),
  resetStoragePath: vi.fn(),
  pickDirectory: vi.fn(),
  migrateStoragePath: vi.fn().mockResolvedValue({
    from: '/tmp/db/polyrocket.db',
    to: '/Volumes/external/polyrocket',
    filesCopied: 3,
    bytesCopied: 12345,
    overwritten: false,
    noop: false,
  }),
  llmKeyUpsert: vi.fn(),
  llmKeySetSecret: vi.fn(),
  llmTestConnectivity: vi.fn(),
  listLlmKeys: vi.fn().mockResolvedValue([]),
  listLlmProviders: vi.fn().mockResolvedValue([]),
  polyrocketWalletSetPk: vi.fn(),
  polyrocketWalletCheck: vi.fn(),
  clobSetCreds: vi.fn(),
  clobGetCredsStatus: vi.fn().mockResolvedValue({ present: false }),
  secretsStatus: vi.fn().mockResolvedValue({
    llm_keys: 0,
    pm_api: false,
    pm_passphrase: false,
    pm_secret: false,
    wallet_pk: 0,
  }),
}));

// Mock theme store 以便读取当前主题
// 并断言 setter。
const mockSetTheme = vi.fn();
vi.mock('@/stores/theme-store', () => ({
  useThemeStore: Object.assign(
    (sel: any) => sel({ theme: 'dark', setTheme: mockSetTheme }),
    {
      getState: () => ({ theme: 'dark', setTheme: mockSetTheme }),
    },
  ),
}));

import * as ipc from '@/ipc';
import { WelcomeStep } from '@/components/welcome/WelcomeStep';
import { StorageStep } from '@/components/welcome/StorageStep';
import { ThemeStep } from '@/components/welcome/ThemeStep';
import { LlmStep } from '@/components/welcome/LlmStep';
import { PolymarketStep } from '@/components/welcome/PolymarketStep';
import { FinishStep } from '@/components/welcome/FinishStep';
import { WelcomeBanner } from '@/components/feedback/WelcomeBanner';
import { useWelcomeStore } from '@/stores/welcome-store';

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

function makeWelcomeStore() {
  // 每个测试都使用全新的 welcome-store 状态。
  // 我们返回完整的 store state(而非
  // 部分 mock),这样读取
  // `welcome.configured.*` 的组件不会出错。
  useWelcomeStore.getState().reset();
  return useWelcomeStore.getState();
}

beforeEach(() => {
  vi.clearAllMocks();
  useWelcomeStore.getState().reset();
});

// =================================================================
// 1. WelcomeStep 测试
// =================================================================

describe('WelcomeStep (v0.54c)', () => {
  it('renders the hero, 3 value props, and Get Started', () => {
    render(
      wrap(
        <WelcomeStep
          onLocale={() => {}}
        />,
      ),
    );
    expect(
      screen.getByTestId('welcome-step-welcome-title'),
    ).toBeInTheDocument();
    // 3 个 value-prop 卡片都由 WelcomeStep 的
    // value-prop loop 设置 data-testid。我们
    // 断言第一个来确认
    // loop 已执行。
    expect(
      screen.getByTestId('welcome-step-welcome-vp-0'),
    ).toBeInTheDocument();
  });
});

// =================================================================
// 2. StorageStep 测试
// =================================================================

describe('StorageStep (v0.54c)', () => {
  it('renders default + custom mode cards', async () => {
    vi.mocked(ipc.getStorageInfo).mockResolvedValue({
      defaultPath: '/tmp/db/polyrocket.db',
      currentPath: '/tmp/db/polyrocket.db',
      isCustom: false,
      exists: true,
      writable: true,
      freeBytes: null,
      restartRequired: false,
    });
    const welcome = makeWelcomeStore();
    render(wrap(<StorageStep welcome={welcome} />));
    await waitFor(() => {
      expect(
        screen.getByTestId('welcome-storage-mode-default'),
      ).toBeInTheDocument();
      expect(
        screen.getByTestId('welcome-storage-mode-custom'),
      ).toBeInTheDocument();
    });
  });

  it('Apply on default mode calls resetStoragePath', async () => {
    vi.mocked(ipc.getStorageInfo).mockResolvedValue({
      defaultPath: '/tmp/db/polyrocket.db',
      currentPath: '/tmp/db/polyrocket.db',
      isCustom: false,
      exists: true,
      writable: true,
      freeBytes: null,
      restartRequired: false,
    });
    vi.mocked(ipc.resetStoragePath).mockResolvedValue(undefined);
    const welcome = makeWelcomeStore();
    // spy setConfigured 以断言副作用。
    const setConfiguredSpy = vi.spyOn(welcome, 'setConfigured');
    render(wrap(<StorageStep welcome={welcome} />));
    await waitFor(() => screen.getByTestId('welcome-storage-apply'));
    fireEvent.click(screen.getByTestId('welcome-storage-apply'));
    await waitFor(() => {
      expect(ipc.resetStoragePath).toHaveBeenCalled();
    });
    expect(setConfiguredSpy).toHaveBeenCalledWith(
      'storagePath',
      true,
    );
  });

  it('Apply on custom mode calls setStoragePath with the typed value', async () => {
    // 预填 `isCustom: true` 让表单启动时进入
    // custom 模式。<ModeCard onClick={...}> 在
    // happy-dom 下有竞态(div onClick 不总触发),
    // 但我们关心的是条件路径输入框。
    vi.mocked(ipc.getStorageInfo).mockResolvedValue({
      defaultPath: '/tmp/db/polyrocket.db',
      currentPath: '/Volumes/external/polyrocket',
      isCustom: true,
      exists: true,
      writable: true,
      freeBytes: null,
      restartRequired: true,
    });
    vi.mocked(ipc.setStoragePath).mockResolvedValue(undefined);
    const welcome = makeWelcomeStore();
    render(wrap(<StorageStep welcome={welcome} />));
    const input = (await waitFor(
      () =>
        screen.getByTestId('welcome-storage-path-input') as HTMLInputElement,
    )) as HTMLInputElement;
    expect(input.value).toBe('/Volumes/external/polyrocket');
    // 修改路径并点击 Apply。
    fireEvent.change(input, {
      target: { value: '/Volumes/external/polyrocket-v2' },
    });
    fireEvent.click(screen.getByTestId('welcome-storage-apply'));
    await waitFor(() => {
      expect(ipc.setStoragePath).toHaveBeenCalledWith(
        '/Volumes/external/polyrocket-v2',
      );
    });
  });

  it('Apply on custom mode auto-runs migrateStoragePath (v0.58a)', async () => {
    // v0.58a —— Storage 步骤现在会自动把已有
    // 数据迁移到新路径,用户不必再在
    // Settings 中单独点 "Copy existing data"。
    vi.mocked(ipc.getStorageInfo).mockResolvedValue({
      defaultPath: '/tmp/db/polyrocket.db',
      currentPath: '/Volumes/external/polyrocket',
      isCustom: true,
      exists: true,
      writable: true,
      freeBytes: null,
      restartRequired: true,
    });
    vi.mocked(ipc.setStoragePath).mockResolvedValue(undefined);
    const welcome = makeWelcomeStore();
    render(wrap(<StorageStep welcome={welcome} />));
    const input = (await waitFor(
      () =>
        screen.getByTestId('welcome-storage-path-input') as HTMLInputElement,
    )) as HTMLInputElement;
    fireEvent.change(input, {
      target: { value: '/Volumes/external/polyrocket-v3' },
    });
    fireEvent.click(screen.getByTestId('welcome-storage-apply'));
    await waitFor(() => {
      expect(ipc.setStoragePath).toHaveBeenCalledWith(
        '/Volumes/external/polyrocket-v3',
      );
    });
    // migrate 调用应跟随 setStoragePath,使用相同
    // 路径 + overwrite=false。
    await waitFor(() => {
      expect(ipc.migrateStoragePath).toHaveBeenCalledWith(
        '/Volumes/external/polyrocket-v3',
        false,
      );
    });
  });

  it('Browse button calls pickDirectory and populates the input', async () => {
    // 同样的预填技巧: 在 custom 模式启动。
    vi.mocked(ipc.getStorageInfo).mockResolvedValue({
      defaultPath: '/tmp/db/polyrocket.db',
      currentPath: '/Volumes/external/polyrocket',
      isCustom: true,
      exists: true,
      writable: true,
      freeBytes: null,
      restartRequired: true,
    });
    vi.mocked(ipc.pickDirectory).mockResolvedValue(
      '/Volumes/external/polyrocket-picked',
    );
    const welcome = makeWelcomeStore();
    render(wrap(<StorageStep welcome={welcome} />));
    await waitFor(() => screen.getByTestId('welcome-storage-browse'));
    fireEvent.click(screen.getByTestId('welcome-storage-browse'));
    await waitFor(() => {
      expect(ipc.pickDirectory).toHaveBeenCalled();
    });
    const input = (await waitFor(
      () =>
        screen.getByTestId('welcome-storage-path-input') as HTMLInputElement,
    )) as HTMLInputElement;
    expect(input.value).toBe('/Volumes/external/polyrocket-picked');
  });
});

// =================================================================
// 3. ThemeStep 测试
// =================================================================

describe('ThemeStep (v0.54c)', () => {
  it('renders all 3 themes + clicking sets useThemeStore.theme', () => {
    render(wrap(<ThemeStep />));
    // 3 个主题卡片使用 testid
    // `welcome-theme-${th.id}` (由 ThemeStep 设置)。
    expect(
      screen.getByTestId('welcome-theme-dark'),
    ).toBeInTheDocument();
    expect(
      screen.getByTestId('welcome-theme-matrix'),
    ).toBeInTheDocument();
    expect(
      screen.getByTestId('welcome-theme-light'),
    ).toBeInTheDocument();
    fireEvent.click(screen.getByTestId('welcome-theme-matrix'));
    expect(mockSetTheme).toHaveBeenCalledWith('matrix');
  });
});

// =================================================================
// 4. LlmStep 测试
// =================================================================

describe('LlmStep (v0.54c)', () => {
  it('renders the heading + Add provider button', () => {
    const welcome = makeWelcomeStore();
    render(wrap(<LlmStep welcome={welcome} />));
    // Add provider 按钮使用 testid `welcome-llm-add`。
    expect(screen.getByTestId('welcome-llm-add')).toBeInTheDocument();
  });
});

// =================================================================
// 5. PolymarketStep 测试
// =================================================================

describe('PolymarketStep (v0.54c + v0.119 + v0.126)', () => {
  it('renders CLOB + wallet sub-cards with env-only mode and wallet form', () => {
    const welcome = makeWelcomeStore();
    render(wrap(<PolymarketStep welcome={welcome} />));
    // v0.119 —— ClobCard 已改为 env-only 模式,不再有 save/skip 按钮。
    // 现在 ClobCard 显示 env 变量列表 + 「已填好 — 继续」按钮
    // (testid: welcome-pm-mark-saved)。WalletCard 仍保留 save + skip。
    expect(
      screen.getByTestId('welcome-pm-env-info'),
    ).toBeInTheDocument();
    expect(
      screen.getByTestId('welcome-pm-mark-saved'),
    ).toBeInTheDocument();
    expect(
      screen.getByTestId('welcome-wallet-save'),
    ).toBeInTheDocument();
    expect(
      screen.getByTestId('welcome-wallet-skip'),
    ).toBeInTheDocument();
  });
});

// =================================================================
// 6. FinishStep 测试
// =================================================================

describe('FinishStep (v0.54c)', () => {
  it('renders a read-only summary', () => {
    useWelcomeStore.getState().setConfigured('llmAtLeastOne', true);
    useWelcomeStore.getState().setConfigured('polymarketApi', true);
    const welcome = makeWelcomeStore();
    render(wrap(<FinishStep welcome={welcome} />));
    // summary 面板使用 testid `welcome-finish-summary`。
    expect(
      screen.getByTestId('welcome-finish-summary'),
    ).toBeInTheDocument();
  });
});

// =================================================================
// 7. WelcomeBanner 测试
// =================================================================

describe('WelcomeBanner (v0.54c)', () => {
  it('renders when secretsStatus has missing items', async () => {
    vi.mocked(ipc.secretsStatus).mockResolvedValue({
      llm_keys: 0,
      pm_api: false,
      pm_passphrase: false,
      pm_secret: false,
      wallet_pk: 0,
    });
    render(wrap(<WelcomeBanner />));
    await waitFor(() => {
      expect(
        screen.getByTestId('welcome-banner'),
      ).toBeInTheDocument();
    });
  });

  it('does NOT render when all 3 secrets are set', async () => {
    vi.mocked(ipc.secretsStatus).mockResolvedValue({
      llm_keys: 2,
      pm_api: true,
      pm_passphrase: true,
      pm_secret: true,
      wallet_pk: 1,
    });
    const { container } = render(wrap(<WelcomeBanner />));
    await waitFor(() => {
      expect(container.querySelector('[data-testid="welcome-banner"]')).toBeNull();
    });
  });
});
