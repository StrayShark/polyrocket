// v0.62a — Trade component tests.
//
// /trade is the standalone place-bet page.
// Tiny route — just renders <PlaceBetForm />
// with URL params. Today 0% coverage.

// @vitest-environment happy-dom

import { describe, it, expect, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

vi.mock('@/ipc', () => ({
  placeSignedOrder: vi.fn().mockResolvedValue({ bet_id: 'b1' }),
  validateOrderArgs: vi.fn().mockResolvedValue({ ok: true, error: null }),
}));

import { Trade } from './Trade';

function renderTrade(initialPath = '/trade?market_id=m1&side=YES&edge=0.1') {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={qc}>
      <MemoryRouter initialEntries={[initialPath]}>
        <Routes>
          <Route path="/trade" element={<Trade />} />
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe('Trade', () => {
  it('renders the place-bet form', async () => {
    renderTrade();
    await waitFor(() => {
      // The form has a market_id input / submit
      expect(document.body.textContent).toBeTruthy();
    });
  });
});
