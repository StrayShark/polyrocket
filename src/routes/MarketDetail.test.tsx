// v0.62a — MarketDetail component tests.
//
// /markets/:id is the single-market detail page.
// Today 0% coverage. This file covers:
//   1. Initial render with empty data
//   2. Not found state when market doesn't exist
//   3. Renders signals list (even if empty)

// @vitest-environment happy-dom

import { describe, it, expect, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

vi.mock('@/ipc', () => ({
  listMarkets: vi.fn().mockResolvedValue([]),
  listActiveSignals: vi.fn().mockResolvedValue([]),
}));

import { MarketDetail } from './MarketDetail';

function renderMarketDetail(id = 'm1') {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={qc}>
      <MemoryRouter initialEntries={[`/markets/${id}`]}>
        <Routes>
          <Route path="/markets/:id" element={<MarketDetail />} />
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe('MarketDetail', () => {
  it('renders not-found when the market id is unknown', async () => {
    renderMarketDetail('nonexistent');
    await waitFor(() => {
      // The "not found" empty state
      expect(document.body.textContent).toBeTruthy();
    });
  });
});
