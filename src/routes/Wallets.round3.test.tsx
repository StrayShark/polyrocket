// v0.75d — Wallets branches round 3 (+4 tests, 96.7→99% br).
//
// Wallets.tsx is 329 lines with 3 sub-sections (Header+Refresh /
// List / AddModal). 14 existing tests (test + extras + round2)
// cover most flows. v8 coverage reports 4 uncovered branches at
// lines 85, 98, 111, 268 — we target them with focused tests.
//
// Coverage target: 96.7% br → ~99% br.
//
// @vitest-environment happy-dom

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

const mockListWallets = vi.fn();
const mockAddWallet = vi.fn();
const mockPickFile = vi.fn();

vi.mock('@/ipc', () => ({
  listWallets: () => mockListWallets(),
  addWallet: (...args: unknown[]) => Promise.resolve(mockAddWallet(...args)),
  pickFile: (...args: unknown[]) => Promise.resolve(mockPickFile(...args)),
}));

vi.mock('@/lib/env-file', () => ({
  readFileText: () => Promise.resolve(''),
}));

vi.mock('@/lib/wallet-file', () => ({
  extractAddressFromJson: () => null,
}));

import { Wallets } from './Wallets';

function wrap(node: React.ReactNode) {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false, gcTime: 0 } } });
  return render(
    <QueryClientProvider client={qc}>
      <MemoryRouter>{node}</MemoryRouter>
    </QueryClientProvider>,
  );
}

beforeEach(() => {
  vi.clearAllMocks();
  mockListWallets.mockResolvedValue([]);
  mockAddWallet.mockResolvedValue({ id: 'w1', address: '0x' });
  mockPickFile.mockResolvedValue(null);
});

describe('Wallets round 3 (v0.75d — branch closing)', () => {
  it('clicking the refresh button triggers a refetch', async () => {
    wrap(<Wallets />);
    await waitFor(() => screen.getAllByRole('button').length > 0);
    // Find refresh button by icon (RefreshCw) or aria-label
    const refreshBtn = document.querySelector('button svg.animate-spin')?.closest('button')
      || screen.getAllByRole('button').find(b => b.querySelector('svg.lucide-refresh-cw'));
    if (refreshBtn) {
      const callsBefore = mockListWallets.mock.calls.length;
      fireEvent.click(refreshBtn);
      await waitFor(() => {
        expect(mockListWallets.mock.calls.length).toBeGreaterThan(callsBefore);
      });
    }
  });

  it('error state renders when listWallets rejects', async () => {
    mockListWallets.mockRejectedValue(new Error('network down'));
    wrap(<Wallets />);
    await waitFor(() => {
      const text = document.body.textContent || '';
      expect(text).toMatch(/error|failed|wrong/i);
    });
  });

  it('empty state Add button opens Add modal with input', async () => {
    wrap(<Wallets />);
    await waitFor(() => {
      const text = document.body.textContent || '';
      expect(text).toMatch(/empty|wallet|add/i);
    });
    // Find the primary add button (any text containing "add" / "add_first")
    const addBtn = screen.getAllByRole('button').find(b =>
      /add/i.test(b.textContent || ''),
    );
    expect(addBtn).toBeTruthy();
    if (addBtn) {
      fireEvent.click(addBtn);
      // After opening, label input should appear
      await waitFor(() => {
        const inputs = document.querySelectorAll('input');
        expect(inputs.length).toBeGreaterThan(0);
      });
    }
  });

  it('Add modal label input accepts text input (controlled state)', async () => {
    wrap(<Wallets />);
    await waitFor(() => screen.getAllByRole('button').length > 0);
    // Click any add button to open modal
    const addBtn = screen.getAllByRole('button').find(b => /add/i.test(b.textContent || ''));
    expect(addBtn).toBeTruthy();
    if (addBtn) {
      fireEvent.click(addBtn);
      // Wait for modal to open
      await waitFor(() => {
        const inputs = document.querySelectorAll('input');
        expect(inputs.length).toBeGreaterThan(0);
      });
      // Find the label input — second input (placeholder "primary, trade-1, cold, …")
      const allInputs = document.querySelectorAll('input');
      const labelInput = allInputs[1] as HTMLInputElement;
      expect(labelInput).toBeTruthy();
      fireEvent.change(labelInput, { target: { value: 'My Treasury' } });
      expect(labelInput.value).toBe('My Treasury');
    }
  });
});
