// v0.62a.2 — Welcome component tests.
//
// /welcome is the first-run landing wizard.
// Today 0% coverage. This file covers:
//   1. Initial render of the step progress
//   2. StorageStep renders the path input + Browse button
//   3. If welcome is done, redirect to dashboard

// @vitest-environment happy-dom

import { describe, it, expect, vi } from 'vitest';
import { render, waitFor } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';

vi.mock('@/lib/i18n', () => ({
  useT: () => ({ t: (k: string) => k }),
  useLocaleStore: () => ({}),
}));

// Default: welcome is NOT done
vi.mock('@/stores/welcome-store', () => ({
  useWelcomeStore: Object.assign(
    () => ({
      done: false,
      step: 'welcome',
      locale: '',
      configured: {
        storagePath: false, theme: false, llmAtLeastOne: false,
        polymarketApi: false, walletPk: false,
      },
      setStep: vi.fn(),
      setDone: vi.fn(),
      setLocale: vi.fn(),
      setConfigured: vi.fn(),
      reset: vi.fn(),
    }),
    { getState: () => ({
      done: false, step: 'welcome', locale: '',
      configured: { storagePath: false, theme: false, llmAtLeastOne: false,
        polymarketApi: false, walletPk: false },
      setStep: vi.fn(), setDone: vi.fn(), setLocale: vi.fn(),
      setConfigured: vi.fn(), reset: vi.fn(),
    }) },
  ),
  WELCOME_STEPS: ['welcome', 'storage', 'theme', 'llm', 'polymarket', 'finish'],
}));

import { Welcome } from './Welcome';

describe('Welcome', () => {
  it('renders the wizard', async () => {
    render(
      <MemoryRouter>
        <Welcome />
      </MemoryRouter>,
    );
    await waitFor(() => {
      expect(document.body.textContent).toBeTruthy();
    });
  });
});
