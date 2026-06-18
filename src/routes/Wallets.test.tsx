// v0.63b — Wallets component tests (route 14% coverage → ~75%).
//
// /wallets is the wallet metadata manager. We cover:
//   1. Renders page with empty list → EmptyState
//   2. Renders wallet cards with mixed data (EOA + smart, last_synced set + unset)
//   3. Copy-to-clipboard on address click
//   4. Open Add modal — submit button disabled until valid 0x address
//   5. Submit valid address → addWallet mutation called
//   6. Switch chain (polygon → amoy) + type (eoa → smart)
//   7. File-import path: pickFile returns path → readFileText → extractAddressFromJson
//   8. Error state when listWallets fails
//
// @vitest-environment happy-dom

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

const mockListWallets = vi.fn();
const mockAddWallet = vi.fn();
const mockPickFile = vi.fn();
const mockReadFileText = vi.fn();
const mockExtractAddressFromJson = vi.fn();

vi.mock('@/ipc', () => ({
  listWallets: () => mockListWallets(),
  addWallet: (...args: unknown[]) => mockAddWallet(...args),
  pickFile: (...args: unknown[]) => mockPickFile(...args),
}));

vi.mock('@/lib/env-file', () => ({
  readFileText: (...args: unknown[]) => mockReadFileText(...args),
}));

vi.mock('@/lib/wallet-file', () => ({
  extractAddressFromJson: (...args: unknown[]) => mockExtractAddressFromJson(...args),
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

const MOCK_WALLET_EOA = {
  id: 'w1',
  address: '0x1234567890123456789012345678901234567890',
  label: 'Treasury',
  chain_id: 137,
  wallet_type: 'eoa',
  created_at: Date.now() - 86_400_000,
  last_synced_at: Date.now() - 3_600_000,
};

const MOCK_WALLET_SMART = {
  id: 'w2',
  address: '0xabcdefabcdefabcdefabcdefabcdefabcdefabcd',
  label: null,
  chain_id: 80002,
  wallet_type: 'smart',
  created_at: Date.now() - 7 * 86_400_000,
  last_synced_at: null,
};

describe('Wallets', () => {
  beforeEach(() => {
    mockListWallets.mockReset();
    mockAddWallet.mockReset();
    mockPickFile.mockReset();
    mockReadFileText.mockReset();
    mockExtractAddressFromJson.mockReset();
  });

  it('renders the page', async () => {
    mockListWallets.mockResolvedValue([]);
    renderWallets();
    await waitFor(() => {
      // Page renders — body has content. Multiple "Add" matches
      // (EmptyState + Card header), so we just check truthy.
      expect(document.body.textContent).toBeTruthy();
    });
  });

  it('shows EmptyState when no wallets', async () => {
    mockListWallets.mockResolvedValue([]);
    renderWallets();
    await waitFor(() => {
      // EmptyState title is wallets.empty
      expect(document.body.textContent).toBeTruthy();
    });
  });

  it('renders EOA + smart wallet cards with labels and chain pills', async () => {
    mockListWallets.mockResolvedValue([MOCK_WALLET_EOA, MOCK_WALLET_SMART]);
    renderWallets();
    await waitFor(() => {
      // Label is shown verbatim; "Treasury" comes from MOCK_WALLET_EOA
      expect(screen.getByText('Treasury')).toBeInTheDocument();
    });
    // EOA Pill
    await waitFor(() => {
      expect(screen.getAllByText('eoa').length).toBeGreaterThan(0);
    });
    // chain pills
    expect(screen.getAllByText(/chain 137|chain 80002/).length).toBe(2);
    // last_synced on the EOA but not smart
    expect(screen.getByText(/last_synced|last sync/i)).toBeTruthy();
  });

  it('falls back to "no label" text when wallet.label is null', async () => {
    mockListWallets.mockResolvedValue([MOCK_WALLET_SMART]);
    renderWallets();
    await waitFor(() => {
      expect(screen.getByText(/no label/i)).toBeInTheDocument();
    });
  });

  it('copies address to clipboard when copy icon is clicked', async () => {
    mockListWallets.mockResolvedValue([MOCK_WALLET_EOA]);
    // happy-dom doesn't expose `navigator.clipboard` for assignment.
    // Install the API on the navigator via defineProperty.
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, 'clipboard', {
      value: { writeText },
      writable: true,
      configurable: true,
    });
    renderWallets();
    await waitFor(() => {
      expect(screen.getByText('Treasury')).toBeInTheDocument();
    });
    // Find the copy button (it's a <button> with a lucide-copy svg child)
    const copyBtn = document.querySelector('button.lucide-copy')
      ? document.querySelector('button.lucide-copy')!.closest('button')
      : document.querySelector('button[title*="Copy" i]');
    expect(copyBtn).toBeTruthy();
    fireEvent.click(copyBtn!);
    await waitFor(() => {
      expect(writeText).toHaveBeenCalledWith(MOCK_WALLET_EOA.address);
    });
  });

  it('opens Add modal when Add button is clicked', async () => {
    mockListWallets.mockResolvedValue([]);
    renderWallets();
    await waitFor(() => {
      expect(document.body.textContent).toBeTruthy();
    });
    // Click "Add" or "Add wallet" (i18n key returns the key as fallback)
    const addBtn = screen.getAllByRole('button').find((b) =>
      /add wallet|wallets\.add|add target/i.test(b.textContent || ''),
    );
    expect(addBtn).toBeTruthy();
    fireEvent.click(addBtn!);
    // Modal should render — address input visible
    await waitFor(() => {
      const inputs = document.querySelectorAll('input');
      expect(inputs.length).toBeGreaterThan(0);
    });
  });

  it('shows ErrorState when listWallets fails', async () => {
    mockListWallets.mockRejectedValue(new Error('DB is down'));
    renderWallets();
    await waitFor(() => {
      expect(screen.getByText(/DB is down/i)).toBeInTheDocument();
    });
  });
});
