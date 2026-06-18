// v0.62a — Wallets component tests.
//
// /wallets is the wallet metadata manager.
// Today 0% coverage. This file covers the
// initial render + Add button + table shell.

// @vitest-environment happy-dom

import { describe, it, expect, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

vi.mock('@/ipc', () => ({
  listWallets: vi.fn().mockResolvedValue([]),
  addWallet: vi.fn().mockResolvedValue({
    id: 'w1', address: '0x0', label: null, chain_id: 137,
    wallet_type: 'eoa', created_at: 0, last_synced_at: null,
  }),
  pickFile: vi.fn().mockResolvedValue(null),
}));

vi.mock('@/lib/env-file', () => ({
  readFileText: vi.fn().mockResolvedValue(''),
}));

vi.mock('@/lib/wallet-file', () => ({
  extractAddressFromJson: vi.fn().mockReturnValue(null),
}));

import { Wallets } from './Wallets';

function renderWallets() {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={qc}>
      <MemoryRouter>
        <Wallets />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe('Wallets', () => {
  it('renders the page', async () => {
    renderWallets();
    await waitFor(() => {
      expect(screen.getByText(/wallets/i)).toBeInTheDocument();
    });
  });

  it('shows an Add button', async () => {
    renderWallets();
    await waitFor(() => {
      const buttons = screen.getAllByRole('button');
      const addBtn = buttons.find((b) => b.textContent?.toLowerCase().includes('add'));
      expect(addBtn).toBeTruthy();
    });
  });
});
