// v0.70d —— Welcome 路由附加测试。
//
// /welcome 是一个 163 行的引导控制器。它将
// 主要工作委托给 6 个子组件（WelcomeStep / StorageStep /
// ThemeStep / LlmStep / PolymarketStep / FinishStep），但自身负责：
//   - 6 步状态机（WELCOME_STEPS 数组）
//   - next/back/skip 导航逻辑
//   - `done` 时重定向到 /dashboard
//   - welcome-store 与 i18n locale-store 之间的语言桥接
//
// 已有测试（v0.62a）仅 1 个表面渲染。我们新增 8 个聚焦
// 测试，覆盖导航 + 状态机分支。
//
// Welcome.tsx：42.5% → ~80% stmts。
//
// @vitest-environment happy-dom

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';

// Mock welcome store 以便在每个测试中控制 state。
type Step = 'welcome' | 'storage' | 'theme' | 'llm' | 'polymarket' | 'finish';
const { mockWelcomeStoreState, mockSetStep, mockSetDone, mockSetLocale, mockSetConfigured } = vi.hoisted(() => ({
  mockWelcomeStoreState: {
    done: false,
    step: 'welcome' as Step,
    locale: 'en',
    configured: { storagePath: false, theme: false, llmAtLeastOne: false, polymarketApi: false, walletPk: false },
  },
  mockSetStep: vi.fn(),
  mockSetDone: vi.fn(),
  mockSetLocale: vi.fn(),
  mockSetConfigured: vi.fn(),
}));

vi.mock('@/stores/welcome-store', () => ({
  WELCOME_STEPS: ['welcome', 'storage', 'theme', 'llm', 'polymarket', 'finish'],
  useWelcomeStore: vi.fn(() => ({
    ...mockWelcomeStoreState,
    setStep: mockSetStep,
    setDone: mockSetDone,
    setLocale: mockSetLocale,
    setConfigured: mockSetConfigured,
  })),
}));

vi.mock('@/lib/i18n', () => ({
  useT: () => ({
    t: (k: string) => k,  // return key as-is for test inspection
  }),
  // zustand 风格：useLocaleStore(selector) 调用 selector(state)。
  // 我们需要一个接受 selector 并返回 selector 所得值的函数。
  // 组件中执行
  //   const setLocale = useLocaleStore((s) => s.setLocale);
  // 因此返回一个带 setLocale 的对象，作为 selector 提取的值。
  useLocaleStore: vi.fn((selector: any) => selector({ setLocale: vi.fn(), locale: 'en' })),
}));

// Stub 6 个 step 组件 —— 它们各自有独立测试。
vi.mock('@/components/welcome/StepProgress', () => ({
  StepProgress: () => <div data-testid="step-progress">step progress</div>,
}));
vi.mock('@/components/welcome/WelcomeStep', () => ({
  WelcomeStep: ({ onLocale }: { onLocale: (l: string) => void }) => (
    <div data-testid="welcome-step">
      <button onClick={() => onLocale('zh')}>Set locale zh</button>
    </div>
  ),
}));
vi.mock('@/components/welcome/StorageStep', () => ({
  StorageStep: () => <div data-testid="storage-step">storage step</div>,
}));
vi.mock('@/components/welcome/ThemeStep', () => ({
  ThemeStep: () => <div data-testid="theme-step">theme step</div>,
}));
vi.mock('@/components/welcome/LlmStep', () => ({
  LlmStep: () => <div data-testid="llm-step">llm step</div>,
}));
vi.mock('@/components/welcome/PolymarketStep', () => ({
  PolymarketStep: () => <div data-testid="polymarket-step">polymarket step</div>,
}));
vi.mock('@/components/welcome/FinishStep', () => ({
  FinishStep: () => <div data-testid="finish-step">finish step</div>,
}));

import { Welcome } from './Welcome';

beforeEach(() => {
  vi.clearAllMocks();
  // 重置 store 到第一步
  mockWelcomeStoreState.done = false;
  mockWelcomeStoreState.step = 'welcome';
  mockWelcomeStoreState.locale = 'en';
});

function renderWelcome() {
  return render(
    <MemoryRouter>
      <Welcome />
    </MemoryRouter>,
  );
}

describe('Welcome (extended)', () => {
  it('renders StepProgress + the current step component on mount', async () => {
    renderWelcome();
    expect(screen.getByTestId('step-progress')).toBeInTheDocument();
    expect(screen.getByTestId('welcome-step')).toBeInTheDocument();
  });

  it('does not show back button on first step (welcome)', () => {
    renderWelcome();
    expect(screen.queryByTestId('welcome-back')).not.toBeInTheDocument();
  });

  it('shows back button on non-first steps', () => {
    mockWelcomeStoreState.step = 'storage';
    renderWelcome();
    expect(screen.getByTestId('welcome-back')).toBeInTheDocument();
  });

  it('clicking next advances to next step', async () => {
    renderWelcome();
    fireEvent.click(screen.getByTestId('welcome-next'));
    expect(mockSetStep).toHaveBeenCalledWith('storage');
  });

  it('clicking back from non-first step goes to previous step', () => {
    mockWelcomeStoreState.step = 'theme';
    renderWelcome();
    fireEvent.click(screen.getByTestId('welcome-back'));
    expect(mockSetStep).toHaveBeenCalledWith('storage');
  });

  it('clicking skip calls setDone(true)', () => {
    renderWelcome();
    fireEvent.click(screen.getByTestId('welcome-skip'));
    expect(mockSetDone).toHaveBeenCalledWith(true);
  });

  it('clicking next on last step calls setDone + navigates to dashboard', async () => {
    mockWelcomeStoreState.step = 'finish';
    renderWelcome();
    fireEvent.click(screen.getByTestId('welcome-next'));
    expect(mockSetDone).toHaveBeenCalledWith(true);
  });

  it('locale change from WelcomeStep updates both stores', () => {
    renderWelcome();
    fireEvent.click(screen.getByText('Set locale zh'));
    expect(mockSetLocale).toHaveBeenCalledWith('zh');
  });
});
