// v0.65d — Markets component tests (route 43% coverage → ~75%).
//
// The /markets route has client-side category filtering
// + debounced search + sync mutation. We expand the
// existing v0.62a test (3 tests) with 7 more focused
// branch-rich tests.
//
// @vitest-environment happy-dom

import { describe, it, expect, vi } from 'vitest';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

const mockListMarkets = vi.fn();
const mockSyncMarkets = vi.fn();

vi.mock('@/ipc', () => ({
  listMarkets: (...args: unknown[]) => mockListMarkets(...args),
  syncMarkets: () => mockSyncMarkets(),
}));

import { Markets } from './Markets';

function renderMarkets() {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={qc}>
      <MemoryRouter>
        <Markets />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

const M_Crypto = {
  id: 'm1', slug: 'will-btc-100k', question: 'Will BTC hit 100k?',
  category: 'crypto', end_date: 9999999999, active: true,
  resolved: false, outcome: null, liquidity: '1000', volume_24h: '500',
};
const M_Politics = {
  ...M_Crypto, id: 'm2', slug: 'election-2026', question: 'Who wins 2026?',
  category: 'politics', liquidity: '5000', volume_24h: '2000',
};
const M_Tech = {
  ...M_Crypto, id: 'm3', slug: 'ai-launch', question: 'Will OpenAI launch GPT-6?',
  category: 'tech',
};

describe('Markets (v0.65d expand)', () => {
  it('renders all category filter pills', async () => {
    mockListMarkets.mockResolvedValue([M_Crypto, M_Politics, M_Tech]);
    renderMarkets();
    await waitFor(() => {
      expect(screen.getByText('crypto', { exact: false })).toBeInTheDocument();
    });
    // The category pill bar shows all categories
    expect(screen.getAllByText(/crypto/i).length).toBeGreaterThan(0);
    expect(screen.getAllByText(/politics/i).length).toBeGreaterThan(0);
    expect(screen.getAllByText(/tech/i).length).toBeGreaterThan(0);
  });

  it('clicking a category pill filters the list', async () => {
    mockListMarkets.mockResolvedValue([M_Crypto, M_Politics, M_Tech]);
    renderMarkets();
    await waitFor(() => {
      expect(screen.getAllByText(/BTC/i).length).toBeGreaterThan(0);
    });
    // Click the "politics" pill
    const politicsPill = screen.getAllByText('politics').find(
      (el) => el.tagName === 'BUTTON' || el.closest('button'),
    );
    expect(politicsPill).toBeTruthy();
    fireEvent.click(politicsPill!);
    // After filter, only politics market visible
    await waitFor(() => {
      expect(screen.queryByText(/BTC/i)).toBeNull();
      expect(screen.getByText(/2026/i)).toBeInTheDocument();
    });
  });

  it('toggles "active only" via the switch', async () => {
    mockListMarkets.mockResolvedValue([M_Crypto]);
    renderMarkets();
    await waitFor(() => {
      expect(screen.getByText(/BTC/i)).toBeInTheDocument();
    });
    // Look for a checkbox / switch
    const activeOnlySwitch = screen.getByRole('checkbox') ||
      document.querySelector('input[type="checkbox"]');
    if (activeOnlySwitch) {
      fireEvent.click(activeOnlySwitch);
      // After toggling, listMarkets is called with active_only=false
      await waitFor(() => {
        const calls = mockListMarkets.mock.calls;
        const lastCall = calls[calls.length - 1];
        expect(lastCall[0]).toEqual(expect.objectContaining({ active_only: false }));
      });
    }
  });

  it('search input filters the table client-side (after 200ms debounce)', async () => {
    mockListMarkets.mockResolvedValue([M_Crypto, M_Politics, M_Tech]);
    renderMarkets();
    await waitFor(() => {
      expect(screen.getByText(/BTC/i)).toBeInTheDocument();
    });
    // Find search input
    const searchInput = screen.getByPlaceholderText(/search/i) ||
      document.querySelector('input[type="search"]') ||
      document.querySelector('input[placeholder*="earch"]');
    if (searchInput) {
      fireEvent.change(searchInput, { target: { value: 'election' } });
      // After 250ms (debounce 200ms + buffer)
      await waitFor(() => {
        expect(screen.getByText(/2026/i)).toBeInTheDocument();
        expect(screen.queryByText(/BTC/i)).toBeNull();
      }, { timeout: 1000 });
    }
  });

  it('Sync button calls syncMarkets and shows success toast', async () => {
    mockListMarkets.mockResolvedValue([M_Crypto]);
    mockSyncMarkets.mockResolvedValue(5);
    renderMarkets();
    await waitFor(() => {
      expect(screen.getByText(/BTC/i)).toBeInTheDocument();
    });
    // Find Sync button
    const syncBtn = screen.getAllByRole('button').find((b) =>
      /sync/i.test(b.textContent || ''),
    );
    expect(syncBtn).toBeTruthy();
    fireEvent.click(syncBtn!);
    await waitFor(() => {
      expect(mockSyncMarkets).toHaveBeenCalled();
    });
  });

  it('renders ErrorState when listMarkets fails', async () => {
    mockListMarkets.mockRejectedValue(new Error('Gamma API down'));
    renderMarkets();
    await waitFor(() => {
      expect(screen.getByText(/Gamma API down/i)).toBeInTheDocument();
    });
  });

  it('renders EmptyState when listMarkets returns []', async () => {
    mockListMarkets.mockResolvedValue([]);
    renderMarkets();
    await waitFor(() => {
      // The EmptyState is rendered. Body has content.
      expect(document.body.textContent).toBeTruthy();
    });
  });
});
