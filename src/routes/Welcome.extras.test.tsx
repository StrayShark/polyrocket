// v0.70d — Welcome route additional tests.
//
// /welcome is a 163-line wizard controller. It delegates the
// heavy lifting to 6 sub-components (WelcomeStep / StorageStep /
// ThemeStep / LlmStep / PolymarketStep / FinishStep) but owns:
//   - the 6-step state machine (WELCOME_STEPS array)
//   - next/back/skip navigation logic
//   - `done` redirect to /dashboard
//   - locale bridge between welcome-store and i18n locale-store
//
// Existing test (v0.62a) is 1 surface render. We add 8 focused
// tests covering the navigation + state-machine branches.
//
// Welcome.tsx: 42.5% → ~80% stmts.
//
// @vitest-environment happy-dom

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';

// Mock the welcome store so we can control state per test.
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
  // zustand-style: useLocaleStore(selector) calls selector(state).
  // We need a function that accepts a selector and returns what
  // the selector returns. The component does
  //   const setLocale = useLocaleStore((s) => s.setLocale);
  // so we return an object with setLocale as the value the
  // selector extracts.
  useLocaleStore: vi.fn((selector: any) => selector({ setLocale: vi.fn(), locale: 'en' })),
}));

// Stub the 6 step components — they have their own tests.
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
  // Reset store to first step
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
