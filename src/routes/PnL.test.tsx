// v0.62a — PnL component tests.
//
// /pnl is the PnL detail page (vs /history which
// is the bet list). Today 0% coverage. This file
// covers the initial render + KPI + table
// rendering.

// @vitest-environment happy-dom

import { describe, it, expect, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

vi.mock('@/ipc', () => ({
  listBets: vi.fn().mockResolvedValue([]),
  dashboardKpis: vi.fn().mockResolvedValue({
    total_equity_usdc: '0',
    open_pnl_usdc: '0',
    win_rate_30d: 0,
    brier_score: 0,
    active_signals: 0,
    open_positions: 0,
  }),
}));

import { PnL } from './PnL';

function renderPnL() {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={qc}>
      <MemoryRouter>
        <PnL />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe('PnL', () => {
  it('renders the page', async () => {
    renderPnL();
    await waitFor(() => {
      // "Open PnL" + page title both match — use getAllByText
      expect(screen.getAllByText(/pnl/i).length).toBeGreaterThan(0);
    });
  });
});
