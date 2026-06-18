// v0.62a — Help component tests.
//
// /help is the documentation + quick-start
// page. No IPC, pure render. Today 0% coverage.

// @vitest-environment happy-dom

import { describe, it, expect, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';

vi.mock('@/lib/i18n', () => ({
  useT: () => ({ t: (k: string) => {
    // Help.tsx uses t(...).split('/signals') / .split('/analysis') — return
    // strings with those markers so the split has at least 2 elements.
    if (k === 'help.quick.step5') return 'Go to /signals and /analysis for live data.';
    if (k === 'help.quick.step2') return 'Open /wallets to add a wallet.';
    return k;
  } }),
}));

import { Help } from './Help';

describe('Help', () => {
  it('renders the page', async () => {
    render(
      <MemoryRouter>
        <Help />
      </MemoryRouter>,
    );
    await waitFor(() => {
      // Multiple "help" text occurrences (h2 + nav)
      expect(screen.getAllByText(/help/i).length).toBeGreaterThan(0);
    });
  });
});
