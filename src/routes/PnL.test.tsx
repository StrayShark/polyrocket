// v0.63b — PnL component tests (route 25% coverage → ~75%).
//
// /pnl is the realized-PnL detail page (3 sections:
// KPI row, Brier card, Breakdown table). Pure read —
// no mutations. We cover:
//   1. Renders page with mock kpis + empty bets → EmptyState
//   2. Renders Breakdown table with mixed won/lost/open bets
//   3. Brier score quality text thresholds (Excellent/Good/...)
//   4. Loading skeletons (no data yet)
//   5. Error state when kpis query fails
//
// @vitest-environment happy-dom

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

const mockListBets = vi.fn();
const mockKpis = vi.fn();

vi.mock('@/ipc', () => ({
  listBets: (...args: unknown[]) => mockListBets(...args),
  dashboardKpis: () => mockKpis(),
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
  beforeEach(() => {
    mockKpis.mockReset();
    mockListBets.mockReset();
  });

  it('renders empty state when there are no bets', async () => {
    mockKpis.mockResolvedValue({
      total_equity_usdc: '0', open_pnl_usdc: '0', win_rate_30d: 0,
      brier_score: 0.0, active_signals: 0, open_positions: 0,
    });
    mockListBets.mockResolvedValue([]);
    renderPnL();
    await waitFor(() => {
      expect(screen.getByText(/Total Equity/i)).toBeInTheDocument();
    });
    // The Breakdown card renders EmptyState when no bets
    await waitFor(() => {
      expect(screen.getByText(/place bets to see/i)).toBeInTheDocument();
    });
  });

  it('renders breakdown table with mixed won/lost/open bets', async () => {
    mockKpis.mockResolvedValue({
      total_equity_usdc: '1500.50', open_pnl_usdc: '120.30', win_rate_30d: 0.6,
      brier_score: 0.08, active_signals: 3, open_positions: 2,
    });
    mockListBets.mockResolvedValue([
      { id: 'b1', pnl: '50.00',  status: 'won'  },
      { id: 'b2', pnl: '30.00',  status: 'won'  },
      { id: 'b3', pnl: '-20.00', status: 'lost' },
      { id: 'b4', pnl: null,     status: 'open' },
    ]);
    renderPnL();
    await waitFor(() => {
      expect(screen.getByText(/Total Equity/i)).toBeInTheDocument();
    });
    // Breakdown shows counts via BreakdownCell
    await waitFor(() => {
      // "Open" / "Won" / "Lost" / "Realized" labels
      expect(screen.getByText(/^Open$/i)).toBeInTheDocument();
      expect(screen.getByText(/^Won$/i)).toBeInTheDocument();
      expect(screen.getByText(/^Lost$/i)).toBeInTheDocument();
      expect(screen.getByText(/^Realized$/i)).toBeInTheDocument();
    });
  });

  it('shows Excellent calibration text when brier_score < 0.1', async () => {
    mockKpis.mockResolvedValue({
      total_equity_usdc: '0', open_pnl_usdc: '0', win_rate_30d: 0,
      brier_score: 0.08, active_signals: 0, open_positions: 0,
    });
    mockListBets.mockResolvedValue([]);
    renderPnL();
    await waitFor(() => {
      expect(screen.getByText(/Excellent calibration/i)).toBeInTheDocument();
    });
  });

  it('shows Good calibration text when 0.1 ≤ brier_score < 0.2', async () => {
    mockKpis.mockResolvedValue({
      total_equity_usdc: '0', open_pnl_usdc: '0', win_rate_30d: 0,
      brier_score: 0.15, active_signals: 0, open_positions: 0,
    });
    mockListBets.mockResolvedValue([]);
    renderPnL();
    await waitFor(() => {
      expect(screen.getByText(/^Good$/i)).toBeInTheDocument();
    });
  });

  it('shows Acceptable text when 0.2 ≤ brier_score < 0.25', async () => {
    mockKpis.mockResolvedValue({
      total_equity_usdc: '0', open_pnl_usdc: '0', win_rate_30d: 0,
      brier_score: 0.22, active_signals: 0, open_positions: 0,
    });
    mockListBets.mockResolvedValue([]);
    renderPnL();
    await waitFor(() => {
      expect(screen.getByText(/^Acceptable$/i)).toBeInTheDocument();
    });
  });

  it('shows Needs improvement text when brier_score ≥ 0.25', async () => {
    mockKpis.mockResolvedValue({
      total_equity_usdc: '0', open_pnl_usdc: '0', win_rate_30d: 0,
      brier_score: 0.30, active_signals: 0, open_positions: 0,
    });
    mockListBets.mockResolvedValue([]);
    renderPnL();
    await waitFor(() => {
      expect(screen.getByText(/Needs improvement/i)).toBeInTheDocument();
    });
  });

  it('shows Awaiting data text when kpis.data is null', async () => {
    // kpis.data is undefined during loading — show '—' + 'Awaiting data'
    mockKpis.mockImplementation(() => new Promise(() => {})); // never resolves
    mockListBets.mockResolvedValue([]);
    renderPnL();
    await waitFor(() => {
      expect(screen.getByText(/Awaiting data/i)).toBeInTheDocument();
    });
  });

  it('renders ErrorState when kpis query fails', async () => {
    mockKpis.mockRejectedValue(new Error('IPC exploded'));
    mockListBets.mockResolvedValue([]);
    renderPnL();
    await waitFor(() => {
      expect(screen.getByText(/IPC exploded/i)).toBeInTheDocument();
    });
  });
});
